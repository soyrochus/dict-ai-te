## ADDED Requirements

### Requirement: Users can select a transcription backend
The application SHALL expose a `backend` setting with the values `"openai"` and `"local"`, selectable from the Settings dialog and persisted to `settings.json`. The default SHALL be `"openai"`. A settings file that omits `backend` SHALL load as `"openai"` without requiring a migration step.

#### Scenario: Default backend for an existing settings file
- **WHEN** the application loads a `settings.json` written before this change, containing no `backend` or `local` keys
- **THEN** the effective backend is `"openai"`, all existing fields retain their values, and no error or warning is shown

#### Scenario: Selection persists across restarts
- **WHEN** the user selects "Local model" in Settings, closes the dialog, and restarts the application
- **THEN** the backend selector shows "Local model" and the next session starts on the local backend

#### Scenario: Unknown settings keys survive a save
- **WHEN** `settings.json` contains a key the current build does not recognise and the user saves settings
- **THEN** the unrecognised key and its value are still present in the written file

### Requirement: Local mode is transcription-only
When the effective backend is `"local"`, the application SHALL force the effective translate flag to `false` regardless of the persisted `translate_by_default` value, and SHALL disable the "Translate Live" control. A single shared helper SHALL compute the effective translate flag, used by both the settings UI and session start.

#### Scenario: Translation control is disabled in local mode
- **WHEN** the user selects the local backend
- **THEN** the "Translate Live" checkbox is unchecked and non-interactive, the target-language selector is hidden, and the note "Local mode supports transcription only. Translation requires the OpenAI Realtime API." is displayed

#### Scenario: Remote translation preference is preserved
- **WHEN** the user has `translate_by_default = true`, switches to the local backend, and later switches back to OpenAI Realtime
- **THEN** the "Translate Live" control is re-enabled and checked, because `translate_by_default` was never overwritten

#### Scenario: Session start ignores a stale translate flag
- **WHEN** a session is started with `backend = "local"` and `translate_by_default = true` in the settings file
- **THEN** a transcription-only local session starts and no translation session is created

### Requirement: Local mode requires no API key
When the effective backend is `"local"`, the application SHALL start sessions without an OpenAI API key, and SHALL NOT show the missing-API-key warning at startup.

#### Scenario: Session starts with no API key configured
- **WHEN** no `OPENAI_API_KEY` is available and the user starts a session with the local backend selected
- **THEN** the session starts normally and no key-related error is reported

#### Scenario: Startup warning is suppressed
- **WHEN** the application starts with the local backend selected and no API key configured
- **THEN** no missing-API-key warning is displayed

#### Scenario: TTS control is gated rather than failing
- **WHEN** the local backend is selected, no API key is configured, and a transcript is present
- **THEN** the speak-transcript control is disabled with a tooltip explaining that TTS requires an OpenAI API key, and clicking it produces no request

#### Scenario: TTS still works with a key present
- **WHEN** the local backend is selected and an API key is configured
- **THEN** the speak-transcript control is enabled and TTS behaves exactly as on the remote backend

### Requirement: Local mode performs no speech-related network traffic
A local session SHALL NOT construct an OpenAI Realtime session configuration and SHALL NOT invoke the remote transcription or translation transport functions. The only network traffic the local path may perform is model download, and only on explicit user action.

#### Scenario: Remote transport is never reached
- **WHEN** a session runs to completion on the local backend
- **THEN** no `RealtimeSessionConfig` is constructed and neither `run_live_transcription` nor `run_live_translation` is called

#### Scenario: Audio is never transmitted
- **WHEN** the user speaks during a local session
- **THEN** captured audio is consumed only by the local engine and no outbound request carries audio data

### Requirement: Local mode respects the existing origin language selector
Local mode SHALL use the existing Origin language selector as its only language input and SHALL NOT introduce a second language setting. The `default` entry SHALL map to engine auto-detect; every other entry SHALL pass its ISO code to the engine.

#### Scenario: Specific language is forwarded
- **WHEN** the user selects "Español (Spanish)" as origin language and starts a local session
- **THEN** the engine receives the language hint `es`

#### Scenario: Auto-detect is requested
- **WHEN** origin language is "Default (Auto-detect)"
- **THEN** the engine is started in auto-detect mode with no language hint

#### Scenario: Model does not support the selected language
- **WHEN** the user selects a non-English origin language while an English-only model such as `base.en` is configured
- **THEN** the application shows a warning that the model is English-only, transcribes as English, and does not fail the session

### Requirement: Backend changes apply at session boundaries
A change to the `backend` setting SHALL take effect at the next session start. If a session is active when the backend changes, the application SHALL cleanly stop the current session before applying the change.

#### Scenario: Change while idle
- **WHEN** no session is active and the user changes the backend
- **THEN** the next session started uses the newly selected backend

#### Scenario: Change during an active session
- **WHEN** a session is running and the user changes the backend
- **THEN** the running session is stopped cleanly, its transcript is finalized and preserved, and the new backend applies to the next session
