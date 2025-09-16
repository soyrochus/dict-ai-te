# Part of dictaite: Recording, transcribing, and translating voice notes | Copyright (c) 2025 | License: MIT
"""dict-ai-te GTK recorder/transcriber."""

from __future__ import annotations

import os
import tempfile
import threading
import time
from typing import Callable

import numpy as np
import sounddevice as sd
import soundfile as sf
import io
from .api import get_openai_client, transcribe_file, translate_text
from .config import AppConfig

import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gtk, GLib, Gdk

LANGUAGES = [
    {"code": "default", "name": "Default (Auto-detect)"},
    {"code": "en", "name": "English"},
    {"code": "zh", "name": "中文 (Chinese, Mandarin)"},
    {"code": "es", "name": "Español (Spanish)"},
    {"code": "de", "name": "Deutsch (German)"},
    {"code": "fr", "name": "Français (French)"},
    {"code": "ja", "name": "日本語 (Japanese)"},
    {"code": "pt", "name": "Português (Portuguese)"},
    {"code": "ru", "name": "Русский (Russian)"},
    {"code": "ar", "name": "العربية (Arabic)"},
    {"code": "it", "name": "Italiano (Italian)"},
    {"code": "ko", "name": "한국어 (Korean)"},
    {"code": "hi", "name": "हिन्दी (Hindi)"},
    {"code": "nl", "name": "Nederlands (Dutch)"},
    {"code": "tr", "name": "Türkçe (Turkish)"},
    {"code": "pl", "name": "Polski (Polish)"},
    {"code": "id", "name": "Bahasa Indonesia (Indonesian)"},
    {"code": "th", "name": "ภาษาไทย (Thai)"},
    {"code": "sv", "name": "Svenska (Swedish)"},
    {"code": "he", "name": "עברית (Hebrew)"},
    {"code": "cs", "name": "Čeština (Czech)"},
]

LANGUAGE_NAME = {item["code"]: item["name"] for item in LANGUAGES}

# Get absolute path to the img directory
BASEDIR = os.path.dirname(os.path.abspath(__file__))
IMGDIR = os.path.abspath(os.path.join(BASEDIR, '..', 'img'))

FEMALE_VOICES = [
    ("nova", "Nova"),
    ("alloy", "Alloy"),
    ("verse", "Verse"),
    ("sol", "Sol"),
]

MALE_VOICES = [
    ("onyx", "Onyx"),
    ("sage", "Sage"),
    ("echo", "Echo"),
    ("ember", "Ember"),
]

VOICE_SAMPLE_TEXT = "This is a short sample to preview the selected voice."


