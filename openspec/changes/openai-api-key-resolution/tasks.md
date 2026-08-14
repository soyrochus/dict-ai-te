## 1. Settings schema and credential resolution

- [x] 1.1 Add defaulted `OpenAiSettings { api_key: Option<String> }` under `Settings.openai` without renaming existing fields or breaking unknown-key preservation
- [x] 1.2 Add source-aware resolved-key types whose secret value is private and excluded from debug output, with accessors for source, client use, and approved masking
- [x] 1.3 Implement one public `resolve_api_key(&Settings)` with environment → settings → absent precedence and a pure injected-input helper for deterministic tests
- [x] 1.4 Normalize both sources by trimming surrounding whitespace and treating blank values as absent
- [x] 1.5 Add tests for precedence, settings fallback, absence, whitespace handling, trimming, source reporting, masking, and non-secret debug output
- [x] 1.6 Add compatibility tests proving pre-change JSON defaults `openai.api_key` to absent and preserves all known and unknown fields
- [x] 1.7 Add serialization tests for stored-key round trip, clearing to absent, and proof that an environment-resolved key is never copied into settings

## 2. Secure settings persistence

- [x] 2.1 Add a Unix key-bearing save path that creates a same-directory temporary file with mode `0600`, writes and closes it, then atomically renames it into place
- [x] 2.2 Tighten an existing destination to `0600` whenever saved settings contain a key, while keeping non-Unix behavior and keyless saves compatible
- [x] 2.3 Clean up temporary files on failed writes or renames and preserve the previous settings file when installation fails
- [x] 2.4 Add Unix tests for a new key-bearing file, replacement of an existing permissive file, final mode `0600`, and absence of usable partial files

## 3. Point-of-use client lifecycle

- [x] 3.1 Remove the startup-created optional `OpenAiClient` as application credential state while retaining startup `.env` loading as an environment convenience
- [x] 3.2 Resolve and construct the OpenAI client at remote transcription/translation session start; refuse before microphone/network startup when no key resolves
- [x] 3.3 Resolve and construct the client inside the existing background path for transcript speech and voice preview
- [x] 3.4 Recompute effective key availability after Settings saves so entered, changed, and cleared stored keys affect the next operation without restart
- [x] 3.5 Route every Rust OpenAI call site through the single resolver and add a structural test or audit assertion preventing direct environment-only client construction from returning
- [x] 3.6 Add integration tests that a newly stored key is used on the next operation, a cleared key is not cached, and cloud session start with no key attempts no request
- [x] 3.7 Preserve local-backend startup and transcription with no key and add coverage proving they emit no missing-key warning or OpenAI request

## 4. Settings user interface

- [x] 4.1 Extend `SettingsModal` with staged stored-key text and ephemeral reveal state; do not seed the editable value from the effective environment key
- [x] 4.2 Add a paste-friendly single-line OpenAI API key editor that masks by default and reveals only after explicit user action
- [x] 4.3 Show the effective source (`environment`, `settings`, or `none`) and the approved masked stored-key representation without rendering the full secret elsewhere
- [x] 4.4 When `OPENAI_API_KEY` wins, show that a stored value is ignored until the variable is unset
- [x] 4.5 Keep the field visible and editable in local mode and show that local transcription does not require it
- [x] 4.6 Persist only the staged user-entered value, normalize blank input to absent, and verify Cancel does not mutate settings
- [x] 4.7 Add focused UI/state tests for mask/reveal behavior, source labels, environment override messaging, clearing, and local-mode visibility

## 5. Actionable errors and secret redaction

- [x] 5.1 Define one actionable missing-key message naming both `OPENAI_API_KEY` and the Settings field, and use it for cloud startup, session refusal, and disabled OpenAI speech controls
- [x] 5.2 Classify HTTP and Realtime authentication rejection (including HTTP 401) as a distinct rejected-key error without clearing the stored key
- [x] 5.3 Ensure connection, transport, HTTP, status, and background-task errors cannot include authorization headers or resolved key material
- [x] 5.4 Add tests distinguishing absent from rejected credentials and proving rejected settings credentials remain stored
- [x] 5.5 Add redaction tests covering resolved-key formatting and representative HTTP, WebSocket, UI, and log error paths with a sentinel secret

## 6. Documentation and verification

- [x] 6.1 Update `README.md` with the environment-first resolution order, in-app settings fallback, launch-directory behavior, local-mode semantics, and plaintext/`0600` storage note
- [x] 6.2 Run `cargo fmt --check`, default and local-feature `cargo test`, and Clippy with warnings denied for both configurations
- [x] 6.3 Run the existing live OpenAI smoke test with a valid environment key to confirm environment-backed transcription remains unchanged
- [ ] 6.4 Manually launch the release binary from a directory with no `.env` and no `OPENAI_API_KEY`, then verify a settings-stored key enables cloud transcription without restart
- [ ] 6.5 Manually verify environment override messaging, stored-key clearing, keyless local startup/session behavior, and disabled speech guidance
- [ ] 6.6 Run the application at every supported log level with a sentinel key and confirm no unmasked key material appears in logs, UI status, or errors
- [x] 6.7 Validate the OpenSpec change and record any manual checks that could not be completed in the current environment
