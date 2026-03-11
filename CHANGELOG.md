# Changelog

All notable changes to Moses are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Moses uses [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added
- Permission gate: Moses asks before writing any file
- File explorer auto-refreshes when Moses creates a file
- Semantic search over indexed codebase on every request
- Special token stripping (removes DeepSeek tokenizer artifacts from output)
- Unit tests for chunking, code block extraction, token stripping, path inference
- CI pipeline (Rust tests + clippy + frontend type-check on every PR)
- CHANGELOG

### Changed
- Agent loop simplified to reliable one-shot pattern
- System prompt tuned for natural conversation + code generation
- Context window increased to 32k tokens, max output 8k tokens
- File tree capped at 1500 chars to preserve context for code
- Status bar cleaned up — removed model name

### Fixed
- Reload (Cmd+R) no longer gets stuck on setup screen
- Truncated code blocks (model cut off mid-file) are now rejected
- DeepSeek `<｜begin▁of▁sentence｜>` tokens no longer leak into output

---

## [0.1.0] - 2026-03-11

### Added
- Initial release
- Tauri desktop app (macOS, Linux, Windows)
- Monaco editor with file explorer
- Chat panel with streaming responses
- Workspace indexing (SQLite FTS5, BM25 search)
- File watcher with incremental re-indexing
- Auto-setup: installs Ollama and pulls DeepSeek model on first launch
- GitHub Actions release pipeline for all 3 platforms
- VSCode extension bridge (WebSocket on port 43210)