class DictAiTeWindow(Gtk.ApplicationWindow):
    """Main application window."""

    def __init__(self, app: Gtk.Application) -> None:
        super().__init__(application=app, title="dict-ai-te")
        self.set_default_size(400, 600)
        self.client = get_openai_client()
        self.config = AppConfig.load()

        self.is_recording = False
        self.is_playing = False
        self.start_time: float | None = None
        self.elapsed_seconds = 0
        self.timer_thread: threading.Thread | None = None
        self.stream: sd.InputStream | None = None
        self.audio_frames: list[np.ndarray] = []
        
        # Icon paths (use absolute paths for reliability)
        self.mic_icon_path = os.path.join(IMGDIR, "microphone.png")
        self.stop_icon_path = os.path.join(IMGDIR, "stop.png")

        # Optionally, print resolved paths for debugging
        print(f"Mic icon path: {self.mic_icon_path}")
        print(f"Stop icon path: {self.stop_icon_path}")

        self.build_ui()
        self.apply_config_defaults()

    # ------------------------------------------------------------------
    # UI
    # ------------------------------------------------------------------

    def build_ui(self) -> None:
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6, margin_top=10,
                      margin_bottom=10, margin_start=10, margin_end=10)
        self.set_child(box)

        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        header.set_halign(Gtk.Align.END)
        self.settings_button = Gtk.Button.new_with_label("Settings")
        self.settings_button.set_halign(Gtk.Align.END)
        self.settings_button.connect("clicked", self.open_settings)
        header.append(self.settings_button)
        box.append(header)

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

        # --- New controls layout with labels ---
        controls_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=3)

        # Create controls before using them
        self.language_combo = Gtk.ComboBoxText()
        for item in LANGUAGES:
            self.language_combo.append(item["code"], item["name"])
        self.language_combo.set_active(0)

        self.translate_switch = Gtk.Switch()
        self.translate_switch.set_halign(Gtk.Align.START)
        self.translate_switch.set_hexpand(False)
        self.translate_switch.connect("notify::active", self.on_translate_switch)

        self.target_combo = Gtk.ComboBoxText()
        for item in LANGUAGES[1:]:
            self.target_combo.append(item["code"], item["name"])
        self.target_combo.set_active(0)
        self.target_combo.set_sensitive(False)

        lang_label = Gtk.Label(label="Origin language")
        lang_label.set_halign(Gtk.Align.START)
        controls_box.append(lang_label)
        controls_box.append(self.language_combo)

        translate_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        translate_label = Gtk.Label(label="Translate to")
        translate_label.set_halign(Gtk.Align.START)
        translate_box.append(translate_label)
        translate_box.append(self.translate_switch)
        controls_box.append(translate_box)

        dest_label = Gtk.Label(label="Destination language")
        dest_label.set_halign(Gtk.Align.START)
        controls_box.append(dest_label)
        controls_box.append(self.target_combo)

        box.append(controls_box)

        self.text_view = Gtk.TextView(wrap_mode=Gtk.WrapMode.WORD)
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_child(self.text_view)
        scrolled.set_vexpand(True)
        box.append(scrolled)

        actions = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)

        save_img = Gtk.Image.new_from_icon_name("document-save-symbolic")
        self.save_btn = Gtk.Button()
        self.save_btn.set_child(save_img)
        self.save_btn.set_tooltip_text("Save Transcript")
        self.save_btn.connect("clicked", self.save_transcript)
        actions.append(self.save_btn)

        copy_img = Gtk.Image.new_from_icon_name("edit-copy-symbolic")
        self.copy_btn = Gtk.Button()
        self.copy_btn.set_child(copy_img)
        self.copy_btn.set_tooltip_text("Copy Transcript")
        self.copy_btn.connect("clicked", self.copy_transcript)
        actions.append(self.copy_btn)

        play_img = Gtk.Image.new_from_icon_name("media-playback-start-symbolic")
        self.play_btn = Gtk.Button()
        self.play_btn.set_child(play_img)
        self.play_btn.set_tooltip_text("Play Transcript")
        self.play_btn.connect("clicked", self.play_transcript)
        actions.append(self.play_btn)

        voice_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=3)
        self.female_btn = Gtk.CheckButton.new_with_label("Female")
        self.male_btn = Gtk.CheckButton.new_with_label("Male")
        self.male_btn.set_group(self.female_btn)
        self.female_btn.set_active(True)
        voice_box.append(self.female_btn)
        voice_box.append(self.male_btn)
        actions.append(voice_box)


        box.append(actions)

    def apply_config_defaults(self) -> None:
        self._set_combo_active_id(self.language_combo, self.config.default_language)
        self._set_combo_active_id(self.target_combo, self.config.default_target_language)
        self.translate_switch.set_active(self.config.translation_enabled)

    def _set_combo_active_id(self, combo: Gtk.ComboBoxText, value: str | None, fallback: int = 0) -> None:
        if value:
            try:
                if combo.set_active_id(value):
                    return
            except TypeError:
                pass
            if combo.get_active_id() == value:
                return
        combo.set_active(fallback)

    def open_settings(self, _button: Gtk.Button) -> None:
        dialog = SettingsDialog(
            self,
            self.config,
            LANGUAGES,
            LANGUAGES[1:],
            FEMALE_VOICES,
            MALE_VOICES,
            self.preview_voice,
        )
        dialog.connect("response", self.on_settings_response)
        dialog.show()

    def on_settings_response(self, dialog: "SettingsDialog", response: int) -> None:
        if response == Gtk.ResponseType.OK:
            new_config = dialog.build_config(self.config)
            self.config = new_config
            self.apply_config_defaults()
            try:
                self.config.save()
            except OSError as exc:
                self.show_error("Settings", f"Failed to save configuration: {exc}")
        dialog.close()

    def preview_voice(self, voice_id: str | None) -> None:
        if not voice_id:
            return
        self.status_label.set_text("Previewing voice...")
        threading.Thread(
            target=self._generate_and_play,
            args=(VOICE_SAMPLE_TEXT, voice_id),
            daemon=True,
        ).start()

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
            if self.start_time is not None:
                self.elapsed_seconds = int(time.time() - self.start_time)
            else:
                self.elapsed_seconds = 0
            h = self.elapsed_seconds // 3600
            m = (self.elapsed_seconds % 3600) // 60
            s = self.elapsed_seconds % 60
            timer_str = f"{h:02}:{m:02}:{s:02}"
            GLib.idle_add(self.timer_label.set_text, timer_str)
            time.sleep(1)

    def on_translate_switch(self, switch: Gtk.Switch, _param: object) -> None:
        self.target_combo.set_sensitive(switch.get_active())

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
            lang_code = self.language_combo.get_active_id() or self.config.default_language
            with open(path, "rb") as f:
                transcript = transcribe_file(f, lang_code)
            os.remove(path)

            if self.translate_switch.get_active():
                target_code = self.target_combo.get_active_id() or self.config.default_target_language
                if target_code:
                    src_name = LANGUAGE_NAME.get(lang_code, "the source language")
                    tgt_name = LANGUAGE_NAME.get(target_code, target_code)
                    try:
                        transcript = translate_text(transcript, src_name, tgt_name)
                    except Exception as exc:  # pragma: no cover - network path
                        GLib.idle_add(self.show_error, "Translation error", str(exc))

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

    def play_transcript(self, button: Gtk.Button) -> None:
        buffer = self.text_view.get_buffer()
        start, end = buffer.get_bounds()
        text = buffer.get_text(start, end, True).strip()
        if not text:
            self.show_warning("No text", "Transcript area is empty.")
            return
        self.status_label.set_text("Generating audio...")
        if self.female_btn.get_active():
            voice = self.config.female_voice or "nova"
        else:
            voice = self.config.male_voice or "onyx"
        threading.Thread(target=self._generate_and_play, args=(text, voice), daemon=True).start()

    def _generate_and_play(self, text: str, voice: str) -> None:
        if not self.client:
            GLib.idle_add(self.show_error, "Configuration", "OpenAI API key not configured")
            GLib.idle_add(self.status_label.set_text, "Press to start recording")
            return
        try:
            resp = self.client.audio.speech.create(
                input=text,
                model="tts-1",
                voice=voice,
                response_format="wav"
            )
            audio_data = None
            if isinstance(resp, (bytes, bytearray)):
                audio_data = resp
            elif hasattr(resp, "read"):
                audio_data = resp.read()
            elif hasattr(resp, "content"):
                audio_data = resp.content
            elif hasattr(resp, "iter_bytes"):
                # For httpx streaming responses
                audio_data = b"".join(resp.iter_bytes())
            else:
                raise TypeError(f"Unknown response type: {type(resp)}")
            bio = io.BytesIO(audio_data)
            data, sr = sf.read(bio, dtype="float32")
            print(f"Playing audio of length {len(data)} at {sr} Hz")
            self.play_audio_with_feedback(data, sr)
        except Exception as exc:
            print(f"Audio error: {exc}")  # Print error for debugging
            GLib.idle_add(self.show_error, "Audio error", str(exc))
        finally:
            GLib.idle_add(self.status_label.set_text, "Press to start recording")

    def play_audio_with_feedback(self, data: np.ndarray, sr: int) -> None:
        """Play audio while updating level and timer."""
        # Ensure data has a channel dimension so the callback always receives
        # 2-D chunks matching the output stream's expectations.
        if data.ndim == 1:
            data = data.reshape(-1, 1)

        self.is_playing = True
        start = time.time()
        idx = 0

        def callback(outdata, frames, time_info, status) -> None:
            nonlocal idx
            if status:
                print(status)
            end = idx + frames
            chunk = data[idx:end]
            if len(chunk) < frames:
                outdata[:len(chunk)] = chunk
                outdata[len(chunk):] = 0
                amplitude = float(np.max(np.abs(chunk))) if len(chunk) else 0.0
                GLib.idle_add(self.level.set_value, amplitude)
                raise sd.CallbackStop()
            outdata[:] = chunk
            amplitude = float(np.max(np.abs(chunk)))
            GLib.idle_add(self.level.set_value, amplitude)
            idx = end

        stream = sd.OutputStream(samplerate=sr, channels=data.shape[1] if data.ndim > 1 else 1, callback=callback)
        stream.start()
        while stream.active:
            elapsed = int(time.time() - start)
            h, m, s = elapsed // 3600, (elapsed % 3600) // 60, elapsed % 60
            timer_str = f"{h:02}:{m:02}:{s:02}"
            GLib.idle_add(self.timer_label.set_text, timer_str)
            time.sleep(0.1)
        stream.stop()
        stream.close()
        self.is_playing = False
        GLib.idle_add(self.level.set_value, 0.0)
        GLib.idle_add(self.timer_label.set_text, "00:00:00")

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

    def copy_transcript(self, button: Gtk.Button) -> None:
        buffer = self.text_view.get_buffer()
        start, end = buffer.get_bounds()
        text = buffer.get_text(start, end, True)
        display = self.get_display()
        if not display:
            return
        clipboard = display.get_clipboard()
        provider = Gdk.ContentProvider.new_for_bytes(
            "text/plain;charset=utf-8", GLib.Bytes.new(text.encode("utf-8"))
        )
        clipboard.set_content(provider)

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


