#!/usr/bin/env bash
set -e

echo "=== Moses Setup ==="

# Check Ollama
if ! command -v ollama &>/dev/null; then
  echo "Installing Ollama..."
  curl -fsSL https://ollama.ai/install.sh | sh
fi

# Pull models
echo "Pulling DeepSeek models..."
ollama pull deepseek-coder-v2:16b
ollama pull nomic-embed-text

echo ""
echo "=== Done! Models ready. ==="
echo "Run: pnpm install && pnpm tauri dev"
