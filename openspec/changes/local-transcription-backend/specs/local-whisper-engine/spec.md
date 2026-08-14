## ADDED Requirements

### Requirement: Local engines are pluggable behind a trait
The application SHALL define a `LocalEngine` trait exposing at minimum `audio_spec`, `start`, `feed_frame`, and `stop`, with events emitted on the existing UI event channel. The remote/local choice SHALL be represented as an enum rather than a shared trait object, and additional engines SHALL be selectable through the `local.engine` setting without changes to the session, capture, or UI layers.

#### Scenario: Whisper is the engine in this change
- **WHEN** `local.engine` is `"whisper"`
- **THEN** the Whisper engine is constructed and used for the session

#### Scenario: Unknown engine is rejected clearly
- **WHEN** `local.engine` names an engine this build does not implement
- **THEN** the session refuses to start with a message naming the unsupported engine, and the remote path remains usable

### Requirement: Local transcription runs off the UI and async threads
The local engine SHALL run inference on a dedicated thread, never on the UI thread or a tokio runtime. `stop` SHALL return promptly even while an inference is in flight, and the engine SHALL finalize any open segment before the session reports `disconnected`.

#### Scenario: UI stays responsive during inference
- **WHEN** a decode is running
- **THEN** the UI continues to repaint and the microphone level meter keeps updating

#### Scenario: Stop finalizes the open segment
- **WHEN** the user stops a session while speech is buffered in an open segment
- **THEN** the engine performs a final decode, emits `SourceCompleted` for that segment, and then emits the `disconnected` session state

#### Scenario: Stop is prompt under load
- **WHEN** the user stops a session while an inference is in flight
- **THEN** the session ends without waiting for further decodes to be queued

### Requirement: Speech is segmented using voice activity detection
The engine SHALL use VAD to open a segment on speech onset, re-decode the open segment periodically to emit provisional text, and close it on sustained silence or when it reaches a maximum length. Long silence SHALL NOT be submitted to the recognizer. Defaults SHALL be a 500 ms partial interval, 600 ms silence threshold, and 15 s maximum segment length.

#### Scenario: Provisional text appears during speech
- **WHEN** the user speaks continuously for three seconds
- **THEN** the engine emits `SourceUpdate` events for the open segment at approximately the partial interval, each carrying the current full hypothesis

#### Scenario: Segment closes after silence
- **WHEN** the user stops speaking and silence persists past the silence threshold
- **THEN** the engine performs a final decode, emits `SourceCompleted`, and closes the segment

#### Scenario: Long speech is force-cut
- **WHEN** a single segment reaches the maximum segment length without a silence break
- **THEN** the engine finalizes that segment with `SourceCompleted` and immediately opens a new segment so transcription continues

#### Scenario: Silence is not decoded
- **WHEN** the microphone is open but no speech is detected
- **THEN** no inference is run and no transcript events are emitted

#### Scenario: Overlapping decodes are skipped
- **WHEN** a partial interval elapses while the previous decode is still running
- **THEN** that partial decode is skipped rather than queued, and no backlog accumulates

### Requirement: Local sessions report their lifecycle state
The engine SHALL emit `SessionState` events using the values `local.loading`, `connecting`, `local.listening`, `local.processing`, and `disconnected`, and the UI SHALL render each as human-readable status text. A running local session SHALL be represented by the existing transcribing live state.

#### Scenario: Model load is visible
- **WHEN** a local session starts and the model is being loaded into memory
- **THEN** the status line reports that the model is loading, before any audio is captured

#### Scenario: Idle and active states are distinguished
- **WHEN** the session is open and no speech is present, and then speech begins and is decoded
- **THEN** the status line moves from a listening state to a transcribing state and back

#### Scenario: No unmapped state strings
- **WHEN** any local session state is emitted
- **THEN** the UI displays a readable label rather than a raw state identifier

### Requirement: Device selection falls back to CPU
The engine SHALL honour the `local.device` setting, with `"auto"` selecting the best acceleration available in the current build and machine. When the requested device is unavailable, the engine SHALL fall back to CPU, warn the user, and continue rather than failing the session.

#### Scenario: Auto selects available acceleration
- **WHEN** `local.device` is `"auto"` on a machine and build with Metal support
- **THEN** the engine initialises with Metal acceleration

#### Scenario: Unavailable device falls back
- **WHEN** `local.device` is `"cuda"` on a machine or build without CUDA support
- **THEN** the engine initialises on CPU and the status line shows a warning explaining the fallback

### Requirement: Local engine support is compiled in optionally
Local engine support SHALL be behind a cargo feature so the default build has no additional native dependencies. A build without the feature SHALL disable the local option in the backend selector with an explanation, and SHALL run in remote mode with a clear message if the settings file selects the local backend.

#### Scenario: Default build excludes native dependencies
- **WHEN** the project is built without the local feature
- **THEN** the build requires no additional native toolchain and the remote path works unchanged

#### Scenario: Local selection in a build without support
- **WHEN** a build without the local feature loads a settings file with `backend = "local"`
- **THEN** the application runs in remote mode, the backend selector shows the local option as unavailable, and the reason is reported to the user rather than a session failing at start

### Requirement: Recognition failures do not end the session
A failed decode SHALL emit an `Error` event and drop only the affected segment; the session SHALL remain open and continue transcribing subsequent speech.

#### Scenario: A decode fails mid-session
- **WHEN** an inference call returns an error for one segment
- **THEN** an `Error` event is surfaced, the segment is dropped, and the next speech segment is transcribed normally

#### Scenario: Latency targets under normal operation
- **WHEN** a user speaks a known phrase and stops
- **THEN** provisional text appears within approximately one second of speech starting and final text within approximately 0.5–2 seconds of speech ending