class SettingsDialog(Gtk.Dialog):
    def __init__(
        self,
        parent: Gtk.Window,
        config: AppConfig,
        languages: list[dict[str, str]],
        target_languages: list[dict[str, str]],
        female_voices: list[tuple[str, str]],
        male_voices: list[tuple[str, str]],
        preview_callback: Callable[[str], None],
    ) -> None:
        super().__init__(title="Settings", transient_for=parent, modal=True)
        self.preview_callback = preview_callback
        self.add_buttons("_Cancel", Gtk.ResponseType.CANCEL, "_Save", Gtk.ResponseType.OK)
        self.set_default_size(380, 300)

        content = self.get_content_area()
        content.set_spacing(12)
        content.set_margin_top(12)
        content.set_margin_bottom(12)
        content.set_margin_start(12)
        content.set_margin_end(12)

        grid = Gtk.Grid(column_spacing=12, row_spacing=12)
        content.append(grid)

        lang_label = Gtk.Label(label="Default language")
        lang_label.set_halign(Gtk.Align.START)
        grid.attach(lang_label, 0, 0, 1, 1)

        self.language_combo = Gtk.ComboBoxText()
        for item in languages:
            self.language_combo.append(item["code"], item["name"])
        self._set_combo_default(self.language_combo, config.default_language)
        grid.attach(self.language_combo, 1, 0, 1, 1)

        translate_label = Gtk.Label(label="Translate by default")
        translate_label.set_halign(Gtk.Align.START)
        grid.attach(translate_label, 0, 1, 1, 1)

        self.translate_switch = Gtk.Switch()
        self.translate_switch.set_active(config.translation_enabled)
        self.translate_switch.set_halign(Gtk.Align.START)
        self.translate_switch.set_hexpand(False)
        self.translate_switch.connect("notify::active", self._on_translate_toggle)
        grid.attach(self.translate_switch, 1, 1, 1, 1)

        target_label = Gtk.Label(label="Default target language")
        target_label.set_halign(Gtk.Align.START)
        grid.attach(target_label, 0, 2, 1, 1)

        self.target_combo = Gtk.ComboBoxText()
        for item in target_languages:
            self.target_combo.append(item["code"], item["name"])
        self._set_combo_default(self.target_combo, config.default_target_language)
        self.target_combo.set_sensitive(config.translation_enabled)
        grid.attach(self.target_combo, 1, 2, 1, 1)

        female_label = Gtk.Label(label="Female voice")
        female_label.set_halign(Gtk.Align.START)
        grid.attach(female_label, 0, 3, 1, 1)

        female_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.female_voice_combo = Gtk.ComboBoxText()
        for value, caption in female_voices:
            self.female_voice_combo.append(value, caption)
        self._set_combo_default(self.female_voice_combo, config.female_voice)
        female_box.append(self.female_voice_combo)
        female_button = Gtk.Button.new_with_label("Play")
        female_button.connect("clicked", self._on_preview_clicked, self.female_voice_combo)
        female_box.append(female_button)
        grid.attach(female_box, 1, 3, 1, 1)

        male_label = Gtk.Label(label="Male voice")
        male_label.set_halign(Gtk.Align.START)
        grid.attach(male_label, 0, 4, 1, 1)

        male_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.male_voice_combo = Gtk.ComboBoxText()
        for value, caption in male_voices:
            self.male_voice_combo.append(value, caption)
        self._set_combo_default(self.male_voice_combo, config.male_voice)
        male_box.append(self.male_voice_combo)
        male_button = Gtk.Button.new_with_label("Play")
        male_button.connect("clicked", self._on_preview_clicked, self.male_voice_combo)
        male_box.append(male_button)
        grid.attach(male_box, 1, 4, 1, 1)

    def _set_combo_default(self, combo: Gtk.ComboBoxText, value: str) -> None:
        try:
            if combo.set_active_id(value):
                return
        except TypeError:
            pass
        if combo.get_active_id() == value:
            return
        combo.set_active(0)

    def _on_preview_clicked(self, _button: Gtk.Button, combo: Gtk.ComboBoxText) -> None:
        voice_id = combo.get_active_id()
        if voice_id and self.preview_callback:
            self.preview_callback(voice_id)

    def _on_translate_toggle(self, switch: Gtk.Switch, _param: object) -> None:
        self.target_combo.set_sensitive(switch.get_active())

    def build_config(self, base: AppConfig) -> AppConfig:
        language = self.language_combo.get_active_id() or base.default_language
        target = self.target_combo.get_active_id() or base.default_target_language
        female_voice = self.female_voice_combo.get_active_id() or base.female_voice
        male_voice = self.male_voice_combo.get_active_id() or base.male_voice
        return AppConfig(
            default_language=language,
            default_target_language=target,
            translation_enabled=self.translate_switch.get_active(),
            female_voice=female_voice,
            male_voice=male_voice,
        )


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
