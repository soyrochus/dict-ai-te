# Part of dictaite: Recording, transcribing, and translating voice notes | Copyright (c) 2025 | License: MIT
"""NiceGUI web interface for dict-ai-te."""

from __future__ import annotations

import io
import tempfile
import threading
from pathlib import Path
from typing import Callable

try:
    from nicegui import ui, app
    from starlette.requests import Request
    from starlette.responses import Response
except ImportError:
    raise ImportError("NiceGUI is required for web interface. Install with: pip install nicegui")

from .api import get_openai_client, transcribe_file, translate_text, format_structured_text, synthesize_speech_wav
from .config import AppConfig
from .constants import LANGUAGES, LANGUAGE_NAME, FEMALE_VOICES, MALE_VOICES, VOICE_SAMPLE_TEXT


# Global app instance
web_app = None


class DictAiTeWebApp:
    """Main web application class."""
    
    def __init__(self):
        self.config = AppConfig.load()
        self.client = get_openai_client()
        self.transcript_text = ""
        
        # UI elements - will be set during build_ui
        self.language_select = None
        self.translate_switch = None
        self.target_select = None
        self.female_voice_select = None
        self.male_voice_select = None
        self.transcript_area = None
        self.upload_area = None
        self.status_label = None
        
    def build_ui(self):
        """Build the web interface."""
        ui.page_title("dict-ai-te")
        
        with ui.column().classes('w-full max-w-4xl mx-auto p-4'):
            # Header
            with ui.row().classes('w-full justify-between items-center mb-4'):
                ui.label("dict-ai-te").classes('text-2xl font-bold')
                ui.button("Settings", on_click=self.open_settings).classes('ml-auto')
            
            # Controls
            with ui.card().classes('w-full p-4 mb-4'):
                ui.label("Language & Translation Settings").classes('text-lg font-semibold mb-2')
                
                with ui.row().classes('w-full gap-4'):
                    with ui.column().classes('flex-1'):
                        ui.label("Origin Language")
                        self.language_select = ui.select(
                            options={lang["code"]: lang["name"] for lang in LANGUAGES},
                            value=self.config.default_language,
                        ).classes('w-full')
                    
                    with ui.column().classes('flex-1'):
                        ui.label("Translate")
                        self.translate_switch = ui.switch(
                            value=self.config.translation_enabled,
                            on_change=self.on_translate_toggle
                        )
                    
                    with ui.column().classes('flex-1'):
                        ui.label("Target Language")
                        self.target_select = ui.select(
                            options={lang["code"]: lang["name"] for lang in LANGUAGES[1:]},
                            value=self.config.default_target_language,
                        ).classes('w-full')
                        self.target_select.enabled = self.config.translation_enabled
            
            # Audio Upload
            with ui.card().classes('w-full p-4 mb-4'):
                ui.label("Audio Upload").classes('text-lg font-semibold mb-2')
                self.upload_area = ui.upload(
                    label="Select or drop audio file",
                    auto_upload=True,
                    on_upload=self.handle_upload,
                    multiple=False
                ).classes('w-full').props('accept="audio/*"')
                
                self.status_label = ui.label("Ready to transcribe audio").classes('mt-2 text-sm text-gray-600')
            
            # Transcript area
            with ui.card().classes('w-full p-4 mb-4'):
                with ui.row().classes('w-full justify-between items-center mb-2'):
                    ui.label("Transcript").classes('text-lg font-semibold')
                    
                    with ui.row().classes('gap-2'):
                        ui.button("Copy", on_click=self.copy_transcript, icon="content_copy").props('flat')
                        ui.button("Download", on_click=self.download_transcript, icon="download").props('flat')
                        ui.button("Play", on_click=self.play_transcript, icon="play_arrow").props('flat')
                
                self.transcript_area = ui.textarea(
                    placeholder="Transcription will appear here...",
                    value=self.transcript_text
                ).classes('w-full min-h-40')
            
            # Voice settings
            with ui.card().classes('w-full p-4'):
                ui.label("Voice Settings").classes('text-lg font-semibold mb-2')
                
                with ui.row().classes('w-full gap-4'):
                    with ui.column().classes('flex-1'):
                        ui.label("Female Voice")
                        with ui.row().classes('w-full gap-2'):
                            self.female_voice_select = ui.select(
                                options={voice[0]: voice[1] for voice in FEMALE_VOICES},
                                value=self.config.female_voice,
                            ).classes('flex-1')
                            ui.button("Play", on_click=lambda: self.preview_voice(self.female_voice_select.value), icon="play_arrow").props('flat')
                    
                    with ui.column().classes('flex-1'):
                        ui.label("Male Voice")
                        with ui.row().classes('w-full gap-2'):
                            self.male_voice_select = ui.select(
                                options={voice[0]: voice[1] for voice in MALE_VOICES},
                                value=self.config.male_voice,
                            ).classes('flex-1')
                            ui.button("Play", on_click=lambda: self.preview_voice(self.male_voice_select.value), icon="play_arrow").props('flat')
    
    def on_translate_toggle(self):
        """Handle translate switch toggle."""
        self.target_select.enabled = self.translate_switch.value
    
    async def handle_upload(self, e):
        """Handle audio file upload and transcription."""
        if not e.content:
            ui.notify("No file content received", type="negative")
            return
            
        if not self.client:
            ui.notify("OpenAI API key not configured", type="negative")
            return
        
        self.status_label.text = "Transcribing audio..."
        
        try:
            # Create temporary file
            with tempfile.NamedTemporaryFile(suffix=f".{e.name.split('.')[-1]}", delete=False) as tmp:
                tmp.write(e.content)
                tmp_path = tmp.name
            
            # Transcribe
            with open(tmp_path, "rb") as f:
                lang_code = self.language_select.value or self.config.default_language
                transcript = transcribe_file(f, lang_code)
            
            # Clean up temp file
            Path(tmp_path).unlink()
            
            # Translate if enabled
            if self.translate_switch.value:
                target_code = self.target_select.value or self.config.default_target_language
                if target_code:
                    src_name = LANGUAGE_NAME.get(lang_code, "the source language")
                    tgt_name = LANGUAGE_NAME.get(target_code, target_code)
                    try:
                        self.status_label.text = "Translating text..."
                        transcript = translate_text(transcript, src_name, tgt_name)
                    except Exception as exc:
                        ui.notify(f"Translation error: {exc}", type="negative")
            
            # Update transcript
            self.transcript_text = transcript
            self.transcript_area.value = transcript
            self.status_label.text = "Transcription complete"
            ui.notify("Transcription completed successfully", type="positive")
            
        except Exception as exc:
            ui.notify(f"Transcription error: {exc}", type="negative")
            self.status_label.text = "Ready to transcribe audio"
    
    def copy_transcript(self):
        """Copy transcript to clipboard."""
        if not self.transcript_text.strip():
            ui.notify("No transcript to copy", type="warning")
            return
        
        # Use JavaScript to copy to clipboard
        ui.run_javascript(f'navigator.clipboard.writeText(`{self.transcript_text}`)')
        ui.notify("Transcript copied to clipboard", type="positive")
    
    def download_transcript(self):
        """Download transcript as text file."""
        if not self.transcript_text.strip():
            ui.notify("No transcript to download", type="warning")
            return
        
        # Create download
        ui.download(self.transcript_text.encode('utf-8'), "transcript.txt")
        ui.notify("Transcript download started", type="positive")
    
    def play_transcript(self):
        """Play transcript using TTS."""
        if not self.transcript_text.strip():
            ui.notify("No transcript to play", type="warning")
            return
        
        if not self.client:
            ui.notify("OpenAI API key not configured", type="negative")
            return
        
        self.status_label.text = "Generating audio..."
        
        # Determine voice based on selection (for simplicity, use female voice setting)
        voice = self.female_voice_select.value or self.config.female_voice
        
        def generate_audio():
            try:
                audio_data = synthesize_speech_wav(self.transcript_text, voice)
                
                # Create download for audio playback
                ui.download(audio_data, "transcript_audio.wav")
                ui.notify("Audio generated - download started", type="positive")
                
            except Exception as exc:
                ui.notify(f"Audio generation error: {exc}", type="negative")
            finally:
                self.status_label.text = "Ready to transcribe audio"
        
        threading.Thread(target=generate_audio, daemon=True).start()
    
    def preview_voice(self, voice_id: str):
        """Preview a voice with sample text."""
        if not voice_id:
            return
        
        if not self.client:
            ui.notify("OpenAI API key not configured", type="negative")
            return
        
        self.status_label.text = "Previewing voice..."
        
        def generate_preview():
            try:
                audio_data = synthesize_speech_wav(VOICE_SAMPLE_TEXT, voice_id)
                
                # Create download for preview
                ui.download(audio_data, f"voice_preview_{voice_id}.wav")
                ui.notify(f"Voice preview generated for {voice_id}", type="positive")
                
            except Exception as exc:
                ui.notify(f"Voice preview error: {exc}", type="negative")
            finally:
                self.status_label.text = "Ready to transcribe audio"
        
        threading.Thread(target=generate_preview, daemon=True).start()
    
    def open_settings(self):
        """Open settings dialog."""
        with ui.dialog() as dialog:
            with ui.card().classes('w-96'):
                ui.label("Settings").classes('text-xl font-bold mb-4')
                
                # Default language
                ui.label("Default Language")
                lang_select = ui.select(
                    options={lang["code"]: lang["name"] for lang in LANGUAGES},
                    value=self.config.default_language,
                ).classes('w-full mb-2')
                
                # Translation enabled
                ui.label("Enable Translation by Default")
                trans_switch = ui.switch(value=self.config.translation_enabled).classes('mb-2')
                
                # Default target language
                ui.label("Default Target Language")
                target_select = ui.select(
                    options={lang["code"]: lang["name"] for lang in LANGUAGES[1:]},
                    value=self.config.default_target_language,
                ).classes('w-full mb-2')
                
                # Female voice
                ui.label("Default Female Voice")
                female_select = ui.select(
                    options={voice[0]: voice[1] for voice in FEMALE_VOICES},
                    value=self.config.female_voice,
                ).classes('w-full mb-2')
                
                # Male voice
                ui.label("Default Male Voice")
                male_select = ui.select(
                    options={voice[0]: voice[1] for voice in MALE_VOICES},
                    value=self.config.male_voice,
                ).classes('w-full mb-4')
                
                with ui.row().classes('w-full justify-end gap-2'):
                    ui.button("Cancel", on_click=dialog.close).props('flat')
                    ui.button("Save", on_click=lambda: self.save_settings(
                        dialog,
                        lang_select.value,
                        trans_switch.value,
                        target_select.value,
                        female_select.value,
                        male_select.value
                    )).props('color=primary')
        
        dialog.open()
    
    def save_settings(self, dialog, default_lang, trans_enabled, target_lang, female_voice, male_voice):
        """Save settings to config file."""
        try:
            new_config = AppConfig(
                default_language=default_lang,
                default_target_language=target_lang,
                translation_enabled=trans_enabled,
                female_voice=female_voice,
                male_voice=male_voice,
            )
            new_config.save()
            self.config = new_config
            
            # Update UI to reflect new settings
            self.language_select.value = default_lang
            self.translate_switch.value = trans_enabled
            self.target_select.value = target_lang
            self.target_select.enabled = trans_enabled
            self.female_voice_select.value = female_voice
            self.male_voice_select.value = male_voice
            
            ui.notify("Settings saved successfully", type="positive")
            dialog.close()
            
        except Exception as exc:
            ui.notify(f"Failed to save settings: {exc}", type="negative")


@ui.page("/")
def index():
    """Main page of the web application."""
    global web_app
    if web_app is None:
        web_app = DictAiTeWebApp()
    web_app.build_ui()


def main(host: str = "127.0.0.1", port: int = 8080) -> int:
    """Main entry point for web interface."""
    try:
        # Set up the global app instance
        global web_app
        web_app = DictAiTeWebApp()
        
        # Run the server
        ui.run(host=host, port=port, title="dict-ai-te", show=False)
        return 0
        
    except Exception as exc:
        print(f"Failed to start web interface: {exc}")
        return 1


if __name__ in {"__main__", "__mp_main__"}:
    import sys
    sys.exit(main())