(() => {
  const config = window.dictaiteSettingsConfig || {};
  const elements = {
    translateToggle: document.getElementById('translateDefault'),
    targetGroup: document.getElementById('defaultTargetGroup'),
    defaultLanguage: document.getElementById('defaultLanguage'),
    defaultTarget: document.getElementById('defaultTarget'),
    femaleVoice: document.getElementById('femaleVoice'),
    maleVoice: document.getElementById('maleVoice'),
    saveButton: document.getElementById('saveSettings'),
    previewAudio: document.getElementById('voicePreview'),
  };

  function init() {
    elements.translateToggle?.addEventListener('click', () => {
      const active = elements.translateToggle.dataset.active === 'true';
      updateTranslateUI(!active);
    });
    document.querySelectorAll('[data-voice-control]')?.forEach((button) => {
      button.addEventListener('click', () => previewVoice(button.dataset.voiceControl));
    });
    elements.saveButton?.addEventListener('click', saveSettings);
    if (elements.translateToggle) {
      updateTranslateUI(elements.translateToggle.dataset.active === 'true');
    }
  }

  function updateTranslateUI(active) {
    if (!elements.translateToggle || !elements.targetGroup) return;
    elements.translateToggle.dataset.active = active ? 'true' : 'false';
    elements.targetGroup.toggleAttribute('hidden', !active);
  }

  async function previewVoice(kind) {
    const voice = kind === 'male' ? elements.maleVoice?.value : elements.femaleVoice?.value;
    if (!voice) return;
    const text = config.sampleText || 'This is a short sample to preview the selected voice.';
    try {
      const response = await fetch(config.ttsUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ gender: kind, voice, text }),
      });
      if (!response.ok) {
        const payload = await response.json().catch(() => ({}));
        const message = payload?.error?.message || 'Preview failed.';
        throw new Error(message);
      }
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      elements.previewAudio.src = url;
      elements.previewAudio.play().catch(() => {});
      elements.previewAudio.addEventListener(
        'ended',
        () => {
          URL.revokeObjectURL(url);
        },
        { once: true }
      );
    } catch (err) {
      alert(err.message || 'Unable to play preview.');
    }
  }

  async function saveSettings() {
    if (!elements.saveButton) return;
    elements.saveButton.disabled = true;
    const originalText = elements.saveButton.textContent;
    elements.saveButton.textContent = 'Saving...';

    const translateActive = elements.translateToggle?.dataset.active === 'true';
    const payload = {
      default_language: normaliseValue(elements.defaultLanguage?.value),
      translate_by_default: translateActive,
      default_target_language: translateActive ? normaliseValue(elements.defaultTarget?.value) : null,
      female_voice: elements.femaleVoice?.value || null,
      male_voice: elements.maleVoice?.value || null,
    };

    try {
      const response = await fetch(config.updateUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      if (!response.ok) {
        const payload = await response.json().catch(() => ({}));
        const message = payload?.error?.message || 'Failed to save settings.';
        throw new Error(message);
      }
      window.location.href = config.redirectUrl || '/';
    } catch (err) {
      alert(err.message || 'Failed to save settings.');
    } finally {
      elements.saveButton.disabled = false;
      elements.saveButton.textContent = originalText;
    }
  }

  function normaliseValue(value) {
    if (!value || value === 'default') return null;
    return value;
  }

  init();
})();
