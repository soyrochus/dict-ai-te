## 1. Realtime API Contract Verification

- [x] 1.1 Verify current official OpenAI Realtime transcription endpoint, supported model names, session update payload shape, audio format fields, append event, transcript delta event, and transcript completion event.
- [x] 1.2 Verify current official OpenAI Realtime translation-capable endpoint, model name, target-language field, source-language support, transcript events, translated audio event, and close/commit semantics.
- [x] 1.3 Update Rust realtime constants, session payloads, event parsing, and architecture notes to match the verified API contract.
- [x] 1.4 If no verified realtime translation-capable path is available, document the blocker and make live translation report unavailable instead of falling back to chained text translation.

## 2. Rust Live Audio Pipeline

- [x] 2.1 Add a Rust live microphone capture path that starts and stops independently from the existing batch `Recorder::stop() -> AudioClip` flow.
- [x] 2.2 Push captured microphone samples or prepared audio units from the `cpal` callback into a bounded channel without network I/O or heavy processing in the callback.
- [x] 2.3 Implement worker-side downmixing, resampling to 24 kHz, PCM16 little-endian conversion, chunking, and base64 or byte encoding according to the verified API contract.
- [x] 2.4 Surface capture startup, overflow/drop, and microphone errors as UI-readable realtime errors.

## 3. Rust Realtime Transport and Session Control

- [x] 3.1 Implement or adapt `run_live_transcription` to start the verified transcription session, forward audio chunks, parse source transcript events, and send normalized UI events.
- [x] 3.2 Implement or adapt `run_live_translation` to start the verified translation session, forward audio chunks, parse source and translated transcript events, ignore translated audio deltas safely, and send normalized UI events.
- [x] 3.3 Add stop signaling so microphone capture, audio forwarding, and WebSocket tasks shut down cleanly on Stop.
- [x] 3.4 Ensure realtime transport errors avoid logging audio payloads, API keys, or other sensitive data.

## 4. Rust egui Live-First Integration

- [x] 4.1 Replace Rust egui post-recording transcription flow with Start Listening/Stop live session controls.
- [x] 4.2 Add Translate Live mode selection, target-language selection, source transcript pane, translated transcript pane when translation is enabled, and connection-state display.
- [x] 4.3 Poll realtime UI events from egui `App::update` and update state only on the UI thread.
- [x] 4.4 Preserve accumulated source and translated transcript edit, copy, save, clear, and text-to-speech playback workflows.
- [x] 4.5 Remove user-facing uploading, finalizing, or post-recording transcription statuses from the Rust live path.

## 5. Automated Tests

- [x] 5.1 Add Rust tests for live audio downmixing, resampling, PCM16 conversion, chunk sizing, and bounded-channel behavior where practical.
- [x] 5.2 Add Rust tests for verified realtime event parsing, unknown event safety, error event forwarding, translated audio delta ignoring, and no sensitive payload logging.
- [x] 5.3 Add Rust tests for transcript segment reconciliation, out-of-order completion handling, anonymous delta handling, and duplicate prevention.
- [x] 5.4 Add Rust tests for live state transitions across start, connected/transcribing/translating, stop, and error cases.
- [x] 5.5 Ensure standard automated tests do not call OpenAI or require microphone hardware.

## 6. Documentation and Cleanup

- [x] 6.1 Update architecture notes or implementation comments with the verified Realtime API contract and any differences from the older OpenSpec assumptions.
- [x] 6.2 Quarantine or remove obsolete Rust user-facing batch transcription code after live Rust paths are verified.
- [x] 6.3 Reconcile the original `realtime-transcription-translation` task list with this completion work so outstanding Rust/API/manual-test tasks are not double-counted or falsely marked complete.

## 7. Manual Acceptance

- [ ] 7.1 Manually test Rust live transcription with a valid API key and working microphone; record date, environment, and outcome.
- [ ] 7.2 Manually test Rust live translation with a selected target language; record date, environment, and outcome.
- [ ] 7.3 Manually test repeated stop/start; record date, environment, and outcome.
- [ ] 7.4 Manually test microphone unavailable or denied; record date, environment, and outcome.
- [ ] 7.5 Manually test invalid or missing API key; record date, environment, and outcome.
- [ ] 7.6 Manually test network/WebSocket disconnect during a live session; record date, environment, and outcome.
- [x] 7.7 Record any blocked manual scenarios and leave corresponding completion tasks unchecked unless the project owner accepts the residual risk.
