# Specification: Convert dict-ai-te into a live-first transcription and translation app

## Context


The existing application is based on a classic batch workflow:

1. Record audio.
2. Stop recording.
3. Upload/process the completed audio file.
4. Transcribe.
5. Optionally translate.
6. Show final text.

This specification replaces that workflow completely.

The new application must be live-first:

1. Start microphone stream.
2. Send audio continuously to OpenAI Realtime.
3. Show transcript deltas while the user is speaking.
4. Optionally show translated text while the user is speaking.
5. Let the user edit, copy, save, or reuse the resulting text.

There should no longer be a “classic recording” mode in the user interface or code path, except where small internal utilities are still reusable.

## Goal

Fully convert both the Python and Rust versions of `dict-ai-te` to use:

- **Live transcription mode** as the default and primary behavior.
- **Optional live translation mode** as an additional layer on top of live transcription.

The application should no longer ask the user to record, stop, and wait for post-processing.

The central interaction becomes:

```text
Start → speak → text appears continuously → optionally translate live → stop
```

## Product model

The app has one primary mode:

```text
Live transcription
```

It has one optional feature:

```text
Live translation
```

This means the user should not select between “classic” and “live”. Live is the product.

The UI should expose this as:

```text
[ Start Listening ]

[✓] Translate live
Target language: Spanish

Source transcript:
...

Translated text:
...
```

When live translation is disabled, only the source transcript is shown.

When live translation is enabled, both source and translated text are shown.

## Removed behavior

Remove or deprecate the following user-facing behavior:

* Record full audio file.
* Stop recording before transcription starts.
* Upload completed recording to `/api/transcribe`.
* Batch Whisper file transcription.
* Batch translation after transcription.
* “Classic mode”.
* Any mode selector containing `Classic`.

Internal code may keep small reusable pieces temporarily if they are useful, but the user-facing app must not expose the old workflow.

## New application behavior

### Default: Live transcription

When the user presses `Start Listening`:

1. The app opens a realtime transcription session.
2. The app starts microphone capture.
3. The app streams audio chunks continuously.
4. The UI updates as transcript deltas arrive.
5. Finalized segments replace or confirm partial text.

When the user presses `Stop`:

1. Microphone capture stops.
2. The realtime session closes cleanly.
3. The accumulated transcript remains editable.
4. The user can copy, save, clear, or continue editing the text.

### Optional: Live translation

When the user enables `Translate live`:

1. The app opens a realtime translation session instead of a transcription-only session.
2. The app streams microphone audio continuously.
3. The UI shows both:

   * Source transcript.
   * Translated transcript.

The app does not need to play translated audio in the first implementation. If the API emits translated audio deltas, they must be ignored safely.

## OpenAI model usage

Use:

```text
gpt-realtime-whisper
```

for live transcription.

Use:

```text
gpt-realtime-translate
```

for live translation.

Important distinction:

* `gpt-realtime-whisper` is for live speech-to-text.
* `gpt-realtime-translate` is for live speech translation.

Do not try to implement live translation by first streaming transcription and then repeatedly calling a separate text translation endpoint. That would increase latency and create unstable partial translations. Use the realtime translation model directly when translation is enabled.

## Conceptual architecture

### Without translation

```text
Microphone
  ↓
PCM16 mono 24 kHz chunks
  ↓
OpenAI Realtime transcription session
  ↓
Transcript deltas
  ↓
Live transcript UI
```

### With translation

```text
Microphone
  ↓
PCM16 mono 24 kHz chunks
  ↓
OpenAI Realtime translation session
  ↓
Source transcript deltas
Translated transcript deltas
  ↓
Two-pane live UI
```

## New shared state model

Remove the old concept of `Classic`.

Use:

```python
class LiveMode(str, Enum):
    TRANSCRIPTION = "transcription"
    TRANSLATION = "translation"
```

or in Rust:

```rust
enum LiveMode {
    Transcription,
    Translation,
}
```

The mode is derived from the translation toggle:

```text
Translate live disabled → LiveMode.Transcription
Translate live enabled  → LiveMode.Translation
```

There should not be a visible “mode selector” unless the app needs it internally for debugging.

## Settings

Update the settings model.

There is no currently tracked `dictation_mode` setting in this repository. If an
older user configuration contains it, ignore it during migration:

```json
{
  "dictation_mode": "classic"
}
```

