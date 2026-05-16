## ADDED Requirements

### Requirement: Rust live microphone streaming
The Rust application SHALL stream microphone audio to realtime sessions without waiting for a completed recording.

#### Scenario: Start live capture
- **WHEN** the user activates Start Listening in the Rust egui application
- **THEN** the application opens a live microphone capture path
- **AND** microphone audio is forwarded toward the realtime session while capture is active

#### Scenario: Bounded capture path
- **WHEN** microphone input is captured by `cpal`
- **THEN** the audio callback returns quickly after pushing samples or prepared audio units into a bounded channel
- **AND** network I/O, WebSocket writes, and heavy resampling do not run inside the audio callback

#### Scenario: Realtime audio format
- **WHEN** audio is sent to an OpenAI realtime session
- **THEN** it is mono 24 kHz signed 16-bit little-endian PCM
- **AND** it is encoded/chunked according to the verified realtime API contract

### Requirement: Rust realtime session lifecycle
The Rust application SHALL manage transcription and translation sessions on a background Tokio runtime and SHALL forward normalized events to egui.

#### Scenario: Start transcription session
- **WHEN** the user starts listening with Translate Live disabled
- **THEN** the Rust application starts a realtime transcription session using the verified transcription API contract
- **AND** source transcript events are forwarded to the UI event channel

#### Scenario: Start translation session
- **WHEN** the user starts listening with Translate Live enabled
- **THEN** the Rust application starts a realtime translation session using the verified translation API contract
- **AND** source and translated transcript events are forwarded to the UI event channel

#### Scenario: Stop live session
- **WHEN** the user activates Stop during a live session
- **THEN** microphone capture stops
- **AND** audio forwarding stops
- **AND** the realtime session is closed using the verified close or commit semantics
- **AND** accumulated transcript text remains available

#### Scenario: Background failure
- **WHEN** microphone, runtime, WebSocket, authentication, model, or network failure occurs
- **THEN** the UI receives a readable error event
- **AND** the accumulated transcript remains available for editing, copying, and saving

### Requirement: Rust live-first egui interface
The Rust egui interface SHALL expose live listening as the primary transcription/translation interaction.

#### Scenario: Main control
- **WHEN** the Rust main screen is displayed
- **THEN** the primary audio control is Start Listening or Stop
- **AND** no user-facing classic recording mode is shown

#### Scenario: Translation disabled
- **WHEN** Translate Live is disabled
- **THEN** the active mode is live transcription
- **AND** the source transcript pane is visible
- **AND** a separate translated transcript pane is not required

#### Scenario: Translation enabled
- **WHEN** Translate Live is enabled
- **THEN** the active mode is live translation
- **AND** both source transcript and translated transcript panes are visible
- **AND** the selected target language is used when starting the session

#### Scenario: No post-recording status
- **WHEN** the user stops a live session
- **THEN** the Rust UI does not enter an uploading-audio or post-recording transcription phase

### Requirement: Rust transcript preservation
The Rust application SHALL preserve edit, copy, save, clear, and text playback workflows for accumulated live transcripts.

#### Scenario: Editable after stop
- **WHEN** a live session stops
- **THEN** accumulated source and translated text remain editable

#### Scenario: Copy and save available text
- **WHEN** source or translated transcript text exists
- **THEN** the user can copy and save the available text

#### Scenario: Source segment reconciliation
- **WHEN** realtime source transcript completion events arrive
- **THEN** the application replaces matching partial text by segment identity
- **AND** the visible transcript does not duplicate partial and final text

#### Scenario: Translated audio ignored
- **WHEN** a realtime translation session emits translated audio delta data
- **THEN** the Rust application ignores the audio payload safely in this implementation
- **AND** source and translated transcript handling continues
