## ADDED Requirements

### Requirement: Live transcription session
The system SHALL start a realtime transcription session when the user starts listening with live translation disabled.

#### Scenario: Start transcription session
- **WHEN** the user activates Start Listening with Translate Live disabled
- **THEN** the application opens an OpenAI Realtime transcription session using `gpt-realtime-whisper`
- **AND** microphone audio is streamed without waiting for a completed recording

#### Scenario: Stop transcription session
- **WHEN** the user activates Stop during live transcription
- **THEN** microphone capture stops
- **AND** the realtime session closes cleanly
- **AND** accumulated transcript text remains available for editing, copying, saving, or clearing

### Requirement: Transcription event handling
The system SHALL normalize realtime transcription events into source transcript deltas and completions.

#### Scenario: Delta event
- **WHEN** OpenAI emits `conversation.item.input_audio_transcription.delta`
- **THEN** the system updates the partial source transcript for the event `item_id`
- **AND** the UI can display the new text before the utterance is complete

#### Scenario: Completion event
- **WHEN** OpenAI emits `conversation.item.input_audio_transcription.completed`
- **THEN** the system replaces the matching partial segment with the final transcript
- **AND** the visible source transcript does not duplicate partial and final text

### Requirement: Segment ordering
The system SHALL assemble source transcripts by segment identity instead of raw append-only text.

#### Scenario: Out-of-order completion
- **WHEN** completion events arrive in a different order from speech turns
- **THEN** the system uses `item_id` and first-seen ordering to preserve coherent visible transcript order

#### Scenario: Missing item id
- **WHEN** a transcription delta does not contain an `item_id`
- **THEN** the system appends it to a temporary live buffer
- **AND** the application does not crash

### Requirement: Transcription-only output
The system SHALL keep live transcription mode focused on source text and SHALL NOT require translated text or translated audio output.

#### Scenario: Translation disabled
- **WHEN** live translation is disabled
- **THEN** only the source transcript area is required to be visible
- **AND** no text translation endpoint is called for partial transcript text
