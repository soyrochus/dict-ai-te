"""GTK front-end package for dict-ai-te."""

__all__ = ["main"]


def main() -> int:
    from .app import main as app_main

    return app_main()
