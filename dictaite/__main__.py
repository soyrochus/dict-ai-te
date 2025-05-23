"""GTK4 based recorder and transcriber."""

from __future__ import annotations

import os
import tempfile
import threading
import time

import gi

# Ensure GTK4 is available
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib

import numpy as np
import sounddevice as sd
import soundfile as sf
from dotenv import load_dotenv
from openai import OpenAI


class DictAiTe(Gtk.Application):
    """Main application class."""

    def __init__(self) -> None:
        super().__init__(application_id="com.soyrochus.dictaite")
        load_dotenv()
        self.client: OpenAI | None = None
        api_key = os.getenv("OPENAI_API_KEY")
        if api_key:
            self.client = OpenAI(api_key=api_key)

        self.window: Gtk.ApplicationWindow | None = None
        self.record_button: Gtk.Button | None = None
        self.status_label: Gtk.Label | None = None
        self.timer_label: Gtk.Label | None = None
        self.text_buffer: Gtk.TextBuffer | None = None
        self.level: Gtk.LevelBar | None = None

        self.stream: sd.InputStream | None = None
        self.audio_frames: list[np.ndarray] = []
        self.is_recording = False
        self.start_time: float | None = None
        self.timer_id: int | None = None

    # ------------------------------------------------------------------
    # GTK lifecycle
    # ------------------------------------------------------------------

    def do_activate(self) -> None:  # pragma: no cover - UI
        self.window = Gtk.ApplicationWindow(application=self)
        self.window.set_title("dict-ai-te")
        self.window.set_default_size(450, 600)
        self.build_ui()
        self.window.present()

    def build_ui(self) -> None:
        assert self.window
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6,
                      margin_top=10, margin_bottom=10, margin_start=10, margin_end=10)
        self.window.set_child(box)

        # waveform / level display
        self.level = Gtk.LevelBar()
        self.level.set_min_value(0.0)
        self.level.set_max_value(1.0)
        box.append(self.level)

        # record button
        self.record_button = Gtk.Button(label="🎙")
        self.record_button.set_size_request(100, 100)
        self.record_button.connect("clicked", self.on_toggle_recording)
        box.append(self.record_button)

        self.status_label = Gtk.Label(label="Press to start recording")
        box.append(self.status_label)

        self.timer_label = Gtk.Label(label="00:00:00")
        box.append(self.timer_label)

        scrolled = Gtk.ScrolledWindow(vexpand=True, hexpand=True)
        text_view = Gtk.TextView(wrap_mode=Gtk.WrapMode.WORD)
        self.text_buffer = text_view.get_buffer()
        scrolled.set_child(text_view)
        box.append(scrolled)

        save_button = Gtk.Button(label="Save Transcript")
        save_button.connect("clicked", self.on_save_transcript)
        box.append(save_button)

    # ------------------------------------------------------------------
    # Recording
    # ------------------------------------------------------------------

    def on_toggle_recording(self, button: Gtk.Button) -> None:  # pragma: no cover - UI
        if not self.is_recording:
            self.start_recording()
        else:
            self.stop_recording()

    def start_recording(self) -> None:
        self.is_recording = True
        self.record_button.set_label("⏸")  # pause symbol
        if self.status_label:
            self.status_label.set_text("Recording... Press to stop.")
        self.start_time = time.time()
        self.audio_frames.clear()
        try:
            self.stream = sd.InputStream(
                samplerate=16000,
                channels=1,
                callback=self.audio_callback,
            )
            self.stream.start()
        except Exception as exc:  # pragma: no cover - runtime error path
            self.show_error("Recording error", str(exc))
            self.is_recording = False
            return
        self.timer_id = GLib.timeout_add_seconds(1, self.update_timer)

    def stop_recording(self) -> None:
        self.is_recording = False
        if self.record_button:
            self.record_button.set_label("🎙")
        if self.timer_id:
            GLib.source_remove(self.timer_id)
            self.timer_id = None
        if self.stream:
            try:
                self.stream.stop()
                self.stream.close()
            except Exception as exc:  # pragma: no cover - runtime error path
                self.show_error("Recording error", str(exc))
        self.stream = None
        if self.status_label:
            self.status_label.set_text("Transcribing...")
        threading.Thread(target=self.transcribe_audio, daemon=True).start()

    def audio_callback(self, indata, frames, time_info, status) -> None:
        if status:
            print(status)
        self.audio_frames.append(indata.copy())
        level = float(np.abs(indata).mean())
        if self.level:
            GLib.idle_add(self.level.set_value, level)

    def update_timer(self) -> bool:
        if not self.is_recording or not self.timer_label:
            return False
        elapsed = int(time.time() - (self.start_time or time.time()))
        h = elapsed // 3600
        m = (elapsed % 3600) // 60
        s = elapsed % 60
        self.timer_label.set_text(f"{h:02}:{m:02}:{s:02}")
        return True

    # ------------------------------------------------------------------
    # Transcription and file saving
    # ------------------------------------------------------------------

    def transcribe_audio(self) -> None:
        if not self.client:
            self.show_error("Configuration", "OpenAI API key not configured")
            GLib.idle_add(self.reset_status)
            return
        try:
            audio = np.concatenate(self.audio_frames, axis=0)
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
                sf.write(tmp.name, audio, 16000)
                path = tmp.name
            with open(path, "rb") as f:
                response = self.client.audio.transcriptions.create(
                    model="whisper-1", file=f
                )
            os.remove(path)
            transcript = response.text
            if self.text_buffer:
                GLib.idle_add(self.text_buffer.set_text, transcript)
        except Exception as exc:  # pragma: no cover - network path
            self.show_error("Transcription error", str(exc))
        finally:
            GLib.idle_add(self.reset_status)

    def reset_status(self) -> None:
        if self.status_label:
            self.status_label.set_text("Press to start recording")

    def on_save_transcript(self, button: Gtk.Button) -> None:  # pragma: no cover - UI
        if not self.text_buffer:
            return
        start_iter = self.text_buffer.get_start_iter()
        end_iter = self.text_buffer.get_end_iter()
        text = self.text_buffer.get_text(start_iter, end_iter, True).strip()
        if not text:
            self.show_message("No text", "Transcript area is empty.")
            return
        assert self.window
        dialog = Gtk.FileChooserDialog(
            title="Save Transcript",
            transient_for=self.window,
            action=Gtk.FileChooserAction.SAVE,
        )
        dialog.add_buttons(
            "_Cancel",
            Gtk.ResponseType.CANCEL,
            "_Save",
            Gtk.ResponseType.ACCEPT,
        )
        dialog.set_current_name("transcript.txt")
        response = dialog.run()
        if response == Gtk.ResponseType.ACCEPT:
            file = dialog.get_file()
            filename = file.get_path() if file else None
            if filename:
                try:
                    with open(filename, "w", encoding="utf-8") as f:
                        f.write(text)
                except OSError as exc:  # pragma: no cover - file system
                    self.show_error("Save failed", str(exc))
        dialog.destroy()

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    def show_error(self, title: str, message: str) -> None:
        if not self.window:
            return
        dialog = Gtk.MessageDialog(
            transient_for=self.window,
            modal=True,
            message_type=Gtk.MessageType.ERROR,
            buttons=Gtk.ButtonsType.CLOSE,
            text=title,
        )
        dialog.format_secondary_text(message)
        dialog.connect("response", lambda d, r: d.destroy())
        dialog.show()

    def show_message(self, title: str, message: str) -> None:
        if not self.window:
            return
        dialog = Gtk.MessageDialog(
            transient_for=self.window,
            modal=True,
            message_type=Gtk.MessageType.INFO,
            buttons=Gtk.ButtonsType.OK,
            text=title,
        )
        dialog.format_secondary_text(message)
        dialog.connect("response", lambda d, r: d.destroy())
        dialog.show()


def main() -> None:
    app = DictAiTe()
    app.run()


if __name__ == "__main__":
    main()
