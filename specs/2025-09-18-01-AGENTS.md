# Prompt: Add real voice recording to doctaite.ui_web

# Role

You are **Senior Full-Stack Engineer (Flask + Vanilla JS)** working on **DictAIT – Web (Flask)**. Your task is to implement working microphone capture and recording controls in the main screen and reuse the existing **audio backend used by the GTK client**. You must not modify that backend’s API or add a new streaming protocol.

# North Star

Enable users to click **Toggle Recording** → record speech → click **Stop Recording** → the recorded audio file is saved server-side and then submitted to the **existing** transcription or translation endpoints used by the GTK client. The UI must show state and a running timer.

# Ground Rules

* **Do not change** the AI/audio backend the GTK version uses. Call it exactly the same way, with the same parameters and content types.
* **No new streaming interface.** If you stream chunks from the browser, you must **reassemble** them in the Flask web app and then call the existing backend as a file upload, just like GTK does after it records.
* Keep dependencies minimal: Vanilla JS in the browser. Flask + standard Python libs server-side. Avoid heavy front-end frameworks.
* Security: request mic permission only on user action, never on page load. Stop and release the mic on “Stop Recording,” page hide, or navigation.
* Accessibility: button must be focusable and operable by keyboard. Aria label reflects state.

# Repo Assumptions (adapt if paths differ)

* Flask app package: `web/` with `app.py`, templates in `web/templates/`, static assets in `web/static/`.
* GTK client shows how to call the backend. Look for a module like `core/audio_client.py` or similar that performs the POST to the backend. **Reuse identical call semantics.**

# What to Build

1. **Main screen wiring**

   * There is already a **big button** “Toggle Recording” under the **Settings** link, with caption text “Press to Start Recording,” plus a **timer** under it.
   * Implement click handler that:

     * On idle state: requests mic access via `navigator.mediaDevices.getUserMedia({ audio: true })`, creates a `MediaRecorder`, starts recording, and starts the timer. Update label to **Stop Recording** and the caption to **Recording…**.
     * On recording state: stops recorder, stops timer, finalizes client-side state, and triggers upload completion flow.

2. **Chunked upload to server**

   * Use `MediaRecorder` with `mimeType` negotiated from the browser, commonly `audio/webm` or `audio/webm;codecs=opus`.
   * On `dataavailable`, **POST each chunk** to Flask endpoint `/api/record/append` as `multipart/form-data` with fields:

     * `session_id` (UUID generated once per recording),
     * `seq` (monotonic integer),
     * `chunk` (the Blob).
   * Add an endpoint `/api/record/start` to allocate a `session_id` and create a temp file path, and `/api/record/finalize` to close and return the final server file path.
   * Server behavior:

     * Append chunk bytes to a temp file under `tmp/recordings/<session_id>.webm` in arrival order. Use a simple append protocol keyed by `session_id`, validating `seq`.

     * On finalize, ensure the file is closed and accessible to the following step.

   > Note: This is **not a new streaming protocol** for the AI backend. It is only a browser→Flask upload mechanism, after which you will call the **existing** backend with a single file, exactly as the GTK client already does.

3. **Reuse the existing backend invocation**

   * After finalize, call the **same function or HTTP endpoint** the GTK client uses for transcription or translation.
   * Preserve:

     * **Content type**,
     * **File format** expectations,
     * **Query/body parameters**,
     * **Auth headers or API keys**,
     * **Model selection** and options.
   * Provide a **user option** to choose **Transcribe** vs **Translate** before recording begins. Default to Transcribe if none chosen. Do not change backend behavior.

4. **UI feedback**

   * Timer format `MM:SS`, updates every second while recording.
   * Button text toggles between **Toggle Recording** → **Stop Recording**.
   * Show subtle status text: “Ready,” “Recording…,” “Uploading…,” “Processing…,” “Done.”
   * On success, render the returned transcript or translation in the existing result panel or a simple `<pre>`.

