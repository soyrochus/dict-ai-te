from __future__ import annotations

import asyncio
import base64
import json
import struct

import numpy as np

from dictaite_core.realtime import (
    LiveMode,
    OpenAIRealtimeClient,
    RealtimeClientConfig,
    RealtimeEventType,
    TranscriptAssembler,
    base64_pcm16,
    chunk_pcm16,
    float_samples_to_pcm16,
    normalize_audio,
    parse_realtime_event,
)

from dictaite_core.realtime.transport import (
    RealtimeClientError,
    TRANSLATION_MODEL,
    _normalize_optional_language,
    _websocket_header_kwargs,
)


def test_pcm_conversion_chunking_and_base64():
    stereo = np.array([[1.0, -1.0], [0.5, 0.5]], dtype=np.float32)
    mono = normalize_audio(stereo, 48_000, target_rate=24_000)
    assert mono.ndim == 1
    pcm = float_samples_to_pcm16(mono)
    assert len(pcm) % 2 == 0
    chunks = chunk_pcm16(pcm, chunk_ms=20)
    assert chunks
    assert base64.b64decode(base64_pcm16(chunks[0]))
    assert struct.unpack("<hh", float_samples_to_pcm16(np.array([-1.0, 1.0], dtype=np.float32))) == (-32768, 32767)


def test_event_parsing_and_unknown_safety():
    event = parse_realtime_event(
        {
            "type": "conversation.item.input_audio_transcription.delta",
            "item_id": "item-1",
            "delta": "Hello",
        }
    )
    assert event.type == RealtimeEventType.SOURCE_DELTA
    assert event.item_id == "item-1"
    assert parse_realtime_event({"type": "future.event"}).type == RealtimeEventType.UNKNOWN
    assert parse_realtime_event({"type": "session.output_audio.delta", "delta": "secret"}).type == RealtimeEventType.TRANSLATED_AUDIO_DELTA
    assert parse_realtime_event({"type": "response.audio.delta"}).type == RealtimeEventType.TRANSLATED_AUDIO_DELTA
    assert parse_realtime_event({"type": "response.output_audio.delta"}).type == RealtimeEventType.TRANSLATED_AUDIO_DELTA
    assert parse_realtime_event({"type": "response.output_text.delta", "delta": "Hola"}).type == RealtimeEventType.TRANSLATION_DELTA
    assert parse_realtime_event({"type": "response.output_audio_transcript.delta", "delta": "Hola"}).type == RealtimeEventType.TRANSLATION_DELTA
    assert parse_realtime_event({"type": "response.completed"}).type == RealtimeEventType.SESSION_STATE
    assert parse_realtime_event({"type": "session.anything.new"}).type == RealtimeEventType.SESSION_STATE


def test_transcript_assembly_replaces_partial_without_duplicate():
    assembler = TranscriptAssembler()
    assembler.add_delta("Hel", "a")
    assembler.add_delta("lo", "a")
    assembler.complete("Hello.", "a")
    assembler.add_delta("Second", "b")
    assert assembler.text == "Hello. Second"


def test_transcript_assembly_keeps_repeated_anonymous_completions():
    assembler = TranscriptAssembler()
    assembler.complete("Again.", None)
    assembler.complete("Again.", None)

    assert assembler.text == "Again. Again."


def test_realtime_client_loads_dotenv_for_api_key(tmp_path, monkeypatch):
    env_file = tmp_path / ".env"
    env_file.write_text("OPENAI_API_KEY=from-dotenv\n", encoding="utf-8")
    monkeypatch.chdir(tmp_path)
    monkeypatch.delenv("OPENAI_API_KEY", raising=False)

    config = RealtimeClientConfig(mode=LiveMode.TRANSCRIBE)

    assert config.resolved_api_key == "from-dotenv"


def test_realtime_client_omits_legacy_beta_header(monkeypatch):
    captured = {}

    class FakeWebSocket:
        async def __aenter__(self):
            return self

        async def __aexit__(self, exc_type, exc, tb):
            return False

        async def send(self, _message):
            return None

        def __aiter__(self):
            return self

        async def __anext__(self):
            raise StopAsyncIteration

    def fake_connect(url, *, additional_headers):
        captured["url"] = url
        captured["headers"] = additional_headers
        return FakeWebSocket()

    async def no_audio():
        if False:
            yield ""

    async def emit(_event):
        return None

    monkeypatch.setenv("OPENAI_API_KEY", "test-key")
    monkeypatch.setattr("websockets.connect", fake_connect)

    client = OpenAIRealtimeClient(RealtimeClientConfig(mode=LiveMode.TRANSCRIBE))
    asyncio.run(client.run(no_audio(), emit))

    assert captured["headers"] == {"Authorization": "Bearer test-key"}
    assert "intent=transcription" in captured["url"]


