## MODIFIED Requirements

### Requirement: PCM16 conversion uses asymmetric negative/positive multipliers
The `float_samples_to_pcm16` function SHALL use `32768.0` as the multiplier for negative samples and `32767.0` for positive samples (matching the Rust `pcm16_le` implementation). The current implementation using a uniform `32767.0` multiplier for all samples is replaced.

#### Scenario: Negative full-scale sample converts to minimum i16
- **WHEN** `float_samples_to_pcm16` is called with a sample value of `-1.0`
- **THEN** the output bytes represent the signed i16 value `-32768`

#### Scenario: Positive full-scale sample converts to maximum i16
- **WHEN** `float_samples_to_pcm16` is called with a sample value of `1.0`
- **THEN** the output bytes represent the signed i16 value `32767`

### Requirement: Transcription session emits lifecycle state events
The `OpenAIRealtimeClient.run` method SHALL emit a `SESSION_STATE` event with `state="connecting"` immediately after sending the session update, and a `SESSION_STATE` event with `state="disconnected"` after the WebSocket closes. This matches the Rust `run_verified_transcription_session` lifecycle behaviour.

#### Scenario: Connecting event emitted on session start
- **WHEN** a transcription session is started and the session update is sent
- **THEN** `on_event` is called with a `SESSION_STATE` event where `state="connecting"` before any audio or transcript events arrive

#### Scenario: Disconnected event emitted on session end
- **WHEN** the transcription session ends (audio exhausted or stop requested)
- **THEN** `on_event` is called with a `SESSION_STATE` event where `state="disconnected"`

## REMOVED Requirements

### Requirement: Batch transcription pipeline (GTK)
**Reason**: The Rust app has no batch-record-then-transcribe path. Recording always connects live to OpenAI Realtime. The `transcribe_audio`, `prepare_wav`, `stt.transcribe` call chain in the GTK app is removed.
**Migration**: Use the live transcription session (WebSocket) instead. There is no REST-based transcription replacement.

### Requirement: Batch transcription REST endpoints (Flask)
**Reason**: The Rust app has no equivalent. The `/api/transcribe` (single-upload) and `/api/record/*` (chunked-session) endpoints are removed.
**Migration**: Use the WebSocket live transcription endpoint `/ws/live/transcribe` instead.

### Requirement: Post-hoc chat-based translation service
**Reason**: The Rust app does not use a chat-completions call for translation. Live translation uses the Realtime API translation endpoint only.
**Migration**: Use `/ws/live/translate` WebSocket endpoint for live translation.

### Requirement: Anonymous transcript completion deduplication
**Reason**: The Rust `TranscriptAssembler.complete` does not deduplicate anonymous segment completions. Python parity requires the same behaviour.
**Migration**: None — this is an internal assembler detail with no user-visible API.
