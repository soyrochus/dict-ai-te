from __future__ import annotations

import io

def test_transcribe_endpoint_returns_text(monkeypatch, client, wav_bytes):
    monkeypatch.setattr("dictaite.ui_web.app.transcribe", lambda audio, mimetype, language: "Hello world")
    monkeypatch.setattr("dictaite.ui_web.app.translate", lambda text, target: f"Translated {target}")

    data = {
        'audio': (io.BytesIO(wav_bytes), 'sample.wav'),
        'language': 'en',
        'translate': 'true',
        'target_lang': 'es',
    }
    response = client.post('/api/transcribe', data=data, content_type='multipart/form-data')
    assert response.status_code == 200
    payload = response.get_json()
    assert payload['text'] == 'Hello world'
    assert payload['translatedText'] == 'Translated es'
    assert payload['durationMs'] >= 0


def test_settings_roundtrip(client):
    with client.application.app_context():
        from dictaite_core import load_settings

        original_voice = load_settings().female_voice

    response = client.post('/api/settings', json={'female_voice': 'nova', 'translate_by_default': True})
    assert response.status_code == 200
    updated = response.get_json()
    assert updated['female_voice'] == 'nova'
    assert updated['translate_by_default'] is True

    response = client.get('/api/settings')
    assert response.status_code == 200
    fetched = response.get_json()
    assert fetched['female_voice'] == 'nova'

    # revert to avoid side effects
    client.post('/api/settings', json={'female_voice': original_voice, 'translate_by_default': False})
