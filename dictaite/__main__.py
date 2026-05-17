"""Entry point for the GTK application."""

from .ui_gtk.bootstrap import ensure_macos_gtk_paths

ensure_macos_gtk_paths("dictaite")

from .ui_gtk.app import main


def run() -> int:
    return main()


if __name__ == "__main__":  # pragma: no cover - CLI entry
    import sys

    raise SystemExit(run())
