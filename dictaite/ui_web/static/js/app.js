(() => {
  const config = window.dictaiteConfig || {};

  const elements = {
    translateLive: document.getElementById('translateLive'),
    targetContainer: document.getElementById('targetContainer'),
    translationPane: document.getElementById('translationPane'),
    targetSelect: document.getElementById('targetSelect'),
    transcript: document.getElementById('transcript'),
    translatedTranscript: document.getElementById('translatedTranscript'),
    copyButton: document.getElementById('copyButton'),
    downloadButton: document.getElementById('downloadButton'),
    clearButton: document.getElementById('clearButton'),
    playButton: document.getElementById('playButton'),
    voiceRadios: document.querySelectorAll('input[name="voiceGender"]'),
    playback: document.getElementById('playback'),
    recordStatus: document.getElementById('recordStatus'),
  };

  function init() {
    setupTranslateToggle();
    elements.copyButton?.addEventListener('click', copyTranscript);
    elements.downloadButton?.addEventListener('click', downloadTranscript);
    elements.clearButton?.addEventListener('click', clearTranscript);
    elements.playButton?.addEventListener('click', playTranscript);
    window.addEventListener('keydown', handleShortcuts);
  }

  function setupTranslateToggle() {
    if (!elements.translateLive) {
      return;
    }
    updateTargetVisibility(isTranslateSelected());
    elements.translateLive.addEventListener('change', () => {
      updateTargetVisibility(isTranslateSelected());
    });
  }

  function isTranslateSelected() {
    return Boolean(elements.translateLive?.checked);
  }

  function updateTargetVisibility(active) {
    if (!elements.targetContainer) {
      elements.targetContainer = document.getElementById('targetContainer'); // ensure reference exists after DOM changes
    }
    if (!elements.targetContainer) return;
    elements.targetContainer.toggleAttribute('hidden', !active);
    elements.translationPane?.toggleAttribute('hidden', !active);
  }

  async function copyTranscript() {
    const text = combinedTranscript();
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
    const text = combinedTranscript();
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
    if (elements.translatedTranscript) {
      elements.translatedTranscript.value = '';
    }
    setStatus('Transcript cleared.');
  }

  async function playTranscript() {
    const text = (elements.translatedTranscript?.value || elements.transcript?.value || '').trim();
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

  function combinedTranscript() {
    const source = elements.transcript?.value?.trim() || '';
    const translated = elements.translatedTranscript?.value?.trim() || '';
    if (source && translated) {
      return `Source:\n${source}\n\nTranslation:\n${translated}`;
    }
    return translated || source;
  }

  init();
})();
