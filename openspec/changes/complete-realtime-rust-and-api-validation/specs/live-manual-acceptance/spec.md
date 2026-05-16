## ADDED Requirements

### Requirement: Manual live transcription acceptance
The change SHALL NOT be considered complete until live transcription has been manually tested or explicitly recorded as blocked by environment constraints.

#### Scenario: Successful live transcription
- **WHEN** the tester starts live transcription with a valid API key and working microphone
- **THEN** source text appears while or shortly after speech is captured
- **AND** stopping the session leaves the accumulated source text available

#### Scenario: Repeated stop and start
- **WHEN** the tester starts and stops live transcription repeatedly
- **THEN** each session starts cleanly
- **AND** stale audio channels, stale WebSocket tasks, and duplicate transcript state do not leak into the next session

### Requirement: Manual live translation acceptance
The change SHALL NOT be considered complete until live translation has been manually tested or explicitly recorded as blocked by environment constraints.

#### Scenario: Successful live translation
- **WHEN** the tester starts listening with Translate Live enabled and a target language selected
- **THEN** source transcript text and translated transcript text are surfaced in the Rust UI
- **AND** translated audio deltas, if emitted, do not disrupt transcript handling

#### Scenario: Target language selection
- **WHEN** the tester changes the target language before starting live translation
- **THEN** the realtime translation session uses the selected target language according to the verified API contract

### Requirement: Manual error acceptance
The change SHALL manually verify user-readable failure handling for common live-session failures.

#### Scenario: Microphone unavailable
- **WHEN** no microphone is available or microphone access fails
- **THEN** the Rust UI shows a readable microphone error
- **AND** the app remains usable

#### Scenario: Invalid API key
- **WHEN** the user starts a live session with an invalid or missing API key
- **THEN** the Rust UI shows a readable authentication or configuration error
- **AND** no sensitive key material is displayed

#### Scenario: Network disconnect
- **WHEN** the network or WebSocket connection fails during a live session
- **THEN** the Rust UI shows a readable connection error
- **AND** any accumulated transcript remains available

### Requirement: Acceptance evidence
The implementation SHALL record the outcome of manual acceptance checks before marking the realtime migration complete.

#### Scenario: Evidence recorded
- **WHEN** manual acceptance is performed
- **THEN** the tested scenarios, date, environment, and pass/fail/blocked outcomes are recorded in the change tasks or an adjacent verification note

#### Scenario: Blocked manual test
- **WHEN** a manual acceptance scenario cannot be run in the current environment
- **THEN** the reason is recorded
- **AND** the corresponding task remains incomplete unless the project owner accepts the residual risk
