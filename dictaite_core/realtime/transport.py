"""Async OpenAI realtime WebSocket transport."""

from __future__ import annotations

import asyncio
import inspect
import json
import logging
import os
from collections.abc import AsyncIterator, Awaitable, Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from dotenv import load_dotenv

from .events import NormalizedEvent, RealtimeEventType, parse_realtime_event
from .models import LiveMode

LOGGER = logging.getLogger(__name__)

TRANSCRIPTION_MODEL = "gpt-4o-transcribe"
TRANSCRIPTION_URL = "wss://api.openai.com/v1/realtime?intent=transcription"


class RealtimeClientError(RuntimeError):
    pass


@dataclass(frozen=True, slots=True)
class RealtimeClientConfig:
    mode: LiveMode
    api_key: str | None = None
    target_language: str | None = None
    source_language: str | None = None

    @property
    def resolved_api_key(self) -> str:
        load_dotenv(dotenv_path=Path.cwd() / ".env")
        api_key = self.api_key or os.environ.get("OPENAI_API_KEY")
        if not api_key:
            raise RealtimeClientError("OPENAI_API_KEY not configured")
        return api_key


class OpenAIRealtimeClient:
    """Small async client around the OpenAI realtime WebSocket APIs."""

    def __init__(self, config: RealtimeClientConfig) -> None:
        self.config = config

    async def run(
        self,
        audio_chunks: AsyncIterator[str],
        on_event: Callable[[NormalizedEvent], Awaitable[None]],
    ) -> None:
        try:
            import websockets
        except ImportError as exc:  # pragma: no cover - dependency guard
            raise RealtimeClientError("Install websockets to use realtime sessions") from exc

        if self.config.mode == LiveMode.TRANSLATE:
            raise RealtimeClientError(
                "Live translation is unavailable: no current official OpenAI Realtime "
                "translation endpoint/model contract was verified."
            )

        url = TRANSCRIPTION_URL
        headers = {
            "Authorization": f"Bearer {self.config.resolved_api_key}",
        }
        connect_kwargs = _websocket_header_kwargs(websockets.connect, headers)
        async with websockets.connect(url, **connect_kwargs) as websocket:
            await websocket.send(json.dumps(self._session_update()))
            sender = asyncio.create_task(self._send_audio(websocket, audio_chunks))
            receiver = asyncio.create_task(self._receive_events(websocket, on_event))
            done, pending = await asyncio.wait({sender, receiver}, return_when=asyncio.FIRST_EXCEPTION)
            for task in pending:
                task.cancel()
            for task in done:
                task.result()

    def _session_update(self) -> dict[str, Any]:
        session: dict[str, Any] = {
            "type": "session.update",
            "session": {
                "type": "transcription",
                "audio": {
                    "input": {
                        "format": {"type": "audio/pcm", "rate": 24000},
                        "transcription": {"model": TRANSCRIPTION_MODEL},
                        "turn_detection": {
                            "type": "server_vad",
                            "threshold": 0.5,
                            "prefix_padding_ms": 300,
                            "silence_duration_ms": 500,
                        },
                    }
                },
            },
        }
        source_language = _normalize_optional_language(self.config.source_language)
        if source_language:
            session["session"]["audio"]["input"]["transcription"]["language"] = source_language
        return session

    async def _send_audio(self, websocket: Any, audio_chunks: AsyncIterator[str]) -> None:
        async for chunk in audio_chunks:
            if chunk:
                await websocket.send(json.dumps({"type": "input_audio_buffer.append", "audio": chunk}))
        await websocket.send(json.dumps({"type": "input_audio_buffer.commit"}))

    async def _receive_events(
        self,
        websocket: Any,
        on_event: Callable[[NormalizedEvent], Awaitable[None]],
    ) -> None:
        async for message in websocket:
            try:
                payload = json.loads(message)
            except json.JSONDecodeError:
                LOGGER.info("Ignoring non-JSON realtime message")
                continue
            event = parse_realtime_event(payload)
            if event.type != RealtimeEventType.UNKNOWN:
                await on_event(event)


async def bridge_websocket_messages(
    incoming: AsyncIterator[dict[str, Any]],
    send_browser_event: Callable[[dict[str, Any]], Awaitable[None]],
    client: OpenAIRealtimeClient,
) -> None:
    """Proxy browser audio messages to OpenAI and normalized events back."""

    queue: asyncio.Queue[str | None] = asyncio.Queue(maxsize=8)

    async def audio_iter() -> AsyncIterator[str]:
        while True:
            item = await queue.get()
            if item is None:
                break
            yield item

    async def browser_reader() -> None:
        async for message in incoming:
            msg_type = message.get("type")
            if msg_type == "audio":
                audio = message.get("audio")
                if isinstance(audio, str) and audio:
                    await queue.put(audio)
            elif msg_type in {"stop", "close"}:
                break
        await queue.put(None)

    async def emit(event: NormalizedEvent) -> None:
        await send_browser_event(
            {
                "type": event.type.value,
                "text": event.text,
                "item_id": event.item_id,
                "state": event.state,
                "error": event.error,
            }
        )

    await asyncio.gather(browser_reader(), client.run(audio_iter(), emit))


def _websocket_header_kwargs(connect: Callable[..., Any], headers: dict[str, str]) -> dict[str, dict[str, str]]:
    """Return the header kwarg name supported by the installed websockets version."""

    parameters = inspect.signature(connect).parameters
    if "additional_headers" in parameters:
        return {"additional_headers": headers}
    return {"extra_headers": headers}


def _normalize_optional_language(value: str | None) -> str | None:
    if value is None:
        return None
    normalized = value.strip()
    if not normalized or normalized == "default":
        return None
    return normalized
