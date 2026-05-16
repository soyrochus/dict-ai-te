## 1. Core library — events

- [x] 1.1 In `dictaite_core/realtime/events.py`, add `"response.output_text.delta"` and `"response.output_audio_transcript.delta"` as additional patterns that return `TRANSLATION_DELTA`
- [x] 1.2 Add `"response.audio.delta"` and `"response.output_audio.delta"` as additional patterns that return `TRANSLATED_AUDIO_DELTA`
- [x] 1.3 Replace the explicit `SESSION_STATE` allow-list `{"session.created", "session.updated", "response.created", "response.done"}` with a catch-all: any `event_type` starting with `"session."` or `"response."` maps to `SESSION_STATE`
- [x] 1.4 Update tests in `tests/test_realtime.py` to cover the new event type mappings and the broadened session-state catch-all

## 2. Core library — audio

- [x] 2.1 In `dictaite_core/realtime/audio.py`, update `float_samples_to_pcm16` to use `32768.0` for negative samples and `32767.0` for positive samples (asymmetric, matching Rust `pcm16_le`)
- [x] 2.2 Remove the `chunks_as_base64` helper function from `audio.py` (not present in Rust; verify no callers remain)
- [x] 2.3 Update `chunk_pcm16` to remove the `chunk_ms <= 0` guard and the `max(bytes_per_sample, ...)` minimum-chunk-size guard, aligning to the Rust implementation

## 3. Core library — transcript assembler

- [x] 3.1 In `dictaite_core/realtime/transcript.py`, remove the `if text not in self._anonymous` deduplication check in `TranscriptAssembler.complete`; replace with an unconditional `self._anonymous.append(text)` to match Rust

## 4. Core library — transport (transcription session)

- [x] 4.1 In `dictaite_core/realtime/transport.py`, after sending the session update in the transcription path, emit a `NormalizedEvent(SESSION_STATE, state="connecting")` via `on_event`
- [x] 4.2 After the WebSocket connection closes (end of `_receive_events` or equivalent), emit a `NormalizedEvent(SESSION_STATE, state="disconnected")` via `on_event`

## 5. Core library — transport (translation session)

- [x] 5.1 Add constants `TRANSLATION_URL = "wss://api.openai.com/v1/realtime?model=gpt-realtime"` and `TRANSLATION_MODEL = "gpt-realtime"` to `transport.py`
- [x] 5.2 Implement `run_live_translation` (async function or method) that connects to `TRANSLATION_URL`, sends a `session.update` with `type: "realtime"`, `model: TRANSLATION_MODEL`, `output_modalities: ["text"]`, and a translation instruction string built from `target_language`; include VAD settings with `create_response: true` and `interrupt_response: true`
- [x] 5.3 Wire optional `source_language` into the translation session update's `transcription.language` field (same pattern as the transcription session)
- [x] 5.4 Emit `SESSION_STATE` `"connecting"` and `"disconnected"` lifecycle events in the translation session path
- [x] 5.5 Update `OpenAIRealtimeClient.run` to route `LiveMode.TRANSLATE` to `run_live_translation` instead of raising `RealtimeClientError`
- [x] 5.6 Remove the `bridge_websocket_messages` function from `transport.py` (it will be inlined into the Flask handler in a later task)

## 6. Core library — config and settings

- [x] 6.1 In `dictaite_core/config.py`, change `Settings` defaults to `female_voice = "nova"` and `male_voice = "onyx"` (lowercase)
- [x] 6.2 Add a `fill_defaults` normalisation step in `load_settings`: lowercase and strip both voice fields; replace blank voice strings with the defaults
- [x] 6.3 Delete `dictaite_core/realtime/models.py` (LiveMode and RealtimeSettings); update all imports to use `LiveMode` from a single location or inline as a plain bool/string
- [x] 6.4 Delete `dictaite_core/realtime/settings.py` (`realtime_settings_from_legacy`); update any callers

## 7. Core library — remove batch services

- [x] 7.1 Delete `dictaite_core/services/stt.py` entirely
- [x] 7.2 Delete `dictaite_core/services/translate.py` entirely
- [x] 7.3 Update `dictaite_core/services/__init__.py` to remove exports of `transcribe`, `prepare_wav`, `translate`, `TranscriptionError`, `ALLOWED_MIME_TYPES`, `MAX_AUDIO_DURATION_SECONDS`
- [x] 7.4 Remove `pydub` and `soundfile` from `pyproject.toml` dependencies if they are no longer used anywhere (verify by grepping all remaining source files)