Add realtime settings while preserving compatibility with the existing
`translate_by_default` and `default_target_language` settings. The UI may expose
these under the clearer live names, but the migration should not silently break
existing `~/.dictaite/settings.json` files.

Recommended persisted shape:

```json
{
  "live_translation_enabled": false,
  "realtime_transcription_model": "gpt-realtime-whisper",
  "realtime_translation_model": "gpt-realtime-translate",
  "source_language": "auto",
  "target_language": "es",
  "realtime_pcm_sample_rate": 24000,
  "realtime_vad_enabled": true,
  "realtime_vad_threshold": 0.5,
  "realtime_vad_prefix_padding_ms": 300,
  "realtime_vad_silence_duration_ms": 500
}
```

Keep existing settings for:

* API key handling.
* UI language.
* default target language, if already present.
* save/export preferences.

Mapping from existing settings:

* `translate_by_default` → `live_translation_enabled`.
* `default_target_language` → `target_language`.
* `default_language` → `source_language`, using `"auto"` when unset.

The repository currently defaults `default_target_language` to `"en"`. Changing
the live translation default to `"es"` is a product decision and should be made
explicitly. If no such decision is intended, preserve the existing default.

Remove or ignore settings related to:

* recording file format.
* batch upload transcription.
* post-recording transcription.
* classic mode.

## Audio requirements

All live modes should use the same audio pipeline.

Input:

* Microphone stream.

Processing:

* Downmix to mono.
* Resample to 24 kHz.
* Convert to signed 16-bit little-endian PCM.
* Chunk into approximately 20 to 100 ms blocks.
* Base64 encode chunks before sending to OpenAI.

Do not use ffmpeg for live mode.

ffmpeg-related code should be removed unless it is still needed for another explicit feature.

## Event handling

### Live transcription events

Handle:

```text
conversation.item.input_audio_transcription.delta
conversation.item.input_audio_transcription.completed
```

On delta:

* Append the delta to the partial segment identified by `item_id`.
* Show it immediately in the UI.

On completed:

* Replace the partial segment for `item_id` with the final transcript.
* Move it into the finalized transcript buffer.
* Avoid duplicate text.

Important:

* Do not assume completion events arrive in the same order as speech.
* Track segments by `item_id`.

### Live translation events

Handle:

```text
session.input_transcript.delta
session.output_transcript.delta
session.output_audio.delta
```

On `session.input_transcript.delta`:

* Append to the source transcript.

On `session.output_transcript.delta`:

* Append to the translated transcript.

On `session.output_audio.delta`:

* Ignore safely for the first implementation.

Unknown events:

* Must not crash the app.
* Log only event type and minimal metadata.
* Do not log audio payloads.

## Python implementation

### Remove or replace old endpoints

Remove user-facing dependence on:

```text
POST /api/transcribe
```

This endpoint may be deleted, or temporarily left unused during migration, but the UI must not call it.

Remove JavaScript behavior based on:

```text
MediaRecorder → Blob → upload → final JSON response
```

Replace with:

```text
getUserMedia → AudioWorklet/AudioContext → PCM chunks → WebSocket → transcript deltas
```

### New Python module structure

Create or reorganize:

```text
dictaite_core/realtime/
    __init__.py
    models.py
    audio.py
    openai_ws.py
    transcription_client.py
    translation_client.py
    events.py
```

Keep shared realtime logic in `dictaite_core`, not in a UI package. The Flask
and GTK packages should contain only UI adapters, route handlers, and widget
integration code. This preserves the existing project architecture where
`dictaite_core` owns reusable service logic.

### `models.py`

```python
from dataclasses import dataclass
from enum import Enum
from typing import Optional


class LiveMode(str, Enum):
    TRANSCRIPTION = "transcription"
    TRANSLATION = "translation"


class RealtimeConnectionState(str, Enum):
    DISCONNECTED = "disconnected"
    CONNECTING = "connecting"
    CONNECTED = "connected"
    RECONNECTING = "reconnecting"
    ERROR = "error"


@dataclass
class RealtimeSettings:
    transcription_model: str = "gpt-realtime-whisper"
    translation_model: str = "gpt-realtime-translate"
    source_language: Optional[str] = None
    target_language: str = "es"
    sample_rate: int = 24000
    vad_enabled: bool = True
    vad_threshold: float = 0.5
    vad_prefix_padding_ms: int = 300
    vad_silence_duration_ms: int = 500
```

### `events.py`

