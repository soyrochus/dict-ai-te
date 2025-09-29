#!/usr/bin/env bash
# demoswitch.sh — quickly switch the repo to specific demo milestones and run the app.
#
# Usage:
#   ./demoswitch.sh <demo>
#   ./demoswitch.sh list   # show available demos
#
# Demos are mapped to specific commits below. After checkout, the script will
# launch the app with `uv run -m dictaite` unless `--no-run` is passed.
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

if ! command -v git >/dev/null 2>&1; then
  echo "Error: git is not installed or not in PATH" >&2
  exit 1
fi
if ! command -v uv >/dev/null 2>&1; then
  echo "Warning: uv is not installed; skipping auto-run. Install from https://github.com/astral-sh/uv" >&2
fi

RUN_APP=1
if [[ "${1-}" == "--no-run" ]]; then
  RUN_APP=0
  shift || true
fi

DEMO_NAME=${1-}
if [[ -z "$DEMO_NAME" ]]; then
  echo "Usage: $0 [--no-run] <demo>" >&2
  echo "Run '$0 list' to see available demos." >&2
  exit 1
fi

# Helper to run the app (if uv is available)
run_app() {
  if [[ $RUN_APP -eq 1 ]] && command -v uv >/dev/null 2>&1; then
    echo "\nStarting app: uv run -m dictaite\n"
    uv run -m dictaite
  else
    echo "\nSkipping app start. You can run it manually with: uv run -m dictaite\n"
  fi
}

# Normalize input (case-insensitive, allow short aliases)
key=$(echo "$DEMO_NAME" | tr '[:upper:]' '[:lower:]')

case "$key" in
  list|--list|-l)
    cat <<EOF
Available demos:
  basic            -> ecd6834b6b2661d8636e8e87d2e61520b0bc83f5
  translation      -> 6c7646aa5cebf73c0d3f75fea50451ec07dfc5ca
  playback         -> eaad8087740447cfce6b9253710ed25b06b6709d
  settings         -> 7089c4abc70ad44ba9b0169ea9a4fd634e5dd8ad
  web              -> ae81d7c94d955732e9db2bb0203cfade6d4f1d51
  rust            ->  7fdb1a358d7da6ccf77c23a10ec880c5eec388e4
EOF
    ;;

  basic|"basic version"|base)
    echo "Checking out 'Basic version'..."
    git checkout ecd6834b6b2661d8636e8e87d2e61520b0bc83f5 --quiet
    run_app
    ;;

  translation|"with translation"|trans)
    echo "Checking out 'With translation'..."
    git checkout 6c7646aa5cebf73c0d3f75fea50451ec07dfc5ca --quiet
    run_app
    ;;

  playback|"with playback"|play)
    echo "Checking out 'With Playback'..."
    git checkout eaad8087740447cfce6b9253710ed25b06b6709d --quiet
    run_app
    ;;

  settings|"with settings screen"|prefs)
    echo "Checking out 'With Settings screen'..."
    git checkout 7089c4abc70ad44ba9b0169ea9a4fd634e5dd8ad --quiet
    run_app
    ;;

  web|"with web interface"|webui)
    echo "Checking out 'With Web Interface'..."
    git checkout ae81d7c94d955732e9db2bb0203cfade6d4f1d51 --quiet
    run_app
    ;;

  rust|"Rust version"|rusty)
    echo "Checking out 'Rust version'..."
    git checkout 7fdb1a358d7da6ccf77c23a10ec880c5eec388e4 --quiet
    run_app
    ;;

  *)
    echo "Unknown demo: '$DEMO_NAME'" >&2
    echo "Run '$0 list' to see available demos." >&2
    exit 1
    ;;
 esac
