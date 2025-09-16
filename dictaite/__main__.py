# Part of dictaite: Recording, transcribing, and translating voice notes | Copyright (c) 2025 | License: MIT
"""dict-ai-te GTK recorder/transcriber."""

from __future__ import annotations

import os
import tempfile
import threading
import time

import numpy as np
import sounddevice as sd
import soundfile as sf
import io
from .api import get_openai_client, transcribe_file, translate_text
from .config import load_config, save_config, AVAILABLE_VOICES

import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gtk, GLib, Gdk, Gio

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


class DictAiTeWindow(Gtk.ApplicationWindow):
    """Main application window."""

    def __init__(self, app: Gtk.Application) -> None:
        super().__init__(application=app, title="dict-ai-te")
        self.set_default_size(400, 600)
        self.client = get_openai_client()
        
        # Load configuration
        self.config = load_config()

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

    # ------------------------------------------------------------------
    # UI
    # ------------------------------------------------------------------

    def build_ui(self) -> None:
        # Create a header bar with menu button
        header_bar = Gtk.HeaderBar()
        self.set_titlebar(header_bar)
        
        # Add settings menu button
        menu_button = Gtk.MenuButton()
        menu_button.set_icon_name("open-menu-symbolic")
        header_bar.pack_end(menu_button)
        
        # Create menu model
        menu_model = Gio.Menu()
        menu_model.append("Settings", "app.settings")
        menu_button.set_menu_model(menu_model)

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

        # --- New controls layout with labels ---
        controls_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=3)

        # Create controls before using them
        self.language_combo = Gtk.ComboBoxText()
        for item in LANGUAGES:
            self.language_combo.append(item["code"], item["name"])
        
        # Set default to English (index 1, since index 0 is "Default (Auto-detect)")
        default_source = self.config.get("languages", {}).get("default_source", "en")
        default_index = 1  # Default to English
        for i, item in enumerate(LANGUAGES):
            if item["code"] == default_source:
                default_index = i
                break
        self.language_combo.set_active(default_index)

        self.translate_switch = Gtk.Switch()
        self.translate_switch.connect("notify::active", self.on_translate_switch)

        self.target_combo = Gtk.ComboBoxText()
        for item in LANGUAGES[1:]:
            self.target_combo.append(item["code"], item["name"])
        
        # Set default target language
        default_target = self.config.get("languages", {}).get("default_target", "es")
        target_index = 0  # Default to first option
        for i, item in enumerate(LANGUAGES[1:]):
            if item["code"] == default_target:
                target_index = i
                break
        self.target_combo.set_active(target_index)
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

        voice_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        voice_label = Gtk.Label(label="Voice:")
        voice_label.set_halign(Gtk.Align.START)
        voice_box.append(voice_label)
        
        # Create horizontal radio buttons for Female/Male
        voice_radio_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=3)
        self.female_btn = Gtk.CheckButton.new_with_label("Female")
        self.male_btn = Gtk.CheckButton.new_with_label("Male")
        self.male_btn.set_group(self.female_btn)
        
        # Set default based on config or default to Female
        default_voice_type = "female"  # Default fallback
        female_voice = self.config.get("voices", {}).get("female", "nova")
        male_voice = self.config.get("voices", {}).get("male", "onyx")
        
        # Default to female voice
        self.female_btn.set_active(True)
        
        voice_radio_box.append(self.female_btn)
        voice_radio_box.append(self.male_btn)
        voice_box.append(voice_radio_box)
        actions.append(voice_box)


        box.append(actions)

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
            lang_code = self.language_combo.get_active_id()
            with open(path, "rb") as f:
                transcript = transcribe_file(f, lang_code)
            os.remove(path)

            if self.translate_switch.get_active():
                target_code = self.target_combo.get_active_id()
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
        threading.Thread(target=self._generate_and_play, args=(text,), daemon=True).start()

    def _generate_and_play(self, text: str) -> None:
        if not self.client:
            GLib.idle_add(self.show_error, "Configuration", "OpenAI API key not configured")
            GLib.idle_add(self.status_label.set_text, "Press to start recording")
            return
        try:
            # Use configured voice based on selected gender
            if self.female_btn.get_active():
                voice = self.config.get("voices", {}).get("female", "nova")
            else:
                voice = self.config.get("voices", {}).get("male", "onyx")
            
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
    # Settings
    # ------------------------------------------------------------------

    def show_settings_dialog(self, action, param):
        """Show the settings dialog."""
        dialog = Gtk.Dialog(
            title="Settings",
            transient_for=self,
            modal=True,
            use_header_bar=True
        )
        dialog.add_buttons(
            "_Cancel", Gtk.ResponseType.CANCEL,
            "_Save", Gtk.ResponseType.ACCEPT
        )
        dialog.set_default_size(400, 300)

        content_area = dialog.get_content_area()
        content_area.set_spacing(12)
        content_area.set_margin_top(12)
        content_area.set_margin_bottom(12)
        content_area.set_margin_start(12)
        content_area.set_margin_end(12)

        # Voice settings section
        voice_frame = Gtk.Frame(label="Voice Settings")
        voice_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        voice_box.set_margin_top(6)
        voice_box.set_margin_bottom(6)
        voice_box.set_margin_start(6)
        voice_box.set_margin_end(6)
        voice_frame.set_child(voice_box)

        # Female voice selection
        female_label = Gtk.Label(label="Female Voice:")
        female_label.set_halign(Gtk.Align.START)
        voice_box.append(female_label)
        
        female_voice_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.female_voice_combo = Gtk.ComboBoxText()
        for voice in AVAILABLE_VOICES["female"]:
            self.female_voice_combo.append(voice, voice.title())
        
        # Set current value
        current_female = self.config.get("voices", {}).get("female", "nova")
        for i, voice in enumerate(AVAILABLE_VOICES["female"]):
            if voice == current_female:
                self.female_voice_combo.set_active(i)
                break
        
        female_play_btn = Gtk.Button(label="▶ Test")
        female_play_btn.connect("clicked", self.test_female_voice)
        
        female_voice_box.append(self.female_voice_combo)
        female_voice_box.append(female_play_btn)
        voice_box.append(female_voice_box)

        # Male voice selection
        male_label = Gtk.Label(label="Male Voice:")
        male_label.set_halign(Gtk.Align.START)
        voice_box.append(male_label)
        
        male_voice_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.male_voice_combo = Gtk.ComboBoxText()
        for voice in AVAILABLE_VOICES["male"]:
            self.male_voice_combo.append(voice, voice.title())
        
        # Set current value
        current_male = self.config.get("voices", {}).get("male", "onyx")
        for i, voice in enumerate(AVAILABLE_VOICES["male"]):
            if voice == current_male:
                self.male_voice_combo.set_active(i)
                break
        
        male_play_btn = Gtk.Button(label="▶ Test")
        male_play_btn.connect("clicked", self.test_male_voice)
        
        male_voice_box.append(self.male_voice_combo)
        male_voice_box.append(male_play_btn)
        voice_box.append(male_voice_box)

        content_area.append(voice_frame)

        # Language settings section
        lang_frame = Gtk.Frame(label="Default Languages")
        lang_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        lang_box.set_margin_top(6)
        lang_box.set_margin_bottom(6)
        lang_box.set_margin_start(6)
        lang_box.set_margin_end(6)
        lang_frame.set_child(lang_box)

        # Default source language
        source_label = Gtk.Label(label="Default Source Language:")
        source_label.set_halign(Gtk.Align.START)
        lang_box.append(source_label)
        
        self.source_lang_combo = Gtk.ComboBoxText()
        for item in LANGUAGES:
            self.source_lang_combo.append(item["code"], item["name"])
        
        # Set current value
        current_source = self.config.get("languages", {}).get("default_source", "en")
        for i, item in enumerate(LANGUAGES):
            if item["code"] == current_source:
                self.source_lang_combo.set_active(i)
                break
        lang_box.append(self.source_lang_combo)

        # Default target language
        target_label = Gtk.Label(label="Default Target Language:")
        target_label.set_halign(Gtk.Align.START)
        lang_box.append(target_label)
        
        self.target_lang_combo = Gtk.ComboBoxText()
        for item in LANGUAGES[1:]:  # Exclude "Default (Auto-detect)"
            self.target_lang_combo.append(item["code"], item["name"])
        
        # Set current value
        current_target = self.config.get("languages", {}).get("default_target", "es")
        for i, item in enumerate(LANGUAGES[1:]):
            if item["code"] == current_target:
                self.target_lang_combo.set_active(i)
                break
        lang_box.append(self.target_lang_combo)

        content_area.append(lang_frame)

        dialog.connect("response", self.on_settings_response)
        dialog.show()

    def test_female_voice(self, button):
        """Test the selected female voice."""
        voice = self.female_voice_combo.get_active_id()
        if voice:
            self._test_voice(voice, "This is a test of the female voice.")

    def test_male_voice(self, button):
        """Test the selected male voice."""
        voice = self.male_voice_combo.get_active_id()
        if voice:
            self._test_voice(voice, "This is a test of the male voice.")

    def _test_voice(self, voice: str, text: str):
        """Play a test audio with the given voice."""
        if not self.client:
            self.show_error("Configuration", "OpenAI API key not configured")
            return
        
        def generate_and_play():
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
                    audio_data = b"".join(resp.iter_bytes())
                else:
                    raise TypeError(f"Unknown response type: {type(resp)}")
                
                bio = io.BytesIO(audio_data)
                data, sr = sf.read(bio, dtype="float32")
                
                # Simple playback without level/timer updates for settings
                if data.ndim == 1:
                    data = data.reshape(-1, 1)
                
                def callback(outdata, frames, time_info, status):
                    nonlocal idx
                    if status:
                        print(status)
                    end = idx + frames
                    chunk = data[idx:end]
                    if len(chunk) < frames:
                        outdata[:len(chunk)] = chunk
                        outdata[len(chunk):] = 0
                        raise sd.CallbackStop()
                    outdata[:] = chunk
                    idx = end
                
                idx = 0
                stream = sd.OutputStream(samplerate=sr, channels=data.shape[1], callback=callback)
                stream.start()
                while stream.active:
                    time.sleep(0.1)
                stream.stop()
                stream.close()
                
            except Exception as exc:
                GLib.idle_add(self.show_error, "Voice test error", str(exc))
        
        threading.Thread(target=generate_and_play, daemon=True).start()

    def on_settings_response(self, dialog, response):
        """Handle settings dialog response."""
        if response == Gtk.ResponseType.ACCEPT:
            # Update configuration
            self.config["voices"]["female"] = self.female_voice_combo.get_active_id() or "nova"
            self.config["voices"]["male"] = self.male_voice_combo.get_active_id() or "onyx"
            self.config["languages"]["default_source"] = self.source_lang_combo.get_active_id() or "en"
            self.config["languages"]["default_target"] = self.target_lang_combo.get_active_id() or "es"
            
            # Save configuration
            if save_config(self.config):
                self.show_info("Settings", "Configuration saved successfully.")
            else:
                self.show_error("Settings", "Failed to save configuration.")
        
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
        
        # Add settings action
        settings_action = Gio.SimpleAction.new("settings", None)
        settings_action.connect("activate", self.on_settings_action)
        self.add_action(settings_action)

    def on_settings_action(self, action, param):
        """Handle settings action."""
        if hasattr(self, '_window'):
            self._window.show_settings_dialog(action, param)

    def do_activate(self) -> None:  # pragma: no cover - UI startup
        win = DictAiTeWindow(self)
        self._window = win  # Store reference for settings action
        win.present()


def main(argv: list[str] | None = None) -> int:
    app = DictAiTeApp()
    return app.run(argv or None)


if __name__ == "__main__":  # pragma: no cover - CLI entry
    import sys

    raise SystemExit(main(sys.argv))