## 8. GTK UI — remove batch path

- [x] 8.1 Delete the `DictaiTeWindow.transcribe_audio` method and its `threading.Thread` call site in `stop_recording` (the batch-record-then-transcribe flow)
- [x] 8.2 Remove the `audio_callback`, `audio_frames`, `stream`, and all related batch-audio bookkeeping from `DictaiTeWindow.__init__` and `start_recording`
- [x] 8.3 Remove any remaining imports of `transcribe`, `translate`, `prepare_wav`, `TranscriptionError`, `ALLOWED_MIME_TYPES`, `MAX_AUDIO_DURATION_SECONDS` from `dictaite/ui_gtk/app.py`
- [x] 8.4 Remove all `sounddevice` and `soundfile` and `numpy` usage from the GTK UI that was serving the old batch-record path (keep only what the live session and TTS playback paths need)

## 9. GTK UI — live session alignment

- [x] 9.1 Update `GtkLiveSession` in `dictaite/ui_gtk/live.py` to correctly route `LiveMode.TRANSLATE` through `run_live_translation` (the updated transport); remove the error-raise guard
- [x] 9.2 Update `DictaiTeWindow.on_live_event` to handle `SESSION_STATE` events: show "Connected to live session" on `"connecting"` / `"session.created"` / `"session.updated"`, show "Disconnected" on `"disconnected"`, and update recording state accordingly
- [x] 9.3 Update `DictaiTeWindow.on_live_event` `TRANSLATION_DELTA` handler to accumulate text in `self.translated_transcript` string (not directly into a GTK buffer), matching Rust's string-accumulation pattern
- [x] 9.4 Verify that the GTK UI status label transitions match the Rust status text (`"Listening live..."`, `"Translating live to <lang>"`, `"Stopped"`, `"Connected to live session"`, `"Disconnected"`)

## 10. Flask/web UI — remove batch endpoints

- [x] 10.1 Delete the `api_transcribe` route and handler (`@API.post("/transcribe")`) from `dictaite/ui_web/app.py`
- [x] 10.2 Delete `api_record_start`, `api_record_append`, `api_record_finalize`, and `api_record_cancel` routes and handlers
- [x] 10.3 Delete the `RecordingSession` dataclass and `RECORDING_SESSIONS` / `RECORDING_LOCK` globals
- [x] 10.4 Delete all helper functions that only served the batch path: `_prepare_wav_and_duration`, `_run_transcription`, `_parse_bool`, `_cleanup_recording_file`, `_duration_seconds` (confirm no other callers)
- [x] 10.5 Remove all batch-service imports from `ui_web/app.py`: `ALLOWED_MIME_TYPES`, `MAX_AUDIO_DURATION_SECONDS`, `TranscriptionError`, `prepare_wav`, `transcribe`, `translate`
- [x] 10.6 Remove the `RECORDINGS_DIR` constant and the `tmp/recordings` directory creation

## 11. Flask/web UI — live WebSocket alignment

- [x] 11.1 Remove the import of `bridge_websocket_messages` from `dictaite_core.realtime` in `ui_web/app.py`
- [x] 11.2 Rewrite `_run_live_socket` to call the realtime client directly (without `bridge_websocket_messages`), forwarding events as JSON over the WebSocket, matching the event field structure the Rust app emits: `type`, `text`, `item_id`, `state`, `error`
- [x] 11.3 Ensure `/ws/live/translate` uses the updated `LiveMode.TRANSLATE` path (which now works via `run_live_translation` in transport)
- [x] 11.4 Update any web template JavaScript that consumed batch endpoints to remove those interactions (or remove the dead JS if no web client code references it)

## 12. Tests and cleanup

- [x] 12.1 Update `tests/test_realtime.py` to remove tests that cover the old batch path or `bridge_websocket_messages`
- [x] 12.2 Update `tests/test_services.py` to remove tests for deleted `stt.py` and `translate.py`
- [x] 12.3 Update `tests/test_api.py` to remove tests for deleted batch REST endpoints
- [x] 12.4 Add or update tests for the new translation session lifecycle (connecting/disconnected events) and the new GA event type mappings
- [x] 12.5 Run the full test suite and confirm all remaining tests pass
- [ ] 12.6 Run `python -m dictaite --gtk` (or equivalent) and manually verify: live transcription works, live translation works, TTS playback works, settings dialog works, save/copy transcript works
