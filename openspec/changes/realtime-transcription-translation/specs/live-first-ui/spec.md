## ADDED Requirements

### Requirement: Live-first controls
The user interface SHALL expose live listening as the primary interaction and SHALL NOT expose a classic recording mode.

#### Scenario: Default controls
- **WHEN** the application main screen is displayed
- **THEN** the primary audio control is Start Listening or Stop
- **AND** no user-facing Classic mode selector is shown

#### Scenario: No upload status
- **WHEN** the user stops live listening
- **THEN** the UI does not enter an uploading-audio or post-recording transcription phase

### Requirement: Live translation toggle
The user interface SHALL derive realtime mode from a Translate Live toggle.

#### Scenario: Toggle disabled
- **WHEN** Translate Live is disabled
- **THEN** the active live mode is transcription
- **AND** only the source transcript pane is required

#### Scenario: Toggle enabled
- **WHEN** Translate Live is enabled
- **THEN** the active live mode is translation
- **AND** both source transcript and translated transcript panes are visible

### Requirement: Connection state display
The user interface SHALL show realtime connection state.

#### Scenario: Connection lifecycle
- **WHEN** a live session moves through connecting, listening, transcribing, translating, disconnected, or error states
- **THEN** the UI displays the current state using user-readable text

#### Scenario: Error state
- **WHEN** microphone permission, API authentication, model availability, WebSocket, or network failure occurs
- **THEN** the UI displays a readable error
- **AND** any accumulated transcript remains available

### Requirement: Editable accumulated text
The user interface SHALL preserve edit, copy, save, and clear workflows for live transcripts.

#### Scenario: Editing after stop
- **WHEN** the user stops a live session
- **THEN** accumulated source and translated text remain editable

#### Scenario: Copy and save
- **WHEN** source or translated text exists
- **THEN** the user can copy and save the available text

### Requirement: Settings migration
The system SHALL migrate existing user settings into live-first settings without silently breaking existing configuration.

#### Scenario: Existing translation default
- **WHEN** existing settings contain `translate_by_default`
- **THEN** the application maps it to live translation being enabled by default

#### Scenario: Existing target language
- **WHEN** existing settings contain `default_target_language`
- **THEN** the application uses it as the live translation target language

#### Scenario: Existing source language
- **WHEN** existing settings contain `default_language`
- **THEN** the application uses it as source language if present
- **AND** uses automatic source-language detection when unset
