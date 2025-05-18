import tkinter as tk
from tkinter import filedialog, messagebox
import time
import threading

class DictAiTeApp:
    def __init__(self, root):
        self.root = root
        self.root.title("dict-ai-te")

        # State
        self.is_recording = False
        self.start_time = None
        self.elapsed_seconds = 0
        self.timer_thread = None

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

    def toggle_recording(self, event=None):
        if not self.is_recording:
            self.is_recording = True
            self.start_time = time.time()
            self.status_label.config(text="Recording... Press to stop.")
            self.btn_canvas.itemconfig(self.circle, fill="#44c767", outline="#33a457")
            self.btn_canvas.itemconfig(self.btn_text, text="⏸")
            self.timer_thread = threading.Thread(target=self.update_timer)
            self.timer_thread.daemon = True
            self.timer_thread.start()
        else:
            self.is_recording = False
            self.status_label.config(text="Press to start recording")
            self.btn_canvas.itemconfig(self.circle, fill="#fa5457", outline="#e03e41")
            self.btn_canvas.itemconfig(self.btn_text, text="🎤")

    def update_timer(self):
        while self.is_recording:
            self.elapsed_seconds = int(time.time() - self.start_time)
            h = self.elapsed_seconds // 3600
            m = (self.elapsed_seconds % 3600) // 60
            s = self.elapsed_seconds % 60
            timer_str = f"{h:02}:{m:02}:{s:02}"
            self.timer_label.config(text=timer_str)
            time.sleep(1)

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
            with open(file_path, "w", encoding="utf-8") as f:
                f.write(text)
            messagebox.showinfo("Saved", f"Transcript saved to:\n{file_path}")

if __name__ == "__main__":
    root = tk.Tk()
    app = DictAiTeApp(root)
    root.mainloop()
