"""Minimal dict-ai-te desktop recorder/transcriber."""

from __future__ import annotations

import os
import tempfile
import threading
import time
import tkinter as tk
from tkinter import filedialog, messagebox, Tk, Canvas, Label, Text, Button

import numpy as np
import sounddevice as sd
import soundfile as sf
from dotenv import load_dotenv
from openai import OpenAI

class DictAiTeApp:
    """Main application window."""

    def __init__(self, root: Tk) -> None:
        load_dotenv()
        self.client = None
        api_key = os.getenv("OPENAI_API_KEY")
        if api_key:
            self.client = OpenAI(api_key=api_key)

        self.root = root
        self.root.title("dict-ai-te")

        # State
        self.is_recording = False
        self.start_time: float | None = None
        self.elapsed_seconds = 0
        self.timer_thread: threading.Thread | None = None

        # audio
        self.stream: sd.InputStream | None = None
        self.audio_frames: list[np.ndarray] = []

        # Layout
        self.create_widgets()

    def create_widgets(self):
        # Waveform mock (colored bar)
        self.waveform = tk.Canvas(self.root, width=400, height=40, bg="#e0e0e0", highlightthickness=0)
        self.waveform.pack(pady=(10,0))

        # Circle record button on Canvas
        self.btn_canvas = tk.Canvas(self.root, width=120, height=120, bg=self.root["bg"], highlightthickness=0)
        self.btn_canvas.pack()
        self.circle = self.btn_canvas.create_oval(10, 10, 110, 110, fill="#fa5457", outline="#e03e41", width=4)
        self.btn_text = self.btn_canvas.create_text(60, 60, text="🎤", font=("Arial", 36))
        self.btn_canvas.bind("<Button-1>", self.toggle_recording)

        # Stop label
        self.status_label = tk.Label(self.root, text="Press to start recording", font=("Arial", 12))
        self.status_label.pack()

        # Timer label
        self.timer_label = tk.Label(self.root, text="00:00:00", font=("Arial", 10))
        self.timer_label.pack(pady=(0,10))

        # Transcript Text area
        self.text_area = tk.Text(self.root, height=12, width=50, font=("Arial", 12))
        self.text_area.pack(padx=10, pady=(5,10))

        # Save Button
        self.save_btn = tk.Button(self.root, text="Save Transcript", command=self.save_transcript)
        self.save_btn.pack(pady=(0,10))

    # ------------------------------------------------------------------
    # Recording control
    # ------------------------------------------------------------------

    def toggle_recording(self, event=None):
        if not self.is_recording:
            self.is_recording = True
            self.start_time = time.time()
            self.status_label.config(text="Recording... Press to stop.")
            self.btn_canvas.itemconfig(self.circle, fill="#44c767", outline="#33a457")
            self.btn_canvas.itemconfig(self.btn_text, text="⏸")
            self.start_recording()
            self.timer_thread = threading.Thread(target=self.update_timer, daemon=True)
            self.timer_thread.start()
        else:
            self.is_recording = False
            self.btn_canvas.itemconfig(self.circle, fill="#fa5457", outline="#e03e41")
            self.btn_canvas.itemconfig(self.btn_text, text="🎤")
            self.stop_recording()

    def update_timer(self):
        while self.is_recording:
            self.elapsed_seconds = int(time.time() - self.start_time)
            h = self.elapsed_seconds // 3600
            m = (self.elapsed_seconds % 3600) // 60
            s = self.elapsed_seconds % 60
            timer_str = f"{h:02}:{m:02}:{s:02}"
            self.timer_label.config(text=timer_str)
            time.sleep(1)

    # ------------------------------------------------------------------
    # Audio handling
    # ------------------------------------------------------------------

    def start_recording(self) -> None:
        """Begin audio capture from the microphone."""
        self.audio_frames.clear()
        try:
            self.stream = sd.InputStream(
                samplerate=16000,
                channels=1,
                callback=self.audio_callback,
            )
            self.stream.start()
        except Exception as exc:  # pragma: no cover - runtime error path
            messagebox.showerror("Recording error", str(exc))
            self.is_recording = False

    def stop_recording(self) -> None:
        """Stop audio capture and trigger transcription."""
        if self.stream:
            try:
                self.stream.stop()
                self.stream.close()
            except Exception as exc:  # pragma: no cover - runtime error path
                messagebox.showerror("Recording error", str(exc))
        self.stream = None
        self.status_label.config(text="Transcribing...")
        threading.Thread(target=self.transcribe_audio, daemon=True).start()

    def audio_callback(self, indata, frames, time_info, status) -> None:
        if status:
            print(status)
        self.audio_frames.append(indata.copy())

    def transcribe_audio(self) -> None:
        """Send recorded audio to OpenAI and display the result."""
        if not self.client:
            messagebox.showerror("Configuration", "OpenAI API key not configured")
            self.status_label.config(text="Press to start recording")
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
            self.text_area.delete("1.0", tk.END)
            self.text_area.insert("1.0", transcript)
        except Exception as exc:  # pragma: no cover - network path
            messagebox.showerror("Transcription error", str(exc))
        finally:
            self.status_label.config(text="Press to start recording")

    def save_transcript(self):
        text = self.text_area.get("1.0", tk.END).strip()
        if not text:
            messagebox.showwarning("No text", "Transcript area is empty.")
            return
        file_path = filedialog.asksaveasfilename(
            defaultextension=".txt",
            filetypes=[("Text files", "*.txt"), ("All files", "*.*")]
        )
        if file_path:
            try:
                with open(file_path, "w", encoding="utf-8") as f:
                    f.write(text)
                messagebox.showinfo("Saved", f"Transcript saved to:\n{file_path}")
            except OSError as exc:  # pragma: no cover - file system
                messagebox.showerror("Save failed", str(exc))

if __name__ == "__main__":
    root = tk.Tk()
    app = DictAiTeApp(root)
    root.mainloop()
