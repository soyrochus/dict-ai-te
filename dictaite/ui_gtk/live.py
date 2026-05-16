"""GTK realtime session adapter."""

from __future__ import annotations

import asyncio
import logging
import queue
import threading
from collections.abc import Callable

import numpy as np
import sounddevice as sd
from gi.repository import GLib

from dictaite_core.realtime import (
    LiveMode,
    NormalizedEvent,
    OpenAIRealtimeClient,
    RealtimeClientConfig,
    base64_pcm16,
    float_samples_to_pcm16,
    normalize_audio,
)

LOGGER = logging.getLogger(__name__)


class GtkLiveSession:
    """Capture microphone audio and feed a realtime client from a worker loop."""

    def __init__(
        self,
        mode: LiveMode,
        source_language: str | None,
        target_language: str | None,
        on_event: Callable[[NormalizedEvent], None],
        on_error: Callable[[str], None],
    ) -> None:
        self.mode = mode
        self.source_language = source_language
        self.target_language = target_language
        self.on_event = on_event
        self.on_error = on_error
        self._queue: queue.Queue[str | None] = queue.Queue(maxsize=8)
        self._stream: sd.InputStream | None = None
        self._thread: threading.Thread | None = None
        self._running = False

    def start(self) -> None:
        if self._running:
            return
        self._running = True
        self._stream = sd.InputStream(samplerate=24_000, channels=1, callback=self._on_audio)
        self._stream.start()
        self._thread = threading.Thread(target=self._run_loop, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._running = False
        if self._stream:
            self._stream.stop()
            self._stream.close()
        self._stream = None
        self._enqueue(None)

    def _on_audio(self, indata, _frames, time_info, status) -> None:
        if status:
            LOGGER.warning("Audio callback status: %s", status)
        mono = normalize_audio(np.asarray(indata, dtype=np.float32), 24_000)
        self._enqueue(base64_pcm16(float_samples_to_pcm16(mono)))

    def _enqueue(self, item: str | None) -> None:
        try:
            self._queue.put_nowait(item)
        except queue.Full:
            if item is None:
                return
            LOGGER.debug("Dropping realtime audio chunk because GTK queue is full")

    def _run_loop(self) -> None:
        asyncio.run(self._run_client())

    async def _run_client(self) -> None:
        client = OpenAIRealtimeClient(
            RealtimeClientConfig(
                mode=self.mode,
                source_language=self.source_language,
                target_language=self.target_language,
            )
        )

        async def chunks():
            while True:
                item = await asyncio.to_thread(self._queue.get)
                if item is None:
                    break
                yield item

        async def emit(event: NormalizedEvent) -> None:
            GLib.idle_add(self.on_event, event)

        try:
            await client.run(chunks(), emit)
        except Exception as exc:  # pragma: no cover - runtime network path
            LOGGER.exception("GTK realtime session failed")
            GLib.idle_add(self.on_error, str(exc))
