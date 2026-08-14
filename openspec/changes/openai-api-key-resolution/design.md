## Context

The Rust application currently loads `.env`, constructs `OpenAiClient` once in `main`, and passes an optional client into `DictaiteApp`. That lifetime makes key availability dependent on the launch working directory and prevents a value saved from the Settings dialog from taking effect until restart. The settings schema already supports nested local-backend configuration and preserves unknown top-level keys, but it has no OpenAI section and writes JSON with ordinary platform defaults.

This change crosses settings serialization, application lifecycle, remote WebSocket startup, blocking TTS requests, UI state, error classification, and filesystem security. The deprecated Python implementations remain untouched. The source specification is `specs/2026-08-14-openai-api-key-resolution.md`.

## Goals / Non-Goals

**Goals:**

- Make one resolver authoritative for every OpenAI operation.
- Preserve environment-first behavior while providing a launch-directory-independent settings fallback.
- Apply settings edits without restarting or retaining stale clients.
- Make credential source and missing/rejected states understandable without exposing secrets.
- Ensure a settings file containing a credential is private on Unix.
- Preserve legacy settings, unknown-key round trips, local keyless behavior, and existing OpenAI feature behavior.

**Non-Goals:**

- OS keychain or credential-manager integration.
- Encryption of the settings file.
- Multiple profiles, per-session overrides, or an in-dialog API validation request.
- Searching for credentials beyond the process environment and application settings.
- Changes to the Python GTK or Flask applications.

## Decisions

### Represent resolution as value plus source

Add `OpenAiSettings { api_key: Option<String> }` to `Settings` and a resolver result that distinguishes `Environment`, `Settings`, and `Absent`. Keep the actual key field private and avoid deriving or implementing a debug representation that prints it. Provide narrow accessors for client construction, source display, and an approved masked value.

The public `resolve_api_key(&Settings)` reads `OPENAI_API_KEY` at call time. A pure internal variant accepts an injected environment value so precedence, trimming, and blank handling can be tested without process-global environment races.

Alternative considered: return only `Option<String>`. Rejected because the UI must report the source and warn when an environment value overrides a stored value; recomputing those facts separately risks divergent precedence rules.

### Stage only the stored value in the Settings modal

The modal owns an editable copy of `settings.openai.api_key`, not the effective environment value. Masking is a presentation property of the text edit, and reveal state is ephemeral modal state. Saving normalizes blank input to `None`; it never assigns the resolver's environment value to settings. Source labels are computed from the resolver against staged settings so they refresh after editing and saving.

Alternative considered: populate the field with the effective key. Rejected because saving an unrelated preference could then copy an environment secret to disk, directly violating the persistence requirement.

### Construct OpenAI clients at operation boundaries

Remove the startup-created optional client as the application's credential authority. At remote session start, transcript speech, and voice preview, resolve the key and construct `OpenAiClient` from that value. Blocking TTS work constructs and owns its client inside the existing background thread. `.env` remains loaded once near startup as a convenience, but its result is simply part of the process environment and does not bypass the resolver.

Alternative considered: mutate or replace a cached client whenever Settings saves. Rejected because it adds cache invalidation paths, can retain cleared credentials, and still requires special handling for environment precedence.

### Centralize actionable and rejected-key errors

Define one missing-key message used by startup, session start, and disabled speech controls. It names both remedies. Map authentication failures from blocking HTTP and Realtime WebSocket setup to a separate rejected-key error without embedding request headers, URLs containing secrets, or key material. Do not clear settings automatically because authentication can fail for transient account or project reasons as well as a mistyped key.

Alternative considered: surface raw library errors everywhere. Rejected because wording differs by transport, can confuse absence with rejection, and makes redaction harder to audit.

### Write key-bearing settings privately from creation

On Unix, serialize first, then write through a same-directory temporary file created with mode `0600`, sync/close it, and rename it over the destination. Explicitly enforce `0600` on the installed file as defense for existing permissive paths. Files without a stored key retain existing save behavior; Windows and other non-Unix targets keep native permission semantics.

Alternative considered: call `fs::write` and chmod afterward. Rejected because a newly created file can be readable under the user's umask between creation and chmod. OS credential storage would be stronger but is outside v1.

### Keep local transcription independent from credential availability

Startup and session gating consult the effective backend before treating absence as an error. The key field remains available in local mode because OpenAI features may still be selected later, but local transcription does not construct an OpenAI client or emit a missing-key warning.

## Risks / Trade-offs

- **Plaintext credentials remain in settings** → Label the behavior in documentation, require explicit user entry, enforce `0600` on Unix, never copy environment secrets, and keep OS credential storage as a future extension.
- **Environment mutation in tests is process-global** → Put precedence logic behind a pure injected-input helper and keep direct environment reads in a thin wrapper.
- **Secret-bearing types can leak through future debug logging** → Keep key fields private, omit secret values from `Debug`, centralize masking, and add redaction-focused tests/audit searches.
- **Atomic replacement can interact with platform-specific rename semantics** → Limit the private temporary-file path to Unix, keep the write in the destination directory, and clean up failed temporary files.
- **Repeated client construction has small overhead** → Accept it because sessions and speech requests are coarse operations; correctness and eliminating stale credentials outweigh negligible setup cost.
- **An environment override may surprise users editing Settings** → Show the active source and a persistent override notice beside the field.

## Migration Plan

1. Add the defaulted nested settings object and resolver without changing call sites; verify legacy and unknown-key round trips.
2. Route remote session and TTS entry points through point-of-use resolution, then remove startup client authority.
3. Add masked settings controls, source reporting, environment override guidance, and local-mode note.
4. Add private Unix persistence, authentication classification, redaction tests, and documentation.
5. Run automated coverage in default and local-feature builds, followed by the source specification's release-launch and log-redaction manual checks.

Rollback is code-only: old settings readers ignore or preserve the nested `openai` object through unknown-key handling, and removing the new resolver restores environment-only behavior without data migration.

## Open Questions

- None for v1. OS credential stores, key validation, and named profiles remain explicitly deferred.
