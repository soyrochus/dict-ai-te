# Specification: OpenAI API Key Resolution for dict-ai-te

**Version:** 1.0
**Status:** Draft for implementation
**Scope:** Give the OpenAI API key a defined resolution order — environment, then settings file — and a clear failure message when it is needed but absent.

> **Note on implementations:** This specification covers the Rust implementation only. The Python implementations (GTK desktop and Flask web) are deprecated and out of scope — they are not modified by this work.

**Companion documents:** `specs/2026-08-14-local-transcription-backend.md` (v2.0), which introduced the `backend` setting, and `specs/2026-08-14-local-speech-synthesis.md` (v1.0).

---

### 1. Problem

The API key is read exclusively from the process environment, with `.env` loaded by `dotenvy` as a
convenience. The settings file has no field for it, and the client is constructed once at startup:
if no key is found at that moment, the application runs keyless for the rest of the process.

Two consequences:

- **The key depends on the working directory.** `dotenvy` searches the current directory and its
  ancestors. `cargo run` from the repository root finds `.env`; the same binary launched from a
  desktop launcher, an application bundle, or any other directory does not. Cloud mode then fails
  while on-device mode works, from an identical binary, for reasons invisible to the user.
- **There is no way to supply a key from inside the application.** The Settings dialog has no field,
  because there is nowhere to persist one. Recovering from a missing key means quitting, arranging
  an environment variable or a `.env` in the right place, and starting again.

### 2. Goals

- A single, documented resolution order, applied consistently everywhere the key is needed.
- A key that can be entered in the application and persisted, so the app works when launched from
  anywhere.
- A clear, actionable message when a key is required and absent — and silence when it is not.
- Environment configuration continues to win, so scripted and CI usage is unaffected.
- No secret is written to disk unless the user explicitly entered it.

### 3. Non-Goals (v1)

- Operating-system credential storage (Keychain, Secret Service, Credential Manager). Noted as a
  future extension in §10.
- Encrypting the settings file.
- Multiple named profiles or per-session key overrides.
- Validating the key against the API before use, beyond reporting authentication failures as they
  occur.
- Any change to the deprecated Python tree.

---

### 4. Resolution Order

The key SHALL be resolved in this order, first match winning:

```
   1. OPENAI_API_KEY in the process environment
        │  (includes values loaded from .env by dotenvy, which searches
        │   the current directory and its ancestors; an existing
        │   environment variable is never overridden by .env)
        ▼
   2. openai.api_key in ~/.dictaite/settings.json
        │
        ▼
   3. Absent
        ├─ backend = "openai"  →  actionable error (§7)
        └─ backend = "local"   →  no error, no warning; the key is not needed
```

**Rules:**

- A value consisting only of whitespace SHALL be treated as absent at every level, so an empty
  settings field falls through to "absent" rather than producing an invalid client.
- Values SHALL be trimmed of surrounding whitespace before use.
- Resolution SHALL be performed when a key is actually needed — at session start, at a speech
  request, and when settings are saved — rather than once at process startup. A key entered in the
  Settings dialog SHALL take effect without restarting the application.
- Nothing outside this order is consulted. There is no implicit search of other files or locations.

---

### 5. Configuration & Settings

```json
{
  "backend": "openai",
  "openai": {
    "api_key": null
  },
  "local": { }
}
```

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `openai.api_key` | string \| null | `null` | Key used when no environment variable is set |

**Rules:**

- The field is nested under `openai` for symmetry with the existing `local` object. No shipped field
  is renamed, and a settings file written before this change loads unchanged.
- The application SHALL NOT persist a key that came from the environment. Only a key the user typed
  into the Settings dialog is written to disk, so a secret provided by the environment never leaks
  into a file that may be backed up or synchronised.
- Clearing the field in the Settings dialog SHALL remove the stored key.
- On Unix, the settings file SHALL be written with owner-only permissions (`0600`) whenever it
  contains a key.
- The unknown-key preservation established by the transcription change continues to apply.

---

### 6. User Interface

- The Settings dialog gains an **OpenAI API key** field, masked by default, with a reveal control
  and a paste-friendly single-line input.
- When the key comes from the environment, the field SHALL indicate this and make clear that the
  stored value is not in use — for example *"Using OPENAI_API_KEY from the environment. A key
  entered here will be ignored until that variable is unset."* Without this, a user who edits the
  field while an environment variable is set sees no effect and concludes the application is broken.
- The field SHALL display only a masked form of any stored key (for example the last four
  characters). The full value is never rendered unless the user activates the reveal control.
- The dialog SHALL show which source is currently supplying the key: environment, settings file, or
  none.
- The key field SHALL remain visible and editable in on-device mode, with a note that it is not
  required there — a user may switch to cloud mode later.

---

### 7. Error Handling

| Condition | Behaviour |
| --- | --- |
| No key, backend is `"local"` | No error, no warning, at startup or at any other time |
| No key, backend is `"openai"`, at startup | A warning stating that no key was found, naming both mechanisms: set `OPENAI_API_KEY` or enter a key in Settings |
| No key, backend is `"openai"`, at session start | The session does not start; the same actionable message is shown |
| No key, backend is `"openai"`, at speech request | The speak control is disabled with the same explanation rather than failing on click |
| Key present but rejected by the API (401) | A distinct message stating the key was rejected, so it is not confused with a missing key; the stored key is not cleared automatically |
| Key present but malformed (empty after trimming) | Treated as absent, per §4 |

**Rules:**

- No error message, log line, status text, or crash report SHALL contain the key or any part of it
  beyond the masked form defined in §6.
- Messages SHALL name the concrete remedy. "OPENAI_API_KEY not configured" states the fact but not
  the fix; the message must tell the user that a key can be set in Settings as well as in the
  environment.

---

### 8. Testing Requirements

**Unit**

- Precedence: environment beats settings; settings is used when the environment is unset; absent
  when neither is present.
- Whitespace-only values at either level are treated as absent.
- Values are trimmed before use.
- A settings file written before this change loads with `openai.api_key` absent.
- A key resolved from the environment is not written to the settings file on save.
- Settings serialization round-trips a stored key, and clearing the field removes it.

**Integration**

- With `backend = "local"` and no key anywhere, the application starts and runs a session with no
  warning at any point.
- With `backend = "openai"` and no key anywhere, session start is refused with the actionable
  message, and no request is attempted.
- A key entered in the Settings dialog takes effect on the next session without restarting.
- On Unix, a settings file containing a key has `0600` permissions.

**Manual**

- Launch the release binary from a directory with no `.env` and no environment variable, with a key
  stored in settings: cloud mode works.
- Confirm no key material appears in application logs at any log level.

---

### 9. Implementation Path

| Phase | Content |
| --- | --- |
| 1 | `openai.api_key` in settings; a single `resolve_api_key(&settings)` helper implementing §4; all call sites routed through it |
| 2 | Defer client construction so resolution happens at point of need, and a key entered at runtime applies without restart |
| 3 | Settings dialog field: masked display, reveal control, source indicator, environment-override notice |
| 4 | Error messages per §7, file permissions, and redaction audit of logs and status text |

---

### 10. Future Extensions (Out of Scope for v1)

- Operating-system credential storage as a higher-precedence source, placed between the environment
  and the settings file, keeping the settings fallback for platforms where it is unavailable.
- A "test key" action in Settings that validates the key against the API and reports the result.
- Per-profile keys for users with separate personal and organisational accounts.
