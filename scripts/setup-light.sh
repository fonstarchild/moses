#!/usr/bin/env bash
set -e
# For 8GB RAM systems — smaller model
ollama pull deepseek-coder:6.7b
ollama pull nomic-embed-text
echo "Light setup done. Using deepseek-coder:6.7b"
