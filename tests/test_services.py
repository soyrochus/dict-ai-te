from __future__ import annotations

import types

from dictaite_core.services import transcribe, translate


class DummyTranscriptionResponse:
    def __init__(self, text: str) -> None:
        self.text = text


class DummyChatMessage:
    def __init__(self, content: str) -> None:
        self.content = content


class DummyChoice:
    def __init__(self, content: str) -> None:
        self.message = DummyChatMessage(content)
        self.index = 0


class DummyChatResponse:
    def __init__(self, content: str) -> None:
        self.choices = [DummyChoice(content)]


def test_transcribe_uses_openai(monkeypatch, wav_bytes):
    def fake_client():
        audio = types.SimpleNamespace()
        audio.transcriptions = types.SimpleNamespace(create=lambda **_: DummyTranscriptionResponse("hello world"))
        return types.SimpleNamespace(audio=audio)

    monkeypatch.setattr("dictaite_core.services.stt.get_openai_client", fake_client)
    result = transcribe(wav_bytes, "audio/wav", "en")
    assert "hello" in result


def test_translate_uses_openai(monkeypatch):
    def fake_client():
        chat = types.SimpleNamespace(completions=types.SimpleNamespace(create=lambda **_: DummyChatResponse("hola mundo")))
        return types.SimpleNamespace(chat=chat)

    monkeypatch.setattr("dictaite_core.services.translate.get_openai_client", fake_client)
    result = translate("Hello world", "Spanish")
    assert result == "hola mundo"
