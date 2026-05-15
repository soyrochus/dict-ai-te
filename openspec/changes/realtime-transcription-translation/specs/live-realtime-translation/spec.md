## ADDED Requirements

### Requirement: Live translation session
The system SHALL start a dedicated realtime translation session when the user starts listening with live translation enabled.

#### Scenario: Start translation session
- **WHEN** the user activates Start Listening with Translate Live enabled
- **THEN** the application opens an OpenAI realtime translation session at the dedicated translation endpoint using `gpt-realtime-translate`
- **AND** microphone audio is streamed continuously to that session

#### Scenario: No chained text translation
- **WHEN** live translation is enabled
- **THEN** the application MUST NOT implement translation by repeatedly sending partial transcript text to a separate chat or text translation endpoint

### Requirement: Translation target language
The system SHALL configure a target output language for live translation.

#### Scenario: Target language selected
- **WHEN** the user selects a target language and starts live translation
- **THEN** the translation session is configured with that target output language

#### Scenario: Existing target language setting
- **WHEN** the user has an existing default target language setting
- **THEN** live translation uses that setting unless the user selects another target language

### Requirement: Translation transcript streams
The system SHALL surface both source and translated transcript streams during live translation.

#### Scenario: Source transcript delta
- **WHEN** OpenAI emits `session.input_transcript.delta`
- **THEN** the application appends the delta to the visible source transcript stream

#### Scenario: Translated transcript delta
- **WHEN** OpenAI emits `session.output_transcript.delta`
- **THEN** the application appends the delta to the visible translated transcript stream

### Requirement: Translated audio ignored safely
The system SHALL ignore translated audio deltas safely in the first implementation.

#### Scenario: Output audio delta
- **WHEN** OpenAI emits `session.output_audio.delta`
- **THEN** the application discards or ignores the audio payload without logging it
- **AND** source and translated transcript handling continues

### Requirement: Source language handling
The system SHALL treat source language for live translation as optional unless the selected API session documents a supported field.

#### Scenario: Unsupported source language field
- **WHEN** a source language is selected but the translation session does not document a source-language field
- **THEN** the application keeps the source language as UI metadata or a local hint
- **AND** it relies on automatic source-language detection
