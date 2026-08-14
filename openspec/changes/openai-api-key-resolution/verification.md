# Verification

Automated verification completed on 2026-08-14:

- `cargo fmt --check`
- `cargo test` (47 passed; the one live test is ignored by the regular suite)
- `cargo test --features local-whisper` (56 passed; the one live test is ignored by the regular suite)
- `cargo clippy --all-targets -- -D warnings`
- `cargo clippy --all-targets --features local-whisper -- -D warnings`
- `cargo test realtime::transport::tests::live_transcription_session_accepts_audio -- --ignored --nocapture` (passed with the configured environment key)

The interactive release checks in tasks 6.4–6.6 were not completed in this automation environment.
They require changing the host process environment, entering and clearing a real credential in the
GUI, granting microphone access, exercising cloud and local audio, and observing the application at
each supported log level. They remain unchecked for a human smoke test; no result is inferred from
the automated coverage.
