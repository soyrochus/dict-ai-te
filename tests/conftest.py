import io
import numpy as np
import pytest
import soundfile as sf

from dictaite.ui_web.app import create_app


@pytest.fixture()
def wav_bytes() -> bytes:
    duration = 0.25
    samplerate = 16000
    t = np.linspace(0, duration, int(duration * samplerate), endpoint=False)
    tone = 0.1 * np.sin(2 * np.pi * 440 * t)
    buffer = io.BytesIO()
    sf.write(buffer, tone, samplerate, format='WAV')
    return buffer.getvalue()


@pytest.fixture()
def flask_app(monkeypatch, tmp_path):
    monkeypatch.setenv("DICTAITE_HOME", str(tmp_path / "cfg"))
    app = create_app({"TESTING": True})
    yield app


@pytest.fixture()
def client(flask_app):
    return flask_app.test_client()
