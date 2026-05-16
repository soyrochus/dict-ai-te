## ADDED Requirements

### Requirement: Official Realtime API verification
The implementation SHALL verify OpenAI Realtime API assumptions against current official OpenAI documentation before finalizing realtime transport behavior.

#### Scenario: Verify transcription contract
- **WHEN** implementing or reviewing realtime transcription transport
- **THEN** the implementer verifies the transcription endpoint, model names, session update payload shape, input audio format fields, append event, transcript delta event, and transcript completion event against official OpenAI documentation
- **AND** code and documentation use the verified contract

#### Scenario: Verify translation contract
- **WHEN** implementing or reviewing realtime translation transport
- **THEN** the implementer verifies the translation endpoint, model name, target-language field, source-language support, transcript events, translated audio event, and close semantics against official OpenAI documentation
- **AND** unsupported or changed API fields are not sent as authoritative configuration

#### Scenario: Document verified contract
- **WHEN** the verified API contract differs from the older OpenSpec assumptions
- **THEN** the implementation updates the relevant architecture notes, code constants, or task notes to show the current verified contract

### Requirement: API contract drift handling
The realtime implementation SHALL be tolerant of unknown or changed server events without crashing.

#### Scenario: Unknown event
- **WHEN** the realtime server emits an event type the application does not recognize
- **THEN** the event parser returns an unknown/no-op event
- **AND** the active session continues unless the event represents a fatal error

#### Scenario: Error event
- **WHEN** the realtime server emits an error event
- **THEN** the application forwards a readable error to the UI
- **AND** it avoids logging audio payloads or sensitive API key material

#### Scenario: Unsupported source language field
- **WHEN** the selected realtime translation API contract does not document a source-language field
- **THEN** the application treats source language as local UI metadata or a hint only
- **AND** it relies on automatic source-language detection

### Requirement: No chained text translation fallback
The application SHALL NOT satisfy live translation by repeatedly translating partial source transcript text through a separate text/chat translation endpoint.

#### Scenario: Translate Live enabled
- **WHEN** the user starts listening with Translate Live enabled
- **THEN** the application uses a verified realtime translation-capable session path
- **AND** it does not call a separate text translation endpoint for partial transcript text

#### Scenario: Translation API unavailable
- **WHEN** no verified realtime translation-capable API path is available
- **THEN** the implementation reports live translation as unavailable or blocked
- **AND** it does not silently degrade to chained text translation
