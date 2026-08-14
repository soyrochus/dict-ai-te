## Why

OpenAI features currently depend on `OPENAI_API_KEY` being present when the process starts, with `.env` discovery tied to the launch working directory. The same binary therefore works under `cargo run` but can silently lose cloud transcription, translation, and TTS when launched from a desktop bundle, and users cannot repair the configuration inside the running application.

## What Changes

- Add a single API-key resolver with explicit precedence: non-blank `OPENAI_API_KEY`, then a stored `openai.api_key`, then absent.
- Resolve the key at the point of use so a key entered or cleared in Settings takes effect without restarting.
- Add a nested `openai` settings object and preserve compatibility with existing settings files and unknown keys.
- Add a masked, revealable OpenAI API key field to Settings, including the active source and an environment-override notice.
- Persist only keys explicitly entered by the user; never copy an environment-provided key into settings.
- Write key-bearing settings files with owner-only permissions on Unix.
- Replace ambiguous missing-key behavior with actionable messages, distinguish API rejection from absence, and ensure secrets never appear in UI text or logs.
- Keep local transcription fully usable without a key and make no changes to the deprecated Python applications.

## Capabilities

### New Capabilities

- `openai-api-key-resolution`: Environment-first key resolution with a settings fallback, runtime refresh, secure persistence rules, settings UI, source reporting, redaction, and feature-aware missing/rejected-key behavior.

### Modified Capabilities

<!-- No baseline specs exist under openspec/specs/. Interactions with the still-active local transcription change are stated in the new capability without modifying that change's artifacts. -->

## Impact

- **Settings:** `src/settings.rs` gains `OpenAiSettings`, serialization defaults, key resolution metadata, and Unix permission handling.
- **Application:** `src/app.rs` stops treating the startup-created `OpenAiClient` as the source of truth, resolves credentials for remote sessions and TTS at point of use, and adds the settings controls and actionable messages.
- **OpenAI client:** `src/openai.rs` constructs clients from resolved keys and classifies authentication failures without exposing credentials.
- **Startup:** `src/main.rs` retains `.env` loading but no longer permanently fixes key availability for the process lifetime.
- **Documentation/tests:** README guidance and automated/manual coverage expand to include resolution precedence, runtime updates, redaction, compatibility, and file permissions.
- **Dependencies:** no new crate is expected; v1 deliberately excludes operating-system credential stores and settings encryption.
