# Part of dictaite: Recording, transcribing, and translating voice notes | Copyright (c) 2025 | License: MIT
"""dict-ai-te main entry point with argument parsing."""

from __future__ import annotations

import argparse
import sys


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="dict-ai-te: Voice recording, transcription, and translation")
    parser.add_argument("--web", action="store_true", help="Launch web interface instead of GTK")
    parser.add_argument("--host", default="127.0.0.1", help="Host for web interface (default: 127.0.0.1)")
    parser.add_argument("--port", type=int, default=8080, help="Port for web interface (default: 8080)")
    
    # Filter out the script name if present
    if argv and len(argv) > 0 and argv[0].endswith('__main__.py'):
        argv = argv[1:]
    
    args = parser.parse_args(argv)
    
    if args.web:
        try:
            from .web import main as web_main
            return web_main(host=args.host, port=args.port)
        except ImportError:
            print("NiceGUI is not installed. Install it with: pip install nicegui")
            print("Or use uv to add it: uv add nicegui")
            return 1
    else:
        # Launch GTK interface (default)
        try:
            from .gtk_app import main as gtk_main
            return gtk_main(argv)
        except ImportError as e:
            print(f"GTK is not available: {e}")
            print("Try using the web interface with --web flag")
            return 1


if __name__ == "__main__":  # pragma: no cover - CLI entry
    sys.exit(main(sys.argv))
