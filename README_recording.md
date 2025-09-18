# dict-ai-te Web Recording Overview

This note documents the browser ↔ server pipeline that powers the web UI recording button.

## Flow summary

1. **Start** – when the user clicks `Toggle Recording` the browser requests `getUserMedia`, negotiates a `MediaRecorder` mime type, and POSTs to `POST /api/record/start`. The server allocates a UUID session, prepares `tmp/recordings/<session_id>.webm`, and tracks simple counters (expected seq, chunk count, bytes).
2. **Stream uploads** – each `dataavailable` event posts to `POST /api/record/append` with multipart fields `session_id`, `seq`, and `chunk`. The server appends bytes in order (rejecting gaps), updates counters, and keeps everything on disk.
3. **Stop** – calling `MediaRecorder.stop()` flushes the final chunk. Once the queue drains the browser calls `POST /api/record/finalize` with the last session metadata (mode, language, target). The Flask endpoint converts the reconstructed file to wav, invokes `dictaite_core.services.transcribe()` and, if requested, `translate()` using the exact same function signatures the GTK UI already consumes.
4. **Cleanup** – on success the temp file is deleted; on recoverable errors the session stays open so the UI can retry. Cancelling (`POST /api/record/cancel`) removes both the session and the temp artefacts.

### Client state machine

The browser module in `dictaite/ui_web/static/js/record.js` manages these states:

| State        | Description                                                                  |
|--------------|------------------------------------------------------------------------------|
| `idle`       | Ready to start. Controls enabled.                                            |
| `preparing`  | Microphone prompt pending / session allocation in flight.                    |
| `recording`  | MediaRecorder active; timer + VU meter ticking; button shows Stop.           |
| `uploading`  | Recorder stopped, outstanding chunks being flushed.                          |
| `processing` | Server finalising and calling OpenAI.                                        |
| `done`       | Transcript (or translation) returned and rendered.                           |
| `error`      | Any chunk/finalise failure. Retry/Cancel buttons are displayed.              |

Each state updates the button label, ARIA attributes, timer (`MM:SS`), and status messages (`Ready`, `Recording...`, `Uploading...`, `Processing...`, `Done`). `Escape` always stops an active recording, while `Space` toggles recording when focus is outside of inputs.

### Server endpoints

| Endpoint                  | Purpose                                                         |
|---------------------------|-----------------------------------------------------------------|
| `POST /api/record/start`  | Allocate UUID session, create empty `tmp/recordings/<id>.webm`. |
| `POST /api/record/append` | Append chunk bytes, enforcing sequential `seq` ordering.        |
| `POST /api/record/finalize` | Convert assembled file, run `transcribe()` then optional `translate()`, log duration and backend latency, return JSON payload. |
| `POST /api/record/cancel` | Remove session metadata and temp file.                          |

Log entries include the session id, chunk count, byte size, duration and API latency to aid telemetry.

## Local testing checklist

1. Run the Flask app (e.g. `uv run dictaite ui web`). Open the UI at http://127.0.0.1:8000/ (or whichever port you configured).
2. Record a short phrase. Watch the devtools network tab for `/api/record/start`, `/append`, `/finalize`. Verify a transcript appears, and that the server log includes a `Finalized recording` line.
3. Switch Mode to **Translate**, select a target language, then record again. Confirm translated text returns.
4. Deny microphone permission to confirm the UI reports an actionable error and stays idle (no network requests).
5. Simulate a chunk failure (e.g. throttle network or kill `/append`) and try the Retry/Cancel buttons to ensure the session recovers or cleans up.
6. Automated regression tests: `uv run pytest` (ensures the helpers still integrate with OpenAI mocks).

## Implementation notes

- Temporary recordings live under `dictaite/ui_web/tmp/recordings/` and are removed automatically on success or explicit cancel.
- The browser prefers `audio/webm;codecs=opus`, automatically falling back when unsupported.
- For accessibility the record button toggles `aria-pressed`, timer & status lines are `aria-live="polite"`, and keyboard shortcuts match the footer hint.
- No backend APIs were altered; the web UI calls the same `dictaite_core.services.transcribe/translate` helpers as the GTK client.

