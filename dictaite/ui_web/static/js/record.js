(() => {
  const config = window.dictaiteConfig || {};

  const elements = {
    recordBtn: document.getElementById('recordBtn'),
    recordBtnText: document.getElementById('recordBtnText'),
    recordCaption: document.getElementById('recordCaption'),
    recordTimer: document.getElementById('recordTimer'),
    recordStatus: document.getElementById('recordStatus'),
    languageSelect: document.getElementById('languageSelect'),
    targetSelect: document.getElementById('targetSelect'),
    translateLive: document.getElementById('translateLive'),
    transcript: document.getElementById('transcript'),
    translatedTranscript: document.getElementById('translatedTranscript'),
    levelMeter: document.getElementById('levelMeter'),
  };

  const state = {
    current: 'disconnected',
    socket: null,
    mediaStream: null,
    audioContext: null,
    processor: null,
    source: null,
    timerInterval: null,
    startTimestamp: null,
    sourceSegments: new Map(),
    anonymousSource: '',
  };

  function init() {
    elements.recordBtn?.addEventListener('click', toggleListening);
    window.addEventListener('keydown', handleGlobalKeys, { capture: true });
    window.addEventListener('pagehide', stopListening);
    updateUI('disconnected');
  }

  async function toggleListening() {
    if (state.current === 'listening' || state.current === 'connecting') {
      stopListening();
    } else {
      await startListening();
    }
  }

  async function startListening() {
    if (!navigator.mediaDevices?.getUserMedia) {
      setError('Microphone access is not supported in this browser.');
      return;
    }
    resetTranscriptAssembly();
    updateUI('connecting');

    let stream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch (error) {
      setError('Unable to access microphone. Please grant permission.');
      return;
    }

    try {
      state.mediaStream = stream;
      state.audioContext = new AudioContext();
      state.source = state.audioContext.createMediaStreamSource(stream);
      state.processor = state.audioContext.createScriptProcessor(4096, 1, 1);
      state.source.connect(state.processor);
      state.processor.connect(state.audioContext.destination);
      connectWebSocket();
      state.processor.onaudioprocess = handleAudioProcess;
      state.startTimestamp = Date.now();
      state.timerInterval = window.setInterval(updateTimer, 1000);
      updateUI('listening');
    } catch (error) {
      console.error(error);
      stopStream(stream);
      setError('Unable to start live audio capture.');
    }
  }

  function connectWebSocket() {
    const translate = Boolean(elements.translateLive?.checked);
    const path = translate ? config.liveTranslateUrl : config.liveTranscribeUrl;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const socket = new WebSocket(`${protocol}//${window.location.host}${path}`);
    state.socket = socket;

    socket.addEventListener('open', () => {
      socket.send(JSON.stringify({
        type: 'start',
        source_language: selectedValue(elements.languageSelect),
        target_language: translate ? selectedValue(elements.targetSelect) : null,
      }));
      updateStatus(translate ? 'Translating live' : 'Transcribing live');
    });
    socket.addEventListener('message', (event) => {
      const payload = JSON.parse(event.data);
      handleServerEvent(payload);
    });
    socket.addEventListener('close', () => {
      if (state.current !== 'disconnected') {
        updateUI('disconnected');
      }
    });
    socket.addEventListener('error', () => {
      setError('Live connection failed.');
    });
  }

  function handleAudioProcess(event) {
    if (!state.socket || state.socket.readyState !== WebSocket.OPEN) {
      return;
    }
    const input = event.inputBuffer.getChannelData(0);
    const level = input.reduce((max, value) => Math.max(max, Math.abs(value)), 0);
    if (elements.levelMeter) {
      elements.levelMeter.style.width = `${Math.min(100, Math.round(level * 100))}%`;
    }
    const pcm = encodePcm16(resample(input, state.audioContext.sampleRate, 24000));
    state.socket.send(JSON.stringify({ type: 'audio', audio: bytesToBase64(pcm) }));
  }

  function stopListening() {
    if (state.socket && state.socket.readyState === WebSocket.OPEN) {
      state.socket.send(JSON.stringify({ type: 'stop' }));
      state.socket.close();
    }
    state.socket = null;
    if (state.processor) {
      state.processor.disconnect();
      state.processor.onaudioprocess = null;
    }
    state.processor = null;
    if (state.source) {
      state.source.disconnect();
    }
    state.source = null;
    if (state.audioContext) {
      state.audioContext.close().catch(() => {});
    }
    state.audioContext = null;
    stopStream(state.mediaStream);
    state.mediaStream = null;
    clearTimer();
    if (elements.levelMeter) {
      elements.levelMeter.style.width = '0%';
    }
    updateUI('disconnected');
  }

  function handleServerEvent(payload) {
    if (payload.type === 'source_delta') {
      applySourceDelta(payload.text || '', payload.item_id || null);
    } else if (payload.type === 'source_completed') {
      applySourceCompleted(payload.text || '', payload.item_id || null);
    } else if (payload.type === 'translation_delta') {
      if (elements.translatedTranscript) {
        elements.translatedTranscript.value += payload.text || '';
      }
    } else if (payload.type === 'session_state' && payload.state) {
      updateStatus(payload.state.replaceAll('.', ' '));
    } else if (payload.type === 'error') {
      setError(payload.error || 'Live session failed.');
    }
  }

  function applySourceDelta(text, itemId) {
    if (!text) return;
    if (!itemId) {
      state.anonymousSource += text;
    } else {
      state.sourceSegments.set(itemId, (state.sourceSegments.get(itemId) || '') + text);
    }
    renderSourceTranscript();
  }

  function applySourceCompleted(text, itemId) {
    if (!text) return;
    if (!itemId) {
      state.anonymousSource += text;
    } else {
      state.sourceSegments.set(itemId, text);
    }
    renderSourceTranscript();
  }

  function renderSourceTranscript() {
    if (!elements.transcript) return;
    const segments = Array.from(state.sourceSegments.values()).filter(Boolean);
    if (state.anonymousSource.trim()) {
      segments.push(state.anonymousSource);
    }
    elements.transcript.value = segments.join(' ').trim();
  }

  function resample(input, sourceRate, targetRate) {
    if (sourceRate === targetRate) {
      return input;
    }
    const ratio = sourceRate / targetRate;
    const output = new Float32Array(Math.round(input.length / ratio));
    for (let i = 0; i < output.length; i += 1) {
      output[i] = input[Math.min(input.length - 1, Math.floor(i * ratio))];
    }
    return output;
  }

  function encodePcm16(samples) {
    const output = new Int16Array(samples.length);
    for (let i = 0; i < samples.length; i += 1) {
      const sample = Math.max(-1, Math.min(1, samples[i]));
      output[i] = sample < 0 ? sample * 32768 : sample * 32767;
    }
    return new Uint8Array(output.buffer);
  }

  function bytesToBase64(bytes) {
    let binary = '';
    for (let i = 0; i < bytes.byteLength; i += 1) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  }

  function handleGlobalKeys(event) {
    if (event.code === 'Escape' && state.current === 'listening') {
      event.preventDefault();
      stopListening();
      return;
    }
    if (event.code === 'Space') {
      const active = document.activeElement;
      const isTextInput = active && (active.tagName === 'TEXTAREA' || active.tagName === 'INPUT');
      if (active === elements.recordBtn || isTextInput) return;
      event.preventDefault();
      void toggleListening();
    }
  }

  function updateUI(nextState) {
    state.current = nextState;
    const listening = nextState === 'listening';
    elements.recordBtn?.setAttribute('aria-pressed', String(listening));
    if (elements.recordBtnText) {
      elements.recordBtnText.textContent = listening ? 'Stop Listening' : 'Start Listening';
    }
    if (elements.recordCaption) {
      elements.recordCaption.textContent = listening ? 'Listening' : 'Ready to listen';
    }
    updateStatus(nextState);
  }

  function updateStatus(message) {
    if (elements.recordStatus) {
      elements.recordStatus.textContent = message;
      elements.recordStatus.classList.toggle('text-red-400', false);
    }
  }

  function setError(message) {
    updateUI('error');
    if (elements.recordStatus) {
      elements.recordStatus.textContent = message;
      elements.recordStatus.classList.toggle('text-red-400', true);
    }
  }

  function updateTimer() {
    if (!elements.recordTimer || !state.startTimestamp) return;
    const elapsed = Math.floor((Date.now() - state.startTimestamp) / 1000);
    const minutes = String(Math.floor(elapsed / 60)).padStart(2, '0');
    const seconds = String(elapsed % 60).padStart(2, '0');
    elements.recordTimer.textContent = `${minutes}:${seconds}`;
  }

  function clearTimer() {
    if (state.timerInterval) {
      window.clearInterval(state.timerInterval);
    }
    state.timerInterval = null;
    state.startTimestamp = null;
    if (elements.recordTimer) {
      elements.recordTimer.textContent = '00:00';
    }
  }

  function resetTranscriptAssembly() {
    state.sourceSegments.clear();
    state.anonymousSource = '';
  }

  function stopStream(stream) {
    stream?.getTracks?.().forEach((track) => track.stop());
  }

  function selectedValue(select) {
    const value = select?.value || null;
    return value === 'default' ? null : value;
  }

  init();
})();