def test_realtime_client_uses_ga_transcription_session_payload():
    client = OpenAIRealtimeClient(
        RealtimeClientConfig(mode=LiveMode.TRANSCRIBE, source_language="de")
    )

    payload = client._transcription_session_update()

    assert payload["type"] == "session.update"
    assert payload["session"]["type"] == "transcription"
    assert payload["session"]["audio"]["input"]["format"] == {
        "type": "audio/pcm",
        "rate": 24000,
    }
    assert payload["session"]["audio"]["input"]["transcription"]["model"] == "gpt-4o-transcribe"
    assert payload["session"]["audio"]["input"]["transcription"]["language"] == "de"


def test_realtime_client_omits_default_auto_detect_language():
    client = OpenAIRealtimeClient(
        RealtimeClientConfig(mode=LiveMode.TRANSCRIBE, source_language="default")
    )

    payload = client._transcription_session_update()

    assert "language" not in payload["session"]["audio"]["input"]["transcription"]
    assert _normalize_optional_language(None) is None
    assert _normalize_optional_language("") is None
    assert _normalize_optional_language("default") is None
    assert _normalize_optional_language(" es ") == "es"


def test_realtime_translation_session_payload():
    client = OpenAIRealtimeClient(
        RealtimeClientConfig(mode=LiveMode.TRANSLATE, target_language="French", source_language="es")
    )

    payload = client._translation_session_update()

    assert payload["session"]["type"] == "realtime"
    assert payload["session"]["model"] == TRANSLATION_MODEL
    assert payload["session"]["output_modalities"] == ["text"]
    assert "French" in payload["session"]["instructions"]
    audio = payload["session"]["audio"]["input"]
    assert audio["transcription"]["language"] == "es"
    assert audio["turn_detection"]["create_response"] is True
    assert audio["turn_detection"]["interrupt_response"] is True


def test_realtime_client_emits_lifecycle_events(monkeypatch):
    sent = []

    class FakeWebSocket:
        async def __aenter__(self):
            return self

        async def __aexit__(self, exc_type, exc, tb):
            return False

        async def send(self, message):
            sent.append(message)

        def __aiter__(self):
            return self

        async def __anext__(self):
            raise StopAsyncIteration

    def fake_connect(_url, *, additional_headers):
        return FakeWebSocket()

    async def no_audio():
        if False:
            yield ""

    events = []

    async def emit(event):
        events.append(event)

    monkeypatch.setenv("OPENAI_API_KEY", "test-key")
    monkeypatch.setattr("websockets.connect", fake_connect)

    client = OpenAIRealtimeClient(RealtimeClientConfig(mode=LiveMode.TRANSLATE, target_language="German"))
    asyncio.run(client.run(no_audio(), emit))

    assert json_message_types(sent) == ["session.update", "input_audio_buffer.commit"]
    assert [(event.type, event.state) for event in events] == [
        (RealtimeEventType.SESSION_STATE, "connecting"),
        (RealtimeEventType.SESSION_STATE, "disconnected"),
    ]


def test_websocket_header_kwargs_supports_old_and_new_websockets():
    def new_connect(uri, *, additional_headers=None):
        return None

    def old_connect(uri, *, extra_headers=None):
        return None

    headers = {"Authorization": "Bearer test"}

    assert _websocket_header_kwargs(new_connect, headers) == {"additional_headers": headers}
    assert _websocket_header_kwargs(old_connect, headers) == {"extra_headers": headers}


def test_realtime_client_reports_missing_api_key(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    monkeypatch.delenv("OPENAI_API_KEY", raising=False)
    config = RealtimeClientConfig(mode=LiveMode.TRANSCRIBE)

    try:
        _ = config.resolved_api_key
    except RealtimeClientError as exc:
        assert "OPENAI_API_KEY" in str(exc)
    else:
        raise AssertionError("expected missing key error")


def json_message_types(messages):
    return [json.loads(message)["type"] for message in messages]