```python
from dataclasses import dataclass
from typing import Optional


@dataclass
class SourceTranscriptDelta:
    item_id: Optional[str]
    delta: str


@dataclass
class SourceTranscriptCompleted:
    item_id: str
    text: str


@dataclass
class TranslationTranscriptDelta:
    delta: str


@dataclass
class RealtimeStateChanged:
    state: str


@dataclass
class RealtimeError:
    message: str
```

The UI should consume these normalized events, not raw OpenAI JSON.

### `openai_ws.py`

Implement a low-level async WebSocket client.

Responsibilities:

* Connect to OpenAI Realtime.
* Send session configuration.
* Send audio chunks.
* Receive events.
* Close cleanly.
* Never expose or log the API key.

Use `websockets` or equivalent.

### Live transcription WebSocket session

Endpoint:

```text
wss://api.openai.com/v1/realtime?model=gpt-realtime-whisper
```

Send session configuration:

```json
{
  "type": "session.update",
  "session": {
    "type": "transcription",
    "audio": {
      "input": {
        "format": {
          "type": "audio/pcm",
          "rate": 24000
        },
        "transcription": {
          "model": "gpt-realtime-whisper"
        },
        "turn_detection": {
          "type": "server_vad",
          "threshold": 0.5,
          "prefix_padding_ms": 300,
          "silence_duration_ms": 500
        }
      }
    }
  }
}
```

If a source language is selected and is not `auto`, include:

```json
"language": "en"
```

inside `transcription`.

Send audio chunks:

```json
{
  "type": "input_audio_buffer.append",
  "audio": "<base64-pcm16>"
}
```

### Live translation WebSocket session

Endpoint:

```text
wss://api.openai.com/v1/realtime/translations?model=gpt-realtime-translate
```

Send session configuration:

```json
{
  "type": "session.update",
  "session": {
    "audio": {
      "output": {
        "language": "es"
      }
    }
  }
}
```

Send audio chunks:

```json
{
  "type": "session.input_audio_buffer.append",
  "audio": "<base64-pcm16>"
}
```

### Flask web UI

Replace the current recording/upload flow.

Add server-side WebSocket endpoints:

```text
/ws/live/transcribe
/ws/live/translate
```

The current project depends on Flask but not on a WebSocket server extension.
Choose and add one explicit server approach before implementation, for example
`Flask-Sock`/`simple-websocket`, `flask-socketio`, or a migration to an ASGI
framework such as Quart. The chosen option must support bidirectional streaming
without blocking the Flask worker.

Recommended implementation:

* Browser captures microphone audio.
* Browser converts to PCM16 mono 24 kHz.
* Browser sends base64 PCM chunks to Flask WebSocket.
* Flask connects to OpenAI Realtime using the server-side API key.
* Flask forwards audio chunks.
* Flask forwards normalized transcript events back to the browser.

Do not send the OpenAI API key to the browser.

This proxy architecture is acceptable for this application because Flask keeps
the standard OpenAI API key server-side. It is more complex than a direct browser
WebRTC session because the browser must convert audio to PCM16 and the server
must relay every chunk. Keep this tradeoff explicit in the implementation notes.

Browser-to-Flask messages:

```json
{
  "type": "start",
  "sourceLanguage": "auto",
  "targetLanguage": "es"
}
```

`sourceLanguage` should be treated as an optional transcription hint only when
the selected OpenAI session supports it. For live translation, the target output
language is required; source-language support should be verified against the API
before sending any non-documented field. If unsupported, keep source language as
UI metadata and rely on automatic source-language detection.

```json
{
  "type": "audio",
  "audio": "<base64-pcm16>"
}
```

```json
{
  "type": "stop"
}
```

Flask-to-browser messages:

```json
{
  "type": "state",
  "state": "connected"
}
```

```json
{
  "type": "source_delta",
  "itemId": "item_123",
  "delta": "hello"
}
```

```json
{
  "type": "source_completed",
  "itemId": "item_123",
  "text": "hello, this is a test"
}
```

```json
{
  "type": "translation_delta",
  "delta": "hola"
}
```

```json
{
  "type": "error",
  "message": "Realtime session failed"
}
```

### Web UI layout

Replace old record/upload interface with:

```text
Header
  dict-ai-te

Controls
  Start Listening / Stop
  Translate live [toggle]
  Target language [selector]
  Source language [auto/default selector, optional]
  Connection state

Main content
  Source transcript editor

If translation enabled
  Translated transcript editor

Footer/actions
  Copy source
  Copy translation
  Save source
  Save translation
  Clear
```

