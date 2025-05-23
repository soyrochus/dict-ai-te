"""dict-ai-te GTK recorder/transcriber."""

from __future__ import annotations

import os
import tempfile
import threading
import time

import numpy as np
import sounddevice as sd
import soundfile as sf
from dotenv import load_dotenv
from openai import OpenAI

import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib


class DictAiTeWindow(Gtk.ApplicationWindow):
    """Main application window."""

    def __init__(self, app: Gtk.Application) -> None:
        super().__init__(application=app, title="dict-ai-te")
        self.set_default_size(400, 500)
        load_dotenv()
        api_key = os.getenv("OPENAI_API_KEY")
        self.client = OpenAI(api_key=api_key) if api_key else None

        self.is_recording = False
        self.start_time: float | None = None
        self.elapsed_seconds = 0
        self.timer_thread: threading.Thread | None = None
        self.stream: sd.InputStream | None = None
        self.audio_frames: list[np.ndarray] = []
        
        # Icon paths (ensure the img folder is in your project directory)
        self.mic_icon_path = os.path.join("img", "microphone.svg")
        self.stop_icon_path = os.path.join("img", "stop.svg")

        self.build_ui()

    # ------------------------------------------------------------------
    # UI
    # ------------------------------------------------------------------

    def build_ui(self) -> None:
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6, margin_top=10,
                      margin_bottom=10, margin_start=10, margin_end=10)
        self.set_child(box)

        self.level = Gtk.LevelBar(min_value=0.0, max_value=1.0)
        box.append(self.level)


        self.icon_image = Gtk.Image.new_from_file(self.mic_icon_path)
        self.record_btn = Gtk.Button()
        self.record_btn.set_size_request(120, 120)
        self.record_btn.set_child(self.icon_image)
        self.record_btn.connect("clicked", self.toggle_recording)
        box.append(self.record_btn)
        

        self.status_label = Gtk.Label(label="Press to start recording")
        box.append(self.status_label)

        self.timer_label = Gtk.Label(label="00:00:00")
        box.append(self.timer_label)

        self.text_view = Gtk.TextView(wrap_mode=Gtk.WrapMode.WORD)
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_child(self.text_view)
        scrolled.set_vexpand(True)
        box.append(scrolled)

        self.save_btn = Gtk.Button(label="Save Transcript")
        self.save_btn.connect("clicked", self.save_transcript)
        box.append(self.save_btn)

    # ------------------------------------------------------------------
    # Recording control
    # ------------------------------------------------------------------

    def toggle_recording(self, button: Gtk.Button) -> None:
        if not self.is_recording:
            self.start_recording()
        else:
            self.stop_recording()

    def start_recording(self) -> None:
        self.is_recording = True
        self.start_time = time.time()
        self.status_label.set_text("Recording... Press to stop.")
        
         # --- Swap to stop icon ---
        self.icon_image.set_from_file(self.stop_icon_path)
        self.audio_frames.clear()
        try:
            self.stream = sd.InputStream(samplerate=16000, channels=1, callback=self.audio_callback)
            self.stream.start()
        except Exception as exc:  # pragma: no cover - runtime error path
            self.show_error("Recording error", str(exc))
            self.is_recording = False
            return
        self.timer_thread = threading.Thread(target=self.update_timer, daemon=True)
        self.timer_thread.start()

    def stop_recording(self) -> None:
        self.is_recording = False
        # --- Swap back to mic icon ---
        self.icon_image.set_from_file(self.mic_icon_path)
        if self.stream:
            try:
                self.stream.stop()
                self.stream.close()
            except Exception as exc:  # pragma: no cover - runtime error path
                self.show_error("Recording error", str(exc))
        self.stream = None
        self.status_label.set_text("Transcribing...")
        threading.Thread(target=self.transcribe_audio, daemon=True).start()

    def update_timer(self) -> None:
        while self.is_recording:
            self.elapsed_seconds = int(time.time() - self.start_time)
            h = self.elapsed_seconds // 3600
            m = (self.elapsed_seconds % 3600) // 60
            s = self.elapsed_seconds % 60
            timer_str = f"{h:02}:{m:02}:{s:02}"
            GLib.idle_add(self.timer_label.set_text, timer_str)
            time.sleep(1)

    # ------------------------------------------------------------------
    # Audio handling
    # ------------------------------------------------------------------

    def audio_callback(self, indata, frames, time_info, status) -> None:
        if status:
            print(status)
        self.audio_frames.append(indata.copy())
        amplitude = float(np.max(np.abs(indata)))
        GLib.idle_add(self.level.set_value, amplitude)

    def transcribe_audio(self) -> None:
        if not self.client:
            GLib.idle_add(self.show_error, "Configuration", "OpenAI API key not configured")
            GLib.idle_add(self.status_label.set_text, "Press to start recording")
            return
        try:
            audio = np.concatenate(self.audio_frames, axis=0)
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
                sf.write(tmp.name, audio, 16000)
                path = tmp.name
            with open(path, "rb") as f:
                response = self.client.audio.transcriptions.create(model="whisper-1", file=f)
            os.remove(path)
            transcript = response.text
            GLib.idle_add(self.display_transcript, transcript)
        except Exception as exc:  # pragma: no cover - network path
            GLib.idle_add(self.show_error, "Transcription error", str(exc))
        finally:
            GLib.idle_add(self.status_label.set_text, "Press to start recording")
            GLib.idle_add(self.level.set_value, 0.0)
            GLib.idle_add(self.timer_label.set_text, "00:00:00")

    def display_transcript(self, text: str) -> None:
        buffer = self.text_view.get_buffer()
        buffer.set_text(text)

    # ------------------------------------------------------------------
    # Saving
    # ------------------------------------------------------------------

    def save_transcript(self, button: Gtk.Button) -> None:
        buffer = self.text_view.get_buffer()
        start, end = buffer.get_bounds()
        text = buffer.get_text(start, end, True).strip()
        if not text:
            self.show_warning("No text", "Transcript area is empty.")
            return
        dialog = Gtk.FileChooserDialog(
            title="Save Transcript",
            transient_for=self,
            action=Gtk.FileChooserAction.SAVE,
        )
        dialog.add_buttons(
            "_Cancel", Gtk.ResponseType.CANCEL,
            "_Save", Gtk.ResponseType.ACCEPT
        )
        try:
            dialog.set_current_name("transcript.txt")
        except Exception:
            pass
        dialog.connect("response", self.on_save_response, text)
        dialog.show()

    def on_save_response(self, dialog: Gtk.FileChooserDialog, response: int, text: str) -> None:
        if response == Gtk.ResponseType.ACCEPT:
            file = dialog.get_file()
            if file:
                path = file.get_path()
                try:
                    with open(path, "w", encoding="utf-8") as f:
                        f.write(text)
                    self.show_info("Saved", f"Transcript saved to:\n{path}")
                except OSError as exc:
                    self.show_error("Save failed", str(exc))
        dialog.close()

    # ------------------------------------------------------------------
    # Dialog helpers
    # ------------------------------------------------------------------

    def show_error(self, title: str, message: str) -> None:
        self._show_message(title, message, Gtk.MessageType.ERROR)

    def show_warning(self, title: str, message: str) -> None:
        self._show_message(title, message, Gtk.MessageType.WARNING)

    def show_info(self, title: str, message: str) -> None:
        self._show_message(title, message, Gtk.MessageType.INFO)

    def _show_message(self, title: str, message: str, mtype: Gtk.MessageType) -> None:
        dialog = Gtk.MessageDialog(
            transient_for=self,
            modal=True,
            message_type=mtype,
            buttons=Gtk.ButtonsType.OK,
            text=title,
            secondary_text=message
        )
        dialog.connect("response", lambda d, r: d.close())
        dialog.show()


class DictAiTeApp(Gtk.Application):
    def __init__(self) -> None:
        super().__init__(application_id="com.example.dictaite")

    def do_activate(self) -> None:  # pragma: no cover - UI startup
        win = DictAiTeWindow(self)
        win.present()


def main(argv: list[str] | None = None) -> int:
    app = DictAiTeApp()
    return app.run(argv or None)


if __name__ == "__main__":  # pragma: no cover - CLI entry
    import sys

    raise SystemExit(main(sys.argv))
