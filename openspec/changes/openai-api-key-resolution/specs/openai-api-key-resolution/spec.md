## ADDED Requirements

### Requirement: API keys have one deterministic resolution order
The Rust application SHALL resolve an OpenAI API key from the first non-blank source in this order: `OPENAI_API_KEY` in the process environment, then `openai.api_key` in application settings, then absent. Values loaded into the environment by `dotenvy` SHALL participate as environment values, existing environment values SHALL retain precedence over `.env`, surrounding whitespace SHALL be trimmed, and whitespace-only values SHALL be treated as absent. No other file or location SHALL be consulted.

#### Scenario: Environment key wins
- **WHEN** non-blank keys exist in both `OPENAI_API_KEY` and `openai.api_key`
- **THEN** the resolved value is the trimmed environment key and its source is reported as environment

#### Scenario: Settings key is the fallback
- **WHEN** `OPENAI_API_KEY` is absent or blank and `openai.api_key` contains a non-blank value
- **THEN** the resolved value is the trimmed settings key and its source is reported as settings

#### Scenario: No usable key exists
- **WHEN** both sources are absent or contain only whitespace
- **THEN** resolution reports that no key is available

### Requirement: Key resolution occurs at the point of use
The application SHALL resolve the current key when starting an OpenAI transcription or translation session, when starting an OpenAI speech request, and after settings are saved. It SHALL NOT rely on a client constructed once at process startup as the source of truth. A stored key entered, changed, or cleared through Settings SHALL affect the next operation without restarting the application.

#### Scenario: Newly entered key works without restart
- **WHEN** the application started without a key and the user saves a valid key in Settings
- **THEN** the next OpenAI session or speech request constructs its client from that key without an application restart

#### Scenario: Cleared key stops subsequent use
- **WHEN** a settings-sourced key is cleared and saved while no environment key exists
- **THEN** the next OpenAI operation treats the key as absent and does not reuse a previously constructed client

#### Scenario: Environment changes remain authoritative
- **WHEN** the effective environment key differs from the stored key at the time an operation starts
- **THEN** that operation uses the current environment value

### Requirement: Settings store only user-entered credentials
Settings SHALL contain an `openai` object with nullable `api_key`, defaulting to `null`. Existing settings files that omit the object SHALL load unchanged, unknown keys SHALL survive load-save round trips, a user-entered key SHALL round-trip, and clearing the field SHALL remove the stored key. The application SHALL NOT copy an environment-resolved key into settings.

#### Scenario: Pre-change settings remain compatible
- **WHEN** a settings file has no `openai` object
- **THEN** it loads with `openai.api_key` absent and all existing settings retain their values

#### Scenario: Stored key round-trips
- **WHEN** the user enters a key in Settings and saves
- **THEN** the trimmed key is serialized under `openai.api_key` and is available after reload

#### Scenario: Clearing removes the stored key
- **WHEN** the user clears the API key field and saves
- **THEN** `openai.api_key` becomes absent or `null` and the old value is not retained

#### Scenario: Environment key is never persisted implicitly
- **WHEN** the active source is `OPENAI_API_KEY` and the user saves unrelated settings without entering a stored key
- **THEN** the environment value is not written anywhere in the settings file

### Requirement: Key-bearing settings files use owner-only permissions on Unix
On Unix, whenever settings containing `openai.api_key` are written, the resulting settings file SHALL have mode `0600`. The write SHALL not leave a newly created key-bearing file temporarily readable by group or other users. Other platforms SHALL continue to use their native file-permission behavior.

#### Scenario: New key-bearing file is private
- **WHEN** settings containing a stored key are saved to a new file on Unix
- **THEN** the resulting file mode is `0600`

#### Scenario: Existing permissive file is tightened
- **WHEN** settings containing a stored key overwrite an existing Unix file with broader permissions
- **THEN** the resulting file mode is `0600`

### Requirement: Settings exposes safe credential controls and source information
The Settings dialog SHALL always show a paste-friendly single-line OpenAI API key field, including in local mode. It SHALL mask the stored value by default, SHALL reveal the full stored value only while the user explicitly enables reveal, and SHALL identify the active source as environment, settings, or none. When an environment key is active, the dialog SHALL explain that a stored value is ignored until `OPENAI_API_KEY` is unset. In local mode it SHALL explain that the key is optional.

#### Scenario: Stored key is masked by default
- **WHEN** Settings opens with a stored key and no environment override
- **THEN** the full key is not visibly rendered, a masked representation is shown, and the source is identified as settings

#### Scenario: User explicitly reveals a stored key
- **WHEN** the user activates the reveal control
- **THEN** the editable stored value is rendered without masking until reveal is disabled or the dialog closes

#### Scenario: Environment source is explained
- **WHEN** a non-blank environment key is active
- **THEN** the source is identified as environment and the dialog states that a key entered in Settings will be ignored until the environment variable is unset

#### Scenario: Key remains configurable in local mode
- **WHEN** the local transcription backend is selected
- **THEN** the API key field remains visible and editable with a note that local transcription does not require it

### Requirement: Missing-key behavior is feature-aware and actionable
When OpenAI functionality requires a key and none resolves, the application SHALL state that the user can set `OPENAI_API_KEY` or enter a key in Settings. With the OpenAI backend selected, startup SHALL show this warning and session start SHALL be refused before any request is attempted. An OpenAI speech control SHALL be disabled with the same remedy. With the local backend selected, absence of a key SHALL produce no startup or transcription-session warning.

#### Scenario: Cloud startup has no key
- **WHEN** the application starts with the OpenAI backend selected and neither key source is available
- **THEN** it shows a warning naming both `OPENAI_API_KEY` and the Settings field as remedies

#### Scenario: Cloud session is blocked before networking
- **WHEN** the user starts an OpenAI session with no resolved key
- **THEN** the session does not start, no OpenAI request is attempted, and the actionable missing-key message is shown

#### Scenario: Speech control has no key
- **WHEN** an OpenAI speech action is available conceptually but no key resolves
- **THEN** its control is disabled and its explanation names both supported configuration mechanisms

#### Scenario: Local transcription is keyless
- **WHEN** the local backend is selected and neither key source is available
- **THEN** application startup and local transcription proceed without a missing-key warning

### Requirement: Rejected credentials are distinct from missing credentials
An authentication rejection from an OpenAI HTTP or Realtime request SHALL be reported as a rejected-key error distinct from the missing-key message. The application SHALL NOT automatically clear a stored key after rejection.

#### Scenario: API rejects a present key
- **WHEN** OpenAI rejects a request with an authentication response such as HTTP 401
- **THEN** the UI states that the configured key was rejected and does not claim that no key was configured

#### Scenario: Rejected stored key is retained
- **WHEN** a settings-sourced key is rejected by OpenAI
- **THEN** the stored value remains available for the user to correct or replace explicitly

### Requirement: Secret material is never exposed
No application error, log record, status text, debug representation, or crash-oriented message SHALL contain the resolved API key or an unmasked stored key. Outside the explicitly revealed Settings field, the UI SHALL expose at most the approved masked representation.

#### Scenario: Transport or HTTP request fails
- **WHEN** an OpenAI operation fails after a key has been resolved
- **THEN** the resulting logs and user-visible error describe the failure without including the key

#### Scenario: Verbose logging is enabled
- **WHEN** the application runs at any supported log level with an environment or stored key
- **THEN** no log record contains the full key