The transcript areas must remain editable. Live incoming text should append without destroying user edits where possible. If this becomes complex, use two areas:

* Live buffer, read-only while streaming.
* Editable final text after stop.

Prefer editable continuous text if feasible.

### Python GTK desktop

Replace the old recording workflow with live listening.

UI:

* `Start Listening`
* `Stop`
* `Translate live` checkbox
* Target language selector
* Connection status
* Source transcript editor
* Translation editor, visible only when translation is enabled

Implementation:

* Start microphone stream immediately when the user presses `Start Listening`.
* Feed PCM chunks into an async queue.
* Run OpenAI realtime client in a background asyncio loop.
* Send normalized events back to GTK.
* Update GTK widgets only on the GTK main thread.

No “recording complete” phase should exist.

No “uploading audio” status should exist.

Use statuses like:

* `Connecting`
* `Listening`
* `Transcribing`
* `Translating`
* `Disconnected`
* `Error`

## Rust implementation

### Remove classic mode

Remove or stop exposing:

* record-to-file workflow.
* batch upload transcription.
* old `Classic` mode enum variants.
* UI labels such as `Record`, `Upload`, `Transcribe after recording`.

The Rust app should be live-first.

### Module structure

Add:

```text
src/realtime/
    mod.rs
    models.rs
    audio.rs
    openai_ws.rs
    transcription.rs
    translation.rs
```

### `models.rs`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveMode {
    Transcription,
    Translation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

#[derive(Clone, Debug)]
pub struct RealtimeSettings {
    pub transcription_model: String,
    pub translation_model: String,
    pub source_language: Option<String>,
    pub target_language: String,
    pub sample_rate: u32,
    pub vad_enabled: bool,
    pub vad_threshold: f32,
    pub vad_prefix_padding_ms: u32,
    pub vad_silence_duration_ms: u32,
}

#[derive(Clone, Debug)]
pub enum RealtimeUiEvent {
    State(ConnectionState),
    SourceDelta {
        item_id: Option<String>,
        delta: String,
    },
    SourceCompleted {
        item_id: String,
        text: String,
    },
    TranslationDelta {
        delta: String,
    },
    Error {
        message: String,
    },
}
```

### Rust audio pipeline

Use `cpal` for microphone input.

The audio callback must:

* avoid network calls.
* avoid heavy processing where possible.
* push samples or prepared PCM chunks into a bounded channel.
* return quickly.

A worker task should:

* downmix.
* resample.
* convert to PCM16.
* chunk.
* send to the realtime task.

If this is too complex initially, perform simple conversion in the callback only when safe, but avoid blocking.

### Rust WebSocket implementation

Use `tokio-tungstenite`.

Add the required async/runtime dependencies explicitly, including `tokio`,
`tokio-tungstenite`, `futures-util`, and any audio resampling crate selected for
PCM conversion. The current Rust crate only has blocking `reqwest`; realtime
streaming should run on a Tokio runtime owned by the application.

Implement:

```rust
pub async fn run_live_transcription(
    settings: RealtimeSettings,
    audio_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
    ui_tx: crossbeam_channel::Sender<RealtimeUiEvent>,
) -> Result<(), RealtimeError>
```

and:

```rust
pub async fn run_live_translation(
    settings: RealtimeSettings,
    audio_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
    ui_tx: crossbeam_channel::Sender<RealtimeUiEvent>,
) -> Result<(), RealtimeError>
```

### Rust egui UI

Replace the old UI controls with:

```text
Start Listening / Stop

[ ] Translate live

Target language: [selector]

Connection: connected/listening/error

Source transcript:
[editable text area]

Translated transcript:
[editable text area, visible only if translation enabled]
```

Internal app state:

```rust
live_mode: LiveMode,
translation_enabled: bool,
connection_state: ConnectionState,
source_text: String,
translated_text: String,
partial_segments: HashMap<String, String>,
final_segments: Vec<(String, String)>,
audio_tx: Option<tokio::sync::mpsc::Sender<Vec<u8>>>,
ui_rx: crossbeam_channel::Receiver<RealtimeUiEvent>,
stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
```

When translation toggle is disabled:

```rust
live_mode = LiveMode::Transcription;
```

When enabled:

```rust
live_mode = LiveMode::Translation;
```

### Applying incoming UI events

On `SourceDelta`:

* If `item_id` exists, update `partial_segments[item_id]`.
* If no `item_id`, append delta to a temporary live buffer.
* Refresh the visible source transcript.

On `SourceCompleted`:

* Replace the partial segment for that `item_id`.
* Append final text to `final_segments`.
* Rebuild source text without duplication.

On `TranslationDelta`:

* Append delta to `translated_text`.

On `Error`:

* Set `connection_state = Error`.
* Show error text.
* Keep accumulated transcript.

## Data model for transcript assembly

Use a segment model rather than a raw append-only string for source transcription.

Python:

```python
@dataclass
class TranscriptSegment:
    item_id: str
    partial: str = ""
    final: str | None = None
