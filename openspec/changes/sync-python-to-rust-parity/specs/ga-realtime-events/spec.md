## ADDED Requirements

### Requirement: Event parser handles all GA translation text delta variants
The `parse_realtime_event` function SHALL map `"response.output_text.delta"` and `"response.output_audio_transcript.delta"` to `TRANSLATION_DELTA`, in addition to the existing `"session.output_transcript.delta"` mapping. The `delta` field value SHALL be used as the text payload.

#### Scenario: response.output_text.delta produces TRANSLATION_DELTA
- **WHEN** the server sends `{"type":"response.output_text.delta","delta":"Hola"}`
- **THEN** `parse_realtime_event` returns a `NormalizedEvent` with type `TRANSLATION_DELTA` and text `"Hola"`

#### Scenario: response.output_audio_transcript.delta produces TRANSLATION_DELTA
- **WHEN** the server sends `{"type":"response.output_audio_transcript.delta","delta":"Bonjour"}`
- **THEN** `parse_realtime_event` returns a `NormalizedEvent` with type `TRANSLATION_DELTA` and text `"Bonjour"`

### Requirement: Event parser handles all GA audio delta variants
The `parse_realtime_event` function SHALL map `"response.audio.delta"` and `"response.output_audio.delta"` to `TRANSLATED_AUDIO_DELTA`, in addition to the existing `"session.output_audio.delta"` mapping.

#### Scenario: response.audio.delta produces TRANSLATED_AUDIO_DELTA
- **WHEN** the server sends `{"type":"response.audio.delta","delta":"<base64>"}`
- **THEN** `parse_realtime_event` returns a `NormalizedEvent` with type `TRANSLATED_AUDIO_DELTA`

#### Scenario: response.output_audio.delta produces TRANSLATED_AUDIO_DELTA
- **WHEN** the server sends `{"type":"response.output_audio.delta"}`
- **THEN** `parse_realtime_event` returns a `NormalizedEvent` with type `TRANSLATED_AUDIO_DELTA`

### Requirement: Session state catch-all matches any session.* or response.* event
The `parse_realtime_event` function SHALL map any event whose type starts with `"session."` or `"response."` — and that has not already been matched by a more specific rule — to `SESSION_STATE`, with the full event type string as `state`. This replaces the previous explicit allow-list of four event names.

#### Scenario: session.created maps to SESSION_STATE
- **WHEN** the server sends `{"type":"session.created"}`
- **THEN** `parse_realtime_event` returns a `NormalizedEvent` with type `SESSION_STATE` and `state="session.created"`

#### Scenario: response.done maps to SESSION_STATE
- **WHEN** the server sends `{"type":"response.done"}`
- **THEN** `parse_realtime_event` returns a `NormalizedEvent` with type `SESSION_STATE` and `state="response.done"`

#### Scenario: Unknown future response.* event maps to SESSION_STATE
- **WHEN** the server sends `{"type":"response.some_new_event_type"}`
- **THEN** `parse_realtime_event` returns a `NormalizedEvent` with type `SESSION_STATE` and `state="response.some_new_event_type"` (not UNKNOWN)

#### Scenario: Truly unknown events still map to UNKNOWN
- **WHEN** the server sends `{"type":"other.unknown_event"}`
- **THEN** `parse_realtime_event` returns a `NormalizedEvent` with type `UNKNOWN`
