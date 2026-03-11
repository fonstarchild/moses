#!/usr/bin/env bash
# Moses — one-click start script
set -e

OLLAMA_BIN=""
CARGO_BIN="$HOME/.cargo/bin/cargo"

# Find ollama
for p in ollama ~/bin/ollama /Applications/Ollama.app/Contents/Resources/ollama /usr/local/bin/ollama; do
  if [ -x "$p" ]; then OLLAMA_BIN="$p"; break; fi
done

if [ -z "$OLLAMA_BIN" ]; then
  echo "⚠️  Ollama not found. Install it from https://ollama.com"
  echo "   Or run: curl -fsSL https://ollama.com/install.sh | sh"
  exit 1
fi

if [ ! -x "$CARGO_BIN" ]; then
  echo "⚠️  Rust not found. Install it from https://rustup.rs"
  exit 1
fi

# Start Ollama if not running
if ! curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
  echo "▶  Starting Ollama..."
  "$OLLAMA_BIN" serve &>/tmp/moses-ollama.log &
  sleep 2
fi

# Check for a model — pull smallest if none present
MODELS=$("$OLLAMA_BIN" list 2>/dev/null | grep -c "deepseek" || echo 0)
if [ "$MODELS" -eq 0 ]; then
  echo "📥 No DeepSeek model found. Pulling deepseek-coder:6.7b (~3.8GB)..."
  echo "   (For better results, run: ollama pull deepseek-coder-v2:16b)"
  "$OLLAMA_BIN" pull deepseek-coder:6.7b
fi

# Update model in store default if only 6.7b is present
if "$OLLAMA_BIN" list 2>/dev/null | grep -q "deepseek-coder:6.7b"; then
  DEFAULT_MODEL="deepseek-coder:6.7b"
else
  DEFAULT_MODEL="deepseek-coder-v2:16b"
fi
echo "✓ Using model: $DEFAULT_MODEL"

# Kill any stale Vite / Moses processes from previous runs
lsof -ti:1420 -sTCP:LISTEN 2>/dev/null | xargs kill -9 2>/dev/null || true
lsof -ti:43210 -sTCP:LISTEN 2>/dev/null | xargs kill -9 2>/dev/null || true
pkill -9 -f "moses-desktop" 2>/dev/null || true
sleep 1

# Launch Moses
echo "🚀 Starting Moses..."
cd "$(dirname "$0")/apps/desktop"
"$CARGO_BIN" tauri dev
