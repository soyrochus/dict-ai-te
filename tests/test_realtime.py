from __future__ import annotations

import asyncio
import base64

import numpy as np

from dictaite_core.config import Settings
from dictaite_core.realtime import (
    LiveMode,
    NormalizedEvent,
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