5. **Edge cases**

   * Denied microphone permission → show actionable message and revert to idle.
   * If any chunk upload fails → offer Retry or Cancel. Cancel deletes temp file via `/api/record/cancel?session_id=…`.
   * On browser that lacks `MediaRecorder` for the chosen mime type, fall back to default without specifying `mimeType`.

# Files to Add or Modify (suggested)

* `web/templates/main.html`

  * Add `id="recordBtn"`, `id="recordCaption"`, `id="recordTimer"`, and a select or toggle for Transcribe vs Translate.
  * Include `<script src="{{ url_for('static', filename='js/record.js') }}"></script>`.
* `web/static/js/record.js`

  * Implement state machine: `idle | recording | uploading | processing | done | error`.
  * Implement `startRecording()`, `stopRecording()`, `postChunk(seq, blob)`, `finalizeRecording()`, `updateUI(state)`, and timer helpers.
* `web/app.py`

  * Endpoints:

    * `POST /api/record/start` → returns `session_id`.
    * `POST /api/record/append` → appends chunk bytes to `tmp/recordings/{session_id}.webm`.
    * `POST /api/record/finalize` → closes file and triggers **existing backend call** using the same code path as GTK. Returns JSON with transcript or translation.
    * `POST /api/record/cancel` → deletes temp file.
  * A thin wrapper that **reuses** the backend client used by GTK, e.g., import `from core.audio_client import transcribe_file, translate_file` and call those functions unchanged.

# Acceptance Criteria

* Clicking **Toggle Recording** starts mic capture, changes button to **Stop Recording**, shows a running timer, and begins chunk uploads.
* On **Stop Recording**, the temp file is finalized server-side, then submitted to the **same backend endpoint** used by GTK. No parameter or format drift.
* A successful transcript or translation appears in the UI. Errors are surfaced with plain language and a retry option.
* No changes to the AI/audio backend code or API. The Flask web app is the only place you add endpoints.
* Basic keyboard a11y works: Space/Enter toggles the button, Escape stops recording.

# Test Plan

1. **Happy path**

   * Record 5–10 seconds. Verify chunks arrive in order, final file exists with expected size, backend receives the exact file and returns text.
2. **Permission denied**

   * Deny mic access. Verify idle state with helpful prompt and no network calls.
3. **Chunk failure**

   * Simulate one `append` failure. Ensure UI shows retry. After retry, file finalizes correctly.
4. **MIME compatibility**

   * Chrome and Firefox on desktop: verify `audio/webm` or auto mime. Confirm backend accepts the reassembled file. If backend needs WAV, convert server-side before calling it, but **do not** change backend API.
5. **Translate vs Transcribe**

   * Toggle mode and verify the correct existing backend function is called.

# Non-Goals

* No WebRTC or WebSocket low-latency streaming to the AI backend.
* No backend API changes. No new models or parameters.
* No SPA framework introduction.

# Telemetry and Logs

* Server logs per recording: `session_id`, chunk counts, total bytes, duration, backend latency.
* Client console logs gated behind a `DEBUG_RECORDING` flag.

# Deliverables

* PR with the code changes listed above.
* A short `README_recording.md` explaining the flow and how to run a local test.
* A 2-minute screen capture showing the happy path.

# Step-by-Step Plan (execute in order)

1. Inspect GTK client code to find **exact** backend call used for file transcription and translation. Extract the minimal reusable wrapper.
2. Implement Flask endpoints for start, append, finalize, cancel, with simple file append logic keyed by `session_id`.
3. Implement `record.js` with `MediaRecorder` and chunk posts. Add state machine, timer, and button toggling.
4. Wire UI elements and minimal styles. Verify a11y.
5. End-to-end test. Fix MIME or format conversion on the **Flask side** if the backend needs WAV or PCM.
6. Add logs, README, and the demo recording.

