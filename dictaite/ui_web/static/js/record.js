(() => {
  const DEBUG_RECORDING = Boolean(window.DEBUG_RECORDING);
  const config = window.dictaiteConfig || {};
  const START_LABEL = 'Toggle Recording';
  const STOP_LABEL = 'Stop Recording';

  const elements = {
    recordBtn: document.getElementById('recordBtn'),
    recordBtnText: document.getElementById('recordBtnText'),
    recordCaption: document.getElementById('recordCaption'),
    recordTimer: document.getElementById('recordTimer'),
    recordStatus: document.getElementById('recordStatus'),
    recordRetryBtn: document.getElementById('recordRetryBtn'),
    recordCancelBtn: document.getElementById('recordCancelBtn'),
    languageSelect: document.getElementById('languageSelect'),
    targetSelect: document.getElementById('targetSelect'),
    modeSelect: document.getElementById('modeSelect'),
    transcript: document.getElementById('transcript'),
    levelMeter: document.getElementById('levelMeter'),
  };

  const state = {
    current: 'idle',
    mode: 'transcribe',
    language: null,
    targetLang: null,
    sessionId: null,
    mimeType: null,
    mediaRecorder: null,
    mediaStream: null,
    audioContext: null,
    analyser: null,
    levelArray: null,
    levelAnimation: null,
    timerInterval: null,
    startTimestamp: null,
    nextSeq: 0,
    chunkQueue: [],
    uploading: false,
    awaitingFinalization: false,
    failedChunk: null,
    lastFinalizePayload: null,
    lastErrorMessage: '',
  };

  function init() {
    if (!elements.recordBtn || !elements.recordCaption || !elements.recordTimer) {
      return;
    }
    state.mode = getSelectedMode();
    state.targetLang = state.mode === 'translate' ? getSelectedTargetLang() : null;
    setControlsDisabled(false);
    elements.recordBtn.addEventListener('click', handleToggleRecording);
    elements.recordBtn.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        handleToggleRecording();
      }
    });
    window.addEventListener('keydown', handleGlobalKeys, { capture: true });
    window.addEventListener('pagehide', handlePageHide);
    window.addEventListener('beforeunload', handlePageHide);
    elements.recordRetryBtn?.addEventListener('click', handleRetry);
    elements.recordCancelBtn?.addEventListener('click', handleCancel);
    elements.modeSelect?.addEventListener('change', handleModeChange);
    elements.targetSelect?.addEventListener('change', () => {
      state.targetLang = getSelectedTargetLang();
    });
    updateUI('idle');
  }

  function handleGlobalKeys(event) {
    if (event.code === 'Escape' && state.current === 'recording') {
      event.preventDefault();
      stopRecording();
      return;
    }
    if (event.code === 'Space') {
      const active = document.activeElement;
      const isTextInput = active && (active.tagName === 'TEXTAREA' || active.tagName === 'INPUT');
      if (active === elements.recordBtn || isTextInput) {
        return;
      }
      event.preventDefault();
      handleToggleRecording();
    }
  }

  function handlePageHide() {
    if (state.sessionId) {
      cancelRecordingSession(false);
    }
  }

  async function handleToggleRecording() {
    if (state.current === 'recording') {
      stopRecording();
      return;
    }
    if (state.current === 'idle' || state.current === 'done' || state.current === 'error') {
      await startRecording();
    }
  }

  async function startRecording() {
    if (!navigator.mediaDevices?.getUserMedia) {
      setErrorState('Microphone access is not supported in this browser.');
      return;
    }
    if (typeof window.MediaRecorder === 'undefined') {
      setErrorState('MediaRecorder API is not available in this browser.');
      return;
    }

    resetStateForNewRecording();

    state.mode = getSelectedMode();
    state.language = getSelectedLanguage();
    state.targetLang = state.mode === 'translate' ? getSelectedTargetLang() : null;
    state.mimeType = selectMimeType();

    updateUI('preparing');

    let stream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch (error) {
      setErrorState('Unable to access microphone. Please grant permission.');
      return;
    }

    let sessionId;
    try {
      sessionId = await createRecordingSession();
    } catch (error) {
      stopStream(stream);
      setErrorState(error.message || 'Could not start recording session.');
      return;
    }

    let recorder;
    try {
      recorder = buildMediaRecorder(stream);
    } catch (error) {
      stopStream(stream);
      await cancelSessionOnServer(sessionId);
      setErrorState('Unable to initialise audio recorder.');
      return;
    }

    recorder.addEventListener('dataavailable', handleDataAvailable);
    recorder.addEventListener('error', (event) => {
      console.error(event.error);
      setErrorState('Recording failed. Please try again.');
    });
    recorder.addEventListener('stop', handleRecorderStop);

    state.mediaRecorder = recorder;
    state.mediaStream = stream;
    state.sessionId = sessionId;
    state.nextSeq = 0;
    state.chunkQueue = [];
    state.uploading = false;
    state.awaitingFinalization = false;
    setupLevelMeter(stream);

    recorder.start(1000);
    state.startTimestamp = Date.now();
    state.timerInterval = window.setInterval(updateTimer, 1000);
    updateUI('recording');
  }

  function stopRecording() {
    if (state.current !== 'recording') {
      return;
    }
    state.awaitingFinalization = true;
    updateUI('uploading');
    clearTimer();
    if (state.mediaRecorder && state.mediaRecorder.state !== 'inactive') {
      state.mediaRecorder.stop();
    }
    stopLevelMeter();
    if (state.mediaStream) {
      stopStream(state.mediaStream);
      state.mediaStream = null;
    }
    maybeFinalize();
  }

  function handleRecorderStop() {
    stopLevelMeter();
    state.mediaRecorder = null;
    if (state.awaitingFinalization) {
      maybeFinalize();
    }
  }

  function handleDataAvailable(event) {
    if (!event.data || !event.data.size) {
      return;
    }
    const chunk = { seq: state.nextSeq, blob: event.data };
    state.nextSeq += 1;
    state.chunkQueue.push(chunk);
    processQueue();
  }

  function processQueue() {
    if (state.uploading || state.chunkQueue.length === 0 || !state.sessionId) {
      return;
    }
    const { seq, blob } = state.chunkQueue[0];
    state.uploading = postChunk(seq, blob)
      .then(() => {
        state.chunkQueue.shift();
        state.failedChunk = null;
        state.uploading = false;
        if (state.chunkQueue.length > 0) {
          processQueue();
        } else {
          maybeFinalize();
        }
      })
      .catch((error) => {
        console.error(error);
        if (state.mediaRecorder && state.mediaRecorder.state === 'recording') {
          try {
            state.mediaRecorder.stop();
          } catch (err) {
            if (DEBUG_RECORDING) {
              console.debug('Failed to stop recorder after chunk error', err);
            }
          }
        }
        if (state.mediaStream) {
          stopStream(state.mediaStream);
          state.mediaStream = null;
        }
        state.awaitingFinalization = true;
        state.uploading = false;
        state.failedChunk = { seq, blob };
        state.lastErrorMessage = error.message || 'Chunk upload failed.';
        updateUI('error');
      });
  }

  async function postChunk(seq, blob) {
    const form = new FormData();
    form.append('session_id', state.sessionId);
    form.append('seq', String(seq));
    form.append('chunk', blob, `chunk-${seq}.webm`);
    const response = await fetch(config.recordAppendUrl, { method: 'POST', body: form });
    if (!response.ok) {
      const payload = await response.json().catch(() => null);
      const message = payload?.error?.message || 'Chunk upload failed.';
      throw new Error(message);
    }
    if (DEBUG_RECORDING) {
      console.debug('Uploaded chunk', seq);
    }
  }

  function maybeFinalize() {
    if (!state.awaitingFinalization || state.uploading || state.chunkQueue.length) {
      return;
    }
    if (state.current !== 'error' && state.current !== 'processing' && state.current !== 'done') {
      void finalizeRecording();
    }
  }

  async function finalizeRecording() {
    if (!state.sessionId) {
      return;
    }
    state.current = 'processing';
    updateUI('processing');
    const payload = {
      session_id: state.sessionId,
      mode: state.mode,
      language: state.language,
      target_lang: state.mode === 'translate' ? state.targetLang : null,
    };
    state.lastFinalizePayload = payload;
    try {
      const response = await fetch(config.recordFinalizeUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      if (!response.ok) {
        const data = await response.json().catch(() => null);
        const message = data?.error?.message || 'Processing failed.';
        throw new Error(message);
      }
      const result = await response.json();
      const text = result.translatedText || result.text || '';
      if (elements.transcript) {
        elements.transcript.value = text;
      }
      state.current = 'done';
      state.lastFinalizePayload = null;
      updateUI('done');
      resetSession();
    } catch (error) {
      console.error(error);
      state.lastErrorMessage = error.message || 'Processing failed.';
      state.current = 'error';
      updateUI('error');
    }
  }

  function resetStateForNewRecording() {
    state.sessionId = null;
    state.mediaRecorder = null;
    state.mediaStream = null;
    state.mimeType = null;
    state.nextSeq = 0;
    state.chunkQueue = [];
    state.failedChunk = null;
    state.lastFinalizePayload = null;
    state.awaitingFinalization = false;
    state.lastErrorMessage = '';
    clearTimer();
    stopLevelMeter();
  }

  function resetSession() {
    if (!state.sessionId) {
      return;
    }
    cancelSessionOnServer(state.sessionId);
    state.sessionId = null;
  }

  async function cancelRecordingSession(showIdle = true) {
    if (!state.sessionId) {
      updateUI(showIdle ? 'idle' : state.current);
      return;
    }
    await cancelSessionOnServer(state.sessionId);
    resetStateForNewRecording();
    if (showIdle) {
      updateUI('idle');
    }
  }

  async function cancelSessionOnServer(sessionId) {
    try {
      const url = new URL(config.recordCancelUrl, window.location.origin);
      url.searchParams.set('session_id', sessionId);
      await fetch(url.toString(), { method: 'POST' });
    } catch (error) {
      if (DEBUG_RECORDING) {
        console.debug('Failed to cancel session', error);
      }
    }
  }

  async function createRecordingSession() {
    const response = await fetch(config.recordStartUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        mime_type: state.mimeType,
        mode: state.mode,
        language: state.language,
        target_lang: state.mode === 'translate' ? state.targetLang : null,
      }),
    });
    if (!response.ok) {
      const payload = await response.json().catch(() => null);
      const message = payload?.error?.message || 'Failed to initialise recording.';
      throw new Error(message);
    }
    const data = await response.json();
    return data.session_id;
  }

  function buildMediaRecorder(stream) {
    if (!state.mimeType) {
      return new MediaRecorder(stream);
    }
    try {
      return new MediaRecorder(stream, { mimeType: state.mimeType });
    } catch (error) {
      console.warn('Falling back to default MediaRecorder options', error);
      state.mimeType = null;
      return new MediaRecorder(stream);
    }
  }

  function selectMimeType() {
    const preferred = [
      'audio/webm;codecs=opus',
      'audio/webm',
      'audio/ogg;codecs=opus',
      'audio/ogg',
      'audio/wav',
    ];
    if (!window.MediaRecorder || typeof MediaRecorder.isTypeSupported !== 'function') {
      return null;
    }
    for (const type of preferred) {
      if (MediaRecorder.isTypeSupported(type)) {
        return type;
      }
    }
    return null;
  }

  function setupLevelMeter(stream) {
    const AudioContext = window.AudioContext || window.webkitAudioContext;
    if (!AudioContext || !elements.levelMeter) {
      return;
    }
    state.audioContext = new AudioContext();
    const source = state.audioContext.createMediaStreamSource(stream);
    state.analyser = state.audioContext.createAnalyser();
    state.analyser.fftSize = 2048;
    state.levelArray = new Uint8Array(state.analyser.frequencyBinCount);
    source.connect(state.analyser);
    drawLevel();
  }

  function drawLevel() {
    if (!state.analyser || !state.levelArray || !elements.levelMeter) {
      return;
    }
    state.analyser.getByteTimeDomainData(state.levelArray);
    let peak = 0;
    for (let i = 0; i < state.levelArray.length; i += 1) {
      const value = (state.levelArray[i] - 128) / 128;
      peak = Math.max(peak, Math.abs(value));
    }
    elements.levelMeter.style.width = `${Math.min(100, Math.round(peak * 140))}%`;
    state.levelAnimation = requestAnimationFrame(drawLevel);
  }

  function stopLevelMeter() {
    if (state.levelAnimation) {
      cancelAnimationFrame(state.levelAnimation);
      state.levelAnimation = null;
    }
    if (state.audioContext) {
      state.audioContext.close().catch(() => {});
      state.audioContext = null;
    }
    if (elements.levelMeter) {
      elements.levelMeter.style.width = '0%';
    }
  }

  function updateTimer() {
    if (!state.startTimestamp || !elements.recordTimer) {
      return;
    }
    const elapsed = Math.floor((Date.now() - state.startTimestamp) / 1000);
    if (elapsed >= 120) {
      stopRecording();
    }
    elements.recordTimer.textContent = formatElapsed(elapsed);
  }

  function clearTimer() {
    if (state.timerInterval) {
      window.clearInterval(state.timerInterval);
      state.timerInterval = null;
    }
    state.startTimestamp = null;
    if (elements.recordTimer) {
      elements.recordTimer.textContent = '00:00';
    }
  }

  function formatElapsed(seconds) {
    const mins = String(Math.floor(seconds / 60)).padStart(2, '0');
    const secs = String(seconds % 60).padStart(2, '0');
    return `${mins}:${secs}`;
  }

  function setErrorState(message) {
    state.lastErrorMessage = message;
    state.current = 'error';
    updateUI('error');
  }

  function updateUI(nextState) {
    if (!elements.recordBtn || !elements.recordCaption) {
      return;
    }
    const stateName = nextState || state.current || 'idle';
    state.current = stateName;
    const disableControls = stateName === 'recording' || stateName === 'uploading' || stateName === 'processing';
    setControlsDisabled(disableControls);
    switch (stateName) {
      case 'recording':
        elements.recordBtn.setAttribute('aria-label', 'Stop recording');
        elements.recordBtn.dataset.state = 'recording';
        elements.recordCaption.textContent = 'Recording...';
        updateStatus('Press Stop Recording or press Escape to stop.');
        toggleErrorActions(false);
        break;
      case 'uploading':
        elements.recordBtn.setAttribute('aria-label', 'Uploading audio');
        elements.recordBtn.dataset.state = 'uploading';
        elements.recordCaption.textContent = 'Uploading...';
        updateStatus('Uploading audio chunks to the server.');
        toggleErrorActions(false);
        break;
      case 'processing':
        elements.recordBtn.setAttribute('aria-label', 'Processing recording');
        elements.recordBtn.dataset.state = 'processing';
        elements.recordCaption.textContent = 'Processing...';
        updateStatus('Waiting for transcription results.');
        toggleErrorActions(false);
        break;
      case 'done':
        elements.recordBtn.setAttribute('aria-label', 'Start recording');
        elements.recordBtn.dataset.state = 'idle';
        elements.recordCaption.textContent = 'Done';
        updateStatus('Transcript updated below.');
        toggleErrorActions(false);
        clearTimer();
        state.awaitingFinalization = false;
        break;
      case 'error':
        elements.recordBtn.setAttribute('aria-label', 'Retry recording');
        elements.recordBtn.dataset.state = 'error';
        elements.recordCaption.textContent = 'Recording stopped';
        updateStatus(state.lastErrorMessage || 'An error occurred.', true);
        toggleErrorActions(true);
        break;
      case 'preparing':
        elements.recordBtn.setAttribute('aria-label', 'Starting recording');
        elements.recordBtn.dataset.state = 'preparing';
        elements.recordCaption.textContent = 'Preparing...';
        updateStatus('Requesting microphone access.');
        toggleErrorActions(false);
        break;
      case 'idle':
      default:
        elements.recordBtn.setAttribute('aria-label', 'Start recording');
        elements.recordBtn.dataset.state = 'idle';
        elements.recordCaption.textContent = 'Ready';
        updateStatus('Press Toggle Recording to start.');
        toggleErrorActions(false);
        clearTimer();
        state.awaitingFinalization = false;
        break;
    }
    if (elements.recordBtnText) {
      elements.recordBtnText.textContent = stateName === 'recording' ? STOP_LABEL : START_LABEL;
    }
    elements.recordBtn.disabled = stateName === 'processing';
    elements.recordBtn.setAttribute('aria-pressed', stateName === 'recording' ? 'true' : 'false');
  }

  function toggleErrorActions(show) {
    if (!elements.recordRetryBtn || !elements.recordCancelBtn) {
      return;
    }
    elements.recordRetryBtn.toggleAttribute('hidden', !show);
    elements.recordCancelBtn.toggleAttribute('hidden', !show);
    if (!show) {
      updateStatus('');
    }
  }

  function handleRetry() {
    if (state.failedChunk) {
      state.chunkQueue.unshift(state.failedChunk);
      state.failedChunk = null;
      state.lastErrorMessage = '';
      updateUI('uploading');
      processQueue();
      return;
    }
    if (state.lastFinalizePayload) {
      state.lastErrorMessage = '';
      updateUI('processing');
      void finalizeRecording();
    }
  }

  function handleCancel() {
    void cancelRecordingSession();
  }

  function stopStream(stream) {
    const tracks = stream.getTracks?.() || [];
    tracks.forEach((track) => track.stop());
  }

  function getSelectedMode() {
    const value = elements.modeSelect?.value || 'transcribe';
    return value === 'translate' ? 'translate' : 'transcribe';
  }

  function getSelectedLanguage() {
    const value = elements.languageSelect?.value || '';
    return value === 'default' ? null : value;
  }

  function getSelectedTargetLang() {
    const value = elements.targetSelect?.value || '';
    return value === 'default' ? null : value;
  }

  function handleModeChange() {
    state.mode = getSelectedMode();
    state.targetLang = state.mode === 'translate' ? getSelectedTargetLang() : null;
    const disableControls = state.current === 'recording' || state.current === 'uploading' || state.current === 'processing';
    setControlsDisabled(disableControls);
  }

  function updateStatus(message, isError = false) {
    if (elements.recordStatus) {
      elements.recordStatus.textContent = message;
      elements.recordStatus.classList.toggle('text-red-400', Boolean(isError));
    }
  }

  function setControlsDisabled(disabled) {
    if (elements.modeSelect) {
      elements.modeSelect.disabled = disabled;
    }
    if (elements.languageSelect) {
      elements.languageSelect.disabled = disabled;
    }
    if (elements.targetSelect) {
      elements.targetSelect.disabled = disabled || state.mode !== 'translate';
    }
  }

  init();
})();
