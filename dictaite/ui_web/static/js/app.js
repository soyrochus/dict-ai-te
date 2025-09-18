(() => {
  const config = window.dictaiteConfig || {};

  const elements = {
    modeSelect: document.getElementById('modeSelect'),
    targetContainer: document.getElementById('targetContainer'),
    targetSelect: document.getElementById('targetSelect'),
    transcript: document.getElementById('transcript'),
    copyButton: document.getElementById('copyButton'),
    downloadButton: document.getElementById('downloadButton'),
    clearButton: document.getElementById('clearButton'),
    playButton: document.getElementById('playButton'),
    voiceRadios: document.querySelectorAll('input[name="voiceGender"]'),
    playback: document.getElementById('playback'),
    recordStatus: document.getElementById('recordStatus'),
  };

  function init() {
    setupModeSelect();
    elements.copyButton?.addEventListener('click', copyTranscript);
    elements.downloadButton?.addEventListener('click', downloadTranscript);
    elements.clearButton?.addEventListener('click', clearTranscript);
    elements.playButton?.addEventListener('click', playTranscript);
    window.addEventListener('keydown', handleShortcuts);
  }

  function setupModeSelect() {
    if (!elements.modeSelect) {
      return;
    }
    updateTargetVisibility(isTranslateSelected());
    elements.modeSelect.addEventListener('change', () => {
      updateTargetVisibility(isTranslateSelected());
    });
  }

  function isTranslateSelected() {
    return elements.modeSelect?.value === 'translate';
  }

  function updateTargetVisibility(active) {
    if (!elements.targetContainer) {
      return;
    }
    elements.targetContainer.toggleAttribute('hidden', !active);
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
    } catch (error) {
      console.error(error);
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
    if (elements.transcript) {
      elements.transcript.value = '';
    }
    setStatus('Transcript cleared.');
  }

  async function playTranscript() {
    const text = elements.transcript?.value?.trim() || '';
    if (!text) {
      setStatus('Transcript area is empty.', true);
      return;
    }
    const gender = Array.from(elements.voiceRadios || []).find((radio) => radio.checked)?.value || 'female';
    setStatus('Generating audio preview...');
    try {
      const response = await fetch(config.ttsUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ gender, text }),
      });
      if (!response.ok) {
        const payload = await response.json().catch(() => null);
        const message = payload?.error?.message || 'Playback failed.';
        throw new Error(message);
      }
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      if (elements.playback) {
        elements.playback.src = url;
        elements.playback.play().catch((error) => {
          console.error(error);
          setStatus('Unable to start playback.', true);
        });
        elements.playback.addEventListener(
          'ended',
          () => {
            URL.revokeObjectURL(url);
          },
          { once: true }
        );
      }
      setStatus('Playing preview...');
    } catch (error) {
      console.error(error);
      setStatus(error.message || 'Playback failed.', true);
    }
  }

  function handleShortcuts(event) {
    const isModifier = event.metaKey || event.ctrlKey;
    if (!isModifier) {
      return;
    }
    const key = event.key.toLowerCase();
    if (key === 'c') {
      event.preventDefault();
      copyTranscript();
    } else if (key === 's') {
      event.preventDefault();
      downloadTranscript();
    }
  }

  function setStatus(message, isError = false) {
    if (!elements.recordStatus) {
      return;
    }
    elements.recordStatus.textContent = message;
    elements.recordStatus.classList.toggle('text-red-400', Boolean(isError));
  }

  init();
})();
