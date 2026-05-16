## Verification Notes

Date: 2026-05-16
Environment: Codex workspace, non-interactive shell, no GUI/microphone/manual OpenAI session available.

## API Contract

- Verified official OpenAI Realtime transcription documentation for:
  - WebSocket transcription sessions.
  - GA authentication with `Authorization` only.
  - No `OpenAI-Beta: realtime=v1` header on GA WebSocket connections.
  - `session.update`.
  - 24 kHz mono PCM input format.
  - `input_audio_buffer.append`.
  - `conversation.item.input_audio_transcription.delta`.
  - `conversation.item.input_audio_transcription.completed`.
  - Current documented transcription models including `gpt-4o-transcribe`.
- No official documentation was found for:
  - `gpt-realtime-translate`.
  - `/v1/realtime/translations`.
  - `translation_session.update`.
- Rust live translation is therefore implemented as an explicit unavailable error and does not fall back to chained text translation.

Update after local runtime test: the retired beta API returns "The Realtime beta api is no longer supported. Please use /v1/realtime for the GA API" when `OpenAI-Beta: realtime=v1` is sent. The Rust transport now omits that legacy header.

## Automated Verification

- `cargo test`: passed, 9 tests.
- Standard automated tests do not call OpenAI and do not require microphone hardware.

## Manual Acceptance

- Rust live transcription: blocked in this environment; requires GUI, microphone, valid API key, and network.
- Rust live translation: blocked by API contract verification; no current official realtime translation contract was found.
- Repeated stop/start: blocked in this environment; requires GUI and microphone.
- Microphone unavailable/denied: blocked in this environment; requires GUI/microphone permission control.
- Invalid or missing API key: blocked in this environment; requires GUI run.
- Network/WebSocket disconnect: blocked in this environment; requires GUI run and network manipulation.
