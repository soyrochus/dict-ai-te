(() => {
  const config = window.dictaiteConfig || {};
  const elements = {
    recordButton: document.getElementById('recordButton'),
    recordIcon: document.getElementById('recordIcon'),
    statusLabel: document.getElementById('statusLabel'),
    timerLabel: document.getElementById('timerLabel'),
    levelMeter: document.getElementById('levelMeter'),
    languageSelect: document.getElementById('languageSelect'),
    translateToggle: document.getElementById('translateToggle'),
    translateThumb: document.getElementById('translateThumb'),
    targetContainer: document.getElementById('targetContainer'),
    targetSelect: document.getElementById('targetSelect'),
    transcript: document.getElementById('transcript'),
    copyButton: document.getElementById('copyButton'),
    downloadButton: document.getElementById('downloadButton'),
    clearButton: document.getElementById('clearButton'),
    playButton: document.getElementById('playButton'),
    voiceRadios: document.querySelectorAll('input[name="voiceGender"]'),
    playback: document.getElementById('playback'),
  };

  const state = {
    mediaRecorder: null,
    audioChunks: [],
    isRecording: false,
    startTime: null,
    timerId: null,
    audioContext: null,
    analyser: null,
    levelArray: null,
    levelAnimation: null,
    recordedBlob: null,
  };

  function init() {
    setupTranslateToggle();
    elements.recordButton?.addEventListener('click', toggleRecording);
    elements.copyButton?.addEventListener('click', copyTranscript);
    elements.downloadButton?.addEventListener('click', downloadTranscript);
    elements.clearButton?.addEventListener('click', clearTranscript);
    elements.playButton?.addEventListener('click', playTranscript);
    window.addEventListener('keydown', handleShortcuts);
    if (elements.translateToggle) {
      updateTranslateUI(config.defaultTranslate === true || config.defaultTranslate === 'true');
    }
  }

  async function toggleRecording() {
    if (state.isRecording) {
      await stopRecording();
    } else {
      await startRecording();
    }
  }

  async function startRecording() {
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
      setStatus('Microphone access is not supported in this browser.', true);
      return;
    }
    if (typeof window.MediaRecorder === 'undefined') {
      setStatus('MediaRecorder API is not available in this browser.', true);
      return;
    }
    try {
      const constraints = { audio: true };
      const stream = await navigator.mediaDevices.getUserMedia(constraints);
      const mimeType = selectMimeType();
      state.mediaRecorder = new MediaRecorder(stream, mimeType ? { mimeType } : undefined);
      state.audioChunks = [];
      state.mediaRecorder.addEventListener('dataavailable', (event) => {
        if (event.data.size > 0) {
          state.audioChunks.push(event.data);
        }
      });
      state.mediaRecorder.addEventListener('stop', handleRecordingStop);

      setupLevelMeter(stream);
      state.mediaRecorder.start();
      state.isRecording = true;
      state.startTime = Date.now();
      state.timerId = window.setInterval(updateTimer, 1000);
      setStatus('Recording... Press to stop.');
      updateRecordVisuals(true);
    } catch (err) {
      console.error(err);
      setStatus('Unable to access microphone. Please grant permission.', true);
    }
  }

  async function stopRecording() {
    if (!state.isRecording) return;
    state.isRecording = false;
    updateRecordVisuals(false);
    setStatus('Processing audio...');
    window.clearInterval(state.timerId);
    state.timerId = null;
    resetTimer();
    if (state.levelAnimation) {
      cancelAnimationFrame(state.levelAnimation);
      state.levelAnimation = null;
    }
    if (state.mediaRecorder && state.mediaRecorder.state !== 'inactive') {
      state.mediaRecorder.stop();
    }
    if (state.audioContext) {
      state.audioContext.close().catch(() => {});
      state.audioContext = null;
    }
  }

  function handleRecordingStop() {
    const tracks = state.mediaRecorder?.stream?.getTracks?.() || [];
    tracks.forEach((track) => track.stop());
    const blob = new Blob(state.audioChunks, { type: state.mediaRecorder?.mimeType || 'audio/webm' });
    state.recordedBlob = blob;
    state.mediaRecorder = null;
    uploadRecording(blob).catch((error) => {
      console.error(error);
      setStatus(error.message || 'Failed to transcribe audio.', true);
    });
  }

  function setupLevelMeter(stream) {
    const AudioContext = window.AudioContext || window.webkitAudioContext;
    if (!AudioContext) {
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
    if (!state.analyser || !state.levelArray) return;
    state.analyser.getByteTimeDomainData(state.levelArray);
    let peak = 0;
    for (let i = 0; i < state.levelArray.length; i += 1) {
      const value = (state.levelArray[i] - 128) / 128;
      peak = Math.max(peak, Math.abs(value));
    }
    const width = Math.min(100, Math.round(peak * 140));
    elements.levelMeter.style.width = `${width}%`;
    state.levelAnimation = requestAnimationFrame(drawLevel);
  }

  function updateRecordVisuals(isRecording) {
    if (!elements.recordButton || !elements.recordIcon) return;
    elements.recordButton.classList.toggle('recording', isRecording);
    if (isRecording) {
      elements.recordIcon.innerHTML = '<rect x="5" y="5" width="10" height="10" rx="2"></rect>';
    } else {
      elements.recordIcon.innerHTML = '
        <path d="M10 4a3 3 0 0 0-3 3v4a3 3 0 1 0 6 0V7a3 3 0 0 0-3-3z"></path>
        <path d="M5 9a5 5 0 0 0 10 0h1a6 6 0 0 1-5 5.917V18h-2v-3.083A6 6 0 0 1 4 9h1z"></path>';
    }
  }

  function updateTimer() {
    if (!state.startTime) return;
    const elapsed = Math.floor((Date.now() - state.startTime) / 1000);
    elements.timerLabel.textContent = formatDuration(elapsed);
    if (elapsed >= 120) {
      stopRecording();
    }
  }

  function resetTimer() {
    elements.timerLabel.textContent = '00:00:00';
  }

  function formatDuration(totalSeconds) {
    const hours = String(Math.floor(totalSeconds / 3600)).padStart(2, '0');
    const minutes = String(Math.floor((totalSeconds % 3600) / 60)).padStart(2, '0');
    const seconds = String(totalSeconds % 60).padStart(2, '0');
    return `${hours}:${minutes}:${seconds}`;
  }

  function selectMimeType() {
    const preferred = ['audio/webm;codecs=opus', 'audio/webm', 'audio/ogg;codecs=opus', 'audio/ogg'];
    for (const type of preferred) {
      if (MediaRecorder.isTypeSupported?.(type)) {
        return type;
      }
    }
    return null;
  }

  async function uploadRecording(blob) {
    const form = new FormData();
    const language = elements.languageSelect?.value || '';
    const translate = elements.translateToggle?.dataset.active === 'true';
    const target = elements.targetSelect?.value || '';
    form.append('audio', blob, 'recording.webm');
    form.append('language', language);
    form.append('translate', translate ? 'true' : 'false');
    if (translate) {
      form.append('target_lang', target);
    }

    setStatus('Transcribing...');
    const response = await fetch(config.transcribeUrl, { method: 'POST', body: form });
    if (!response.ok) {
      const payload = await response.json().catch(() => ({}));
      const message = payload?.error?.message || 'Transcription failed.';
      throw new Error(message);
    }
    const result = await response.json();
    const text = result.translatedText || result.text || '';
    elements.transcript.value = text;
    setStatus('Ready');
  }

  async function copyTranscript() {
    const text = elements.transcript?.value || '';
    if (!text.trim()) {
      setStatus('Transcript area is empty.', true);
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
      setStatus('Transcript copied to clipboard.');
    } catch (err) {
      console.error(err);
      setStatus('Clipboard permissions denied.', true);
    }
  }

  function downloadTranscript() {
    const text = elements.transcript?.value || '';
    if (!text.trim()) {
      setStatus('Transcript area is empty.', true);
      return;
    }
    const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = 'transcript.txt';
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
    setStatus('Transcript downloaded.');
  }

  function clearTranscript() {
    elements.transcript.value = '';
    state.recordedBlob = null;
    setStatus('Transcript cleared.');
  }

  async function playTranscript() {
    const text = elements.transcript?.value?.trim() || '';
    if (!text) {
      setStatus('Transcript area is empty.', true);
      return;
    }
    const gender = Array.from(elements.voiceRadios || []).find((radio) => radio.checked)?.value || 'female';
    setStatus('Generating audio...');
    try {
      const response = await fetch(config.ttsUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ gender, text }),
      });
      if (!response.ok) {
        const payload = await response.json().catch(() => ({}));
        const message = payload?.error?.message || 'Playback failed.';
        throw new Error(message);
      }
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      elements.playback.src = url;
      elements.playback.play().catch((err) => {
        console.error(err);
        setStatus('Unable to start playback.', true);
      });
      elements.playback.addEventListener(
        'ended',
        () => {
          URL.revokeObjectURL(url);
        },
        { once: true }
      );
      setStatus('Playing preview...');
    } catch (err) {
      console.error(err);
      setStatus(err.message || 'Playback failed.', true);
    }
  }

  function setupTranslateToggle() {
    if (!elements.translateToggle) return;
    elements.translateToggle.addEventListener('click', () => {
      const active = elements.translateToggle.dataset.active === 'true';
      updateTranslateUI(!active);
    });
  }

  function updateTranslateUI(active) {
    if (!elements.translateToggle || !elements.translateThumb || !elements.targetContainer) return;
    elements.translateToggle.dataset.active = active ? 'true' : 'false';
    elements.targetContainer.toggleAttribute('hidden', !active);
  }

  function handleShortcuts(event) {
    const isInTextarea = document.activeElement === elements.transcript;
    if (!isInTextarea && event.code === 'Space') {
      event.preventDefault();
      toggleRecording();
      return;
    }
    const isModifier = event.metaKey || event.ctrlKey;
    if (!isModifier) return;
    if (event.key.toLowerCase() === 'c') {
      event.preventDefault();
      copyTranscript();
    } else if (event.key.toLowerCase() === 's') {
      event.preventDefault();
      downloadTranscript();
    }
  }

  function setStatus(message, isError = false) {
    if (!elements.statusLabel) return;
    elements.statusLabel.textContent = message;
    elements.statusLabel.classList.toggle('text-red-400', Boolean(isError));
  }

  init();
})();
