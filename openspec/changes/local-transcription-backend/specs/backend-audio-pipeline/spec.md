## ADDED Requirements

### Requirement: Backends declare the audio format they require
The application SHALL define an `AudioSpec` carrying a `sample_rate` and a `frame_samples` count, and each backend SHALL declare its own. The OpenAI backend SHALL declare 24 kHz; the Whisper local engine SHALL declare 16 kHz with a frame size of 512 samples. `LiveCapture` SHALL be started with the `AudioSpec` of the selected backend.

#### Scenario: Remote backend drives capture configuration
- **WHEN** a session starts on the OpenAI backend
- **THEN** capture is configured for 24 kHz mono and delivers frames at that rate

#### Scenario: Local backend drives capture configuration
- **WHEN** a session starts on the local Whisper backend
- **THEN** capture is configured for 16 kHz mono and delivers frames of exactly 512 samples

### Requirement: Capture emits raw mono float frames
The capture worker SHALL downmix to mono, resample from the device rate to `AudioSpec.sample_rate`, and emit fixed-size frames of `AudioSpec.frame_samples` `f32` samples. It SHALL NOT perform PCM16 conversion, chunking, or base64 encoding. Device configuration selection SHALL target `AudioSpec.sample_rate` using the existing preference order: mono at the target rate, then any channel count at the target rate, then the mono fallback, then any fallback.

#### Scenario: Multi-channel device audio is downmixed and resampled
- **WHEN** the input device delivers stereo audio at 48 kHz and the backend requires 16 kHz
- **THEN** the worker emits mono frames at 16 kHz containing the averaged channels

#### Scenario: Frames are exactly the requested size
- **WHEN** device callbacks deliver buffers that do not align with `frame_samples`
- **THEN** the worker buffers across callbacks and emits only frames of exactly `frame_samples` samples

#### Scenario: Trailing audio is flushed at stop
- **WHEN** capture stops with fewer than `frame_samples` samples buffered
- **THEN** the remainder is zero-padded to a full frame, emitted, and followed by the capture-stopped session state event

#### Scenario: Full-queue behaviour is retained
- **WHEN** the bounded sample queue between the device callback and the worker is full
- **THEN** the callback drops the buffer without blocking and an `Error` event reports that microphone audio was dropped

### Requirement: The OpenAI backend owns the remote wire format
PCM16 conversion, 40 ms chunking, and base64 encoding SHALL be performed inside the OpenAI backend rather than in the capture worker. The bytes transmitted over the WebSocket SHALL be unchanged by this refactor: base64-encoded mono PCM16 at 24 kHz in 40 ms chunks.

#### Scenario: Encoded output is byte-identical
- **WHEN** a known sequence of `f32` frames at 24 kHz is passed through the OpenAI backend
- **THEN** the resulting base64 chunks are identical to those the previous capture-worker implementation produced for the same input

#### Scenario: Negative samples keep their scaling
- **WHEN** a frame contains negative sample values
- **THEN** they are scaled by 32768.0 and positive values by 32767.0, as before

### Requirement: The audio channel serves both backends without a runtime
The audio channel between capture and backend SHALL carry `Vec<f32>` frames and SHALL be usable from plain threads without a tokio runtime. The local path SHALL consume frames by blocking receive on a dedicated thread and SHALL NOT require a tokio runtime to run a session.

#### Scenario: Local session runs without a runtime
- **WHEN** a local session starts
- **THEN** audio frames reach the inference thread and events reach the UI without any tokio runtime being spawned for the session

#### Scenario: Remote session is unaffected
- **WHEN** a remote session starts
- **THEN** the existing tokio task consumes frames from the same channel type and streams them to the WebSocket
