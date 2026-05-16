"""Audio conversion helpers for OpenAI realtime PCM streams."""

from __future__ import annotations

import base64

import numpy as np

TARGET_SAMPLE_RATE = 24_000
DEFAULT_CHUNK_MS = 50


def normalize_audio(samples: np.ndarray, sample_rate: int, target_rate: int = TARGET_SAMPLE_RATE) -> np.ndarray:
    """Downmix audio to mono float32 and resample to ``target_rate``."""

    audio = np.asarray(samples, dtype=np.float32)
    if audio.ndim == 2:
        audio = audio.mean(axis=1)
    elif audio.ndim > 2:
        raise ValueError("Audio samples must be one or two dimensional")
    if sample_rate <= 0:
        raise ValueError("sample_rate must be positive")
    if audio.size == 0 or sample_rate == target_rate:
        return audio.astype(np.float32, copy=False)

    duration = audio.size / float(sample_rate)
    target_size = max(1, int(round(duration * target_rate)))
    source_x = np.linspace(0.0, duration, num=audio.size, endpoint=False)
    target_x = np.linspace(0.0, duration, num=target_size, endpoint=False)
    return np.interp(target_x, source_x, audio).astype(np.float32)


def float_samples_to_pcm16(samples: np.ndarray) -> bytes:
    """Convert mono float samples in [-1, 1] to signed little-endian PCM16."""

    audio = np.asarray(samples, dtype=np.float32)
    clipped = np.clip(audio, -1.0, 1.0)
    scaled = np.where(clipped < 0.0, clipped * 32768.0, clipped * 32767.0)
    return scaled.astype("<i2").tobytes()


def chunk_pcm16(pcm: bytes, sample_rate: int = TARGET_SAMPLE_RATE, chunk_ms: int = DEFAULT_CHUNK_MS) -> list[bytes]:
    """Split PCM16 into fixed duration chunks."""

    bytes_per_sample = 2
    chunk_size = int(sample_rate * chunk_ms / 1000) * bytes_per_sample
    chunk_size -= chunk_size % bytes_per_sample
    return [pcm[index : index + chunk_size] for index in range(0, len(pcm), chunk_size) if pcm[index : index + chunk_size]]


def base64_pcm16(pcm: bytes) -> str:
    return base64.b64encode(pcm).decode("ascii")
