## ADDED Requirements

### Requirement: Python transport runs a live translation session
The Python `OpenAIRealtimeClient` (or equivalent function) SHALL connect to `wss://api.openai.com/v1/realtime?model=gpt-realtime` when `LiveMode.TRANSLATE` is active, send a `session.update` with `type: "realtime"`, `output_modalities: ["text"]`, a translation instruction targeting the configured language, and VAD settings with `create_response: true` and `interrupt_response: true`, matching the Rust `run_verified_translation_session` implementation exactly.

#### Scenario: Translation session connects and receives translation deltas
- **WHEN** the user starts listening with translation enabled and a target language of "French"
- **THEN** the client connects to the translation endpoint, sends the session update with a "Translate … into French" instruction, and emits `TRANSLATION_DELTA` events carrying translated text fragments

#### Scenario: Source language is forwarded when set
- **WHEN** the user selects "Spanish" as origin language before starting translation mode
- **THEN** the session update includes `transcription.language = "es"` so the server transcribes in Spanish before translating

#### Scenario: No source language when set to default
- **WHEN** origin language is set to "Default (Auto-detect)"
- **THEN** the session update omits the `transcription.language` field

#### Scenario: Lifecycle events emitted around translation session
- **WHEN** a translation session is started and later stopped
- **THEN** a `SESSION_STATE` event with `state="connecting"` is emitted before audio is sent, and a `SESSION_STATE` event with `state="disconnected"` is emitted after the socket closes

### Requirement: GTK UI supports live translation mode
The GTK `GtkLiveSession` SHALL route `LiveMode.TRANSLATE` through the translation session function rather than raising an error. The UI SHALL show translated text in the translated transcript pane as `TRANSLATION_DELTA` events arrive.

#### Scenario: User enables translate switch and starts recording
- **WHEN** the translate switch is active and the user presses the record button
- **THEN** a live translation session starts (no error is raised) and translated text accumulates in the translated transcript area

#### Scenario: GTK UI shows translation status
- **WHEN** the translation session is active
- **THEN** the status label reads "Translating live" while translation events arrive

### Requirement: Flask web UI exposes /ws/live/translate using translation session
The Flask WebSocket route `/ws/live/translate` SHALL start a live translation session via the Python transport, forwarding all `TRANSLATION_DELTA`, `SOURCE_DELTA`, `SOURCE_COMPLETED`, `SESSION_STATE`, and `ERROR` events to the browser as JSON messages.

#### Scenario: Browser connects to translate endpoint with target language
- **WHEN** a browser connects to `/ws/live/translate` and sends `{"type":"start","target_language":"de"}`
- **THEN** the server opens a translation session targeting German and streams translation delta events back

#### Scenario: Error if translation endpoint is unreachable
- **WHEN** the translation WebSocket endpoint returns a connection error
- **THEN** the server sends `{"type":"error","error":"<message>"}` and closes the WebSocket