```

Rust:

```rust
struct TranscriptSegment {
    item_id: String,
    partial: String,
    final_text: Option<String>,
}
```

The visible transcript is:

```text
all finalized segments in known order
+
active partial segments
```

If event order is uncertain, preserve first-seen order using a list of item IDs.

Do not duplicate partial and final text.

For translation, simple append-only text is acceptable in the first implementation unless the API provides stable segment IDs.

## Error handling

The app must not crash on:

* WebSocket disconnect.
* OpenAI auth failure.
* model unavailable.
* unknown event type.
* missing `delta`.
* missing `item_id`.
* invalid JSON.
* microphone permission denied.
* audio device removed.
* network failure.
* empty audio chunks.

The UI should show:

* readable error message.
* disconnected state.
* accumulated transcript still available.

## Security

Never expose the standard OpenAI API key in browser JavaScript.

For Flask:

* Browser connects to Flask.
* Flask connects to OpenAI.
* Flask owns the API key.
* Browser only sees normalized transcript events.

Do not log:

* API keys.
* raw audio payloads.
* full transcript by default.

## Testing

### Python tests

Add tests for:

* PCM16 conversion.
* chunking.
* transcription delta parsing.
* transcription completed parsing.
* translation source delta parsing.
* translation output delta parsing.
* unknown event ignored safely.
* out-of-order `item_id` completion.
* Flask WebSocket bridge message normalization.

Mock OpenAI WebSocket traffic.

Do not call OpenAI in normal unit tests.

### Rust tests

Add tests for:

* sample conversion.
* chunking.
* transcription JSON event parsing.
* translation JSON event parsing.
* unknown event safety.
* duplicate prevention when partial text becomes final text.
* state transitions.

No OpenAI network calls in standard tests.

## README changes

Rewrite the README around the new live-first product.

Remove descriptions implying:

* record first.
* upload recording.
* process after recording.
* batch transcription as the main behavior.

New README positioning:

```markdown
dict-ai-te is a live dictation and translation app.

It streams microphone audio to OpenAI Realtime and shows text while you speak.

Core features:

- Live speech-to-text using `gpt-realtime-whisper`.
- Optional live translation using `gpt-realtime-translate`.
- Editable transcript.
- Copy and save source text.
- Copy and save translated text.
- Python and Rust implementations.
```

Add setup section:

```markdown
export OPENAI_API_KEY="..."
```

Add troubleshooting:

* Microphone permission denied.
* WebSocket blocked.
* Missing API key.
* No audio device.
* Realtime model unavailable.
* Translation target language unsupported.
* High latency due to network.

## Migration plan

1. Introduce realtime modules and event normalization.
2. Replace Flask UI recording/upload logic with WebSocket live streaming.
3. Remove `/api/transcribe` from active UI path.
4. Replace Python GTK recording controls with live listening controls.
5. Add Rust realtime WebSocket client.
6. Replace Rust egui recording controls with live listening controls.
7. Remove or quarantine obsolete batch code.
8. Update settings.
9. Update README.
10. Add tests.
11. Manual test:

* live transcription only.
* live translation to Spanish.
* stop/start repeatedly.
* microphone unavailable.
* network disconnect.
* invalid API key.

## Acceptance criteria

The implementation is complete when:

* There is no user-facing classic recording mode.
* The app starts transcribing while the user is still speaking.
* The user does not need to stop recording to see text.
* Live translation can be enabled or disabled.
* Translation mode shows both source and translated text.
* Python web version works.
* Python GTK version works if still maintained.
* Rust egui version works.
* The UI remains responsive while streaming.
* API key is not exposed in browser JavaScript.
* Unknown OpenAI events do not crash the app.
* Existing save/copy/edit functionality still works.


One architectural point: do **not** treat “live translation” as an optional post-processing step after live transcription. It should switch the realtime session type. Otherwise you will end up with worse latency, unstable partial text translation, and more application logic than necessary.
