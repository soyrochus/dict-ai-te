from __future__ import annotations

import asyncio
import base64

import numpy as np

from dictaite_core.config import Settings
from dictaite_core.realtime import (
    LiveMode,
    NormalizedEvent,
    OpenAIRealtimeClient,
    RealtimeClientConfig,
    RealtimeEventType,
    TranscriptAssembler,
    base64_pcm16,
    bridge_websocket_messages,
    chunk_pcm16,
    float_samples_to_pcm16,
    normalize_audio,
    parse_realtime_event,
    realtime_settings_from_legacy,
)

from dictaite_core.realtime.transport import (
    RealtimeClientError,
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


def test_transcript_assembly_replaces_partial_without_duplicate():
    assembler = TranscriptAssembler()
    assembler.add_delta("Hel", "a")
    assembler.add_delta("lo", "a")
    assembler.complete("Hello.", "a")
    assembler.add_delta("Second", "b")
    assert assembler.text == "Hello. Second"


def test_realtime_settings_migration():
    settings = realtime_settings_from_legacy(
        Settings(default_language="de", translate_by_default=True, default_target_language="es")
    )
    assert settings.live_translation_enabled is True
    assert settings.mode == LiveMode.TRANSLATE
    assert settings.source_language == "de"
    assert settings.target_language == "es"


def test_websocket_bridge_normalizes_mocked_realtime_traffic():
    class FakeClient:
        async def run(self, audio_chunks, on_event):
            chunks = []
            async for chunk in audio_chunks:
                chunks.append(chunk)
            assert chunks == ["abc"]
            await on_event(NormalizedEvent(RealtimeEventType.SOURCE_DELTA, text="Hi", item_id="1"))

    async def incoming():
        yield {"type": "audio", "audio": "abc"}
        yield {"type": "stop"}

    sent = []

    async def send(payload):
        sent.append(payload)

    asyncio.run(bridge_websocket_messages(incoming(), send, FakeClient()))
    assert sent == [{"type": "source_delta", "text": "Hi", "item_id": "1", "state": None, "error": None}]


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

    payload = client._session_update()

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

    payload = client._session_update()

    assert "language" not in payload["session"]["audio"]["input"]["transcription"]
    assert _normalize_optional_language(None) is None
    assert _normalize_optional_language("") is None
    assert _normalize_optional_language("default") is None
    assert _normalize_optional_language(" es ") == "es"


def test_realtime_translation_reports_unavailable_before_connect(monkeypatch):
    async def no_audio():
        if False:
            yield ""

    async def emit(_event):
        return None

    def fail_connect(*_args, **_kwargs):
        raise AssertionError("translation mode should fail before websocket connect")

    monkeypatch.setenv("OPENAI_API_KEY", "test-key")
    monkeypatch.setattr("websockets.connect", fail_connect)
    client = OpenAIRealtimeClient(
        RealtimeClientConfig(mode=LiveMode.TRANSLATE, target_language="es")
    )

    try:
        asyncio.run(client.run(no_audio(), emit))
    except RealtimeClientError as exc:
        assert "Live translation is unavailable" in str(exc)
    else:
        raise AssertionError("expected unavailable translation error")


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
