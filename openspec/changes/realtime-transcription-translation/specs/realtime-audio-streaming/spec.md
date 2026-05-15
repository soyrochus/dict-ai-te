## ADDED Requirements

### Requirement: Shared realtime audio format
The system SHALL stream microphone audio as mono 24 kHz signed 16-bit little-endian PCM chunks for realtime modes.

#### Scenario: Audio conversion
- **WHEN** microphone input is captured at any supported device sample rate or channel count
- **THEN** the application downmixes to mono
- **AND** resamples to 24 kHz
- **AND** converts to signed 16-bit little-endian PCM

#### Scenario: Chunk encoding
- **WHEN** PCM audio is ready to send to OpenAI
- **THEN** the application chunks it into approximately 20 to 100 ms blocks
- **AND** base64 encodes each chunk before sending it over WebSocket

### Requirement: No ffmpeg in live path
The system SHALL NOT use ffmpeg for live microphone streaming.

#### Scenario: Live audio processing
- **WHEN** the application is in live transcription or live translation mode
- **THEN** audio conversion is performed in-process without invoking ffmpeg

### Requirement: Browser WebSocket bridge
The web UI SHALL stream browser microphone audio through a server-owned WebSocket bridge.

#### Scenario: Browser audio message
- **WHEN** the browser has a base64 PCM chunk
- **THEN** it sends a WebSocket message with type `audio` and the encoded audio payload to Flask

#### Scenario: Server forwards audio
- **WHEN** Flask receives a valid audio message for an active live session
- **THEN** it forwards the chunk to the corresponding OpenAI realtime session
- **AND** it does not expose the standard OpenAI API key to browser JavaScript

### Requirement: WebSocket server dependency
The Python web implementation SHALL select an explicit server technology for bidirectional WebSocket streaming.

#### Scenario: Dependency chosen
- **WHEN** the Flask live WebSocket bridge is implemented
- **THEN** the project dependencies include a compatible WebSocket server approach
- **AND** live stream handling does not depend on plain Flask request-response routes alone

### Requirement: Non-blocking Rust audio pipeline
The Rust implementation SHALL keep microphone callbacks short and non-blocking.

#### Scenario: Rust audio callback
- **WHEN** the audio input callback receives samples
- **THEN** it quickly pushes samples or prepared chunks into a bounded channel
- **AND** it does not perform network calls

#### Scenario: Rust audio worker
- **WHEN** the Rust worker receives audio samples
- **THEN** it performs downmixing, resampling, PCM conversion, chunking, and forwarding outside the audio callback
