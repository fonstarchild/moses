# Moses

**A free, local AI coding assistant. No cloud. No subscriptions. No API keys. Yours forever.**

Moses is an open-source desktop app that brings an AI coding assistant to your machine — similar to Claude Code or GitHub Copilot, but running 100% offline. On first launch it downloads the AI model automatically. Just open the app.

> Built for every developer in the world, regardless of budget.

---

## What Moses can do

### Talk to your codebase
Ask anything about your project. Moses reads and understands your files, builds a searchable index, and uses it to give accurate answers.

> *"How does the authentication middleware work?"*
> *"Where is the database connection initialized?"*
> *"Explain the data flow in this module."*

### Edit files autonomously
Tell Moses what to change — it writes the code and asks for your approval before saving anything.

> *"Add input validation to the registration endpoint."*
> *"Extract this function into a separate utility module."*
> *"Refactor this to use async/await."*

### Create new files
Moses creates files in the right place, following your project's conventions.

> *"Create a test file for this module."*
> *"Generate a SQL migration for the users table."*
> *"Build an auth module with JWT support."*

---

## Download & install

Get the installer for your OS from the [Releases page](../../releases) and run it normally:

| Platform | Installer | Notes |
|----------|-----------|-------|
| **macOS** (Apple Silicon) | `Moses_*_aarch64.dmg` | Open DMG → drag to Applications → [see note below](#macos-note) |
| **macOS** (Intel) | `Moses_*_x64.dmg` | Open DMG → drag to Applications → [see note below](#macos-note) |
| **Linux** | `Moses_*_amd64.AppImage` | Make executable → double-click |
| **Linux** (Debian/Ubuntu) | `Moses_*_amd64.deb` | `sudo dpkg -i Moses_*.deb` |
| **Windows** | `Moses_*_x64-setup.exe` | Run installer → launch from Start |

**That's it.** On first launch Moses automatically:
1. Detects whether Ollama is installed — downloads it if not (~50 MB)
2. Downloads the AI model (~4 GB, one time only)
3. Opens the main editor, ready to use

No terminal. No config. No account. Just open the app.

#### macOS note

Because Moses is not yet notarized with Apple, macOS may say **"the image is damaged"** or block the app on first launch. This is a Gatekeeper warning, not actual damage.

**Fix — one of these:**

- Right-click the app → **Open** → **Open anyway**
- Or run this once in Terminal after dragging to Applications:
  ```bash
  xattr -cr /Applications/Moses.app
  ```

Then double-click normally. You won't need to do this again.

---

## How to use Moses

### 1. Open a workspace
Click **"Open Workspace"** (top-left folder icon) and select your project folder. Moses indexes the codebase automatically — you'll see the chunk count in the status bar. Your last workspace is remembered on next launch.

### 2. Select a file
Click any file in the left sidebar to open it in the editor. Moses uses the open file as context for your requests.

### 3. Ask anything
Type your request in the chat panel (right side) and press **Cmd+Enter** (macOS) or **Ctrl+Enter** (Linux/Windows).

Moses responds naturally — explains, asks clarifying questions, or writes code directly.

### 4. Approve file writes
When Moses wants to create or edit a file, a yellow confirmation bubble appears in the chat:

```
Moses wants to write:
src/auth/jwt.ts
[Allow]  [Deny]
```

Click **Allow** to save the file, **Deny** to skip it. The file explorer updates automatically.

### 5. Clear the chat
Click the **✕** button to clear the conversation and start fresh.

---

## How it works

```
You type a request
        ↓
Moses reads the file tree + open file + relevant code (semantic search)
        ↓
The AI model responds — explains, asks, or writes code
        ↓
If code is produced → Moses shows a confirmation bubble
        ↓
You click Allow → file is saved and appears in the file explorer
```

Everything runs locally. Your code never leaves your machine.

---

## Development setup

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| **Rust** | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Node.js** | 18+ | [nodejs.org](https://nodejs.org) or `nvm install 20` |
| **Ollama** | any | [ollama.com](https://ollama.com) |

> **Linux only:** install system dependencies first:
> ```bash
> sudo apt-get install libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev \
>   libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf
> ```

### Run in dev mode

```bash
# 1. Clone
git clone https://github.com/your-org/moses.git
cd moses

# 2. Install frontend dependencies (from repo root)
pnpm install

# 3. Launch everything with one command
bash start.sh
```

`start.sh` will:
- Check that Ollama is installed (exits with instructions if not)
- Start Ollama if it isn't running
- Pull `deepseek-coder:6.7b` if no DeepSeek model is installed (~3.8 GB, one time)
- Kill any stale processes on ports 1420 and 43210
- Run `cargo tauri dev` with hot reload

> **First compile takes a few minutes** — Rust builds ~200 crates. Subsequent starts are fast.

### Useful commands

```bash
# Run Rust unit tests
cd apps/desktop/src-tauri
cargo test

# Type-check the frontend (no emit)
cd apps/desktop
pnpm run typecheck

# Fast Rust compile check (no linking)
cd apps/desktop/src-tauri
cargo check

# Run Clippy linter
cd apps/desktop/src-tauri
cargo clippy

# Build a release binary for your current OS
cd apps/desktop
pnpm tauri build
# Output: apps/desktop/src-tauri/target/release/bundle/
```

### Pull a better model (optional)

```bash
ollama pull deepseek-coder-v2:16b   # best quality — needs 16 GB RAM
ollama pull deepseek-r1:14b         # strong reasoning — needs 16 GB RAM
```

Moses always uses whichever DeepSeek model Ollama has available. The larger the model, the better the results.

---

## Release a new version

Releases are fully automated by GitHub Actions.

```bash
# Tag a version — this triggers the pipeline
git tag v0.2.0
git push origin v0.2.0
```

The pipeline will:
1. **Run all tests** (Rust unit tests + clippy + frontend type-check) — if any fail, the build is cancelled
2. **Build installers** for macOS arm64, macOS x64, Linux x64, and Windows x64 in parallel
3. **Create a draft GitHub Release** with all 4 installers attached

You review and publish the draft release when ready.

You can also trigger a build manually from the **Actions** tab without creating a tag.

---

## Repository structure

```
moses/
├── .github/workflows/
│   ├── ci.yml              # Runs on every PR: Rust tests + clippy + tsc
│   └── release.yml         # Triggered by git tag: tests → build all platforms
├── apps/desktop/
│   ├── src/                # React + TypeScript frontend
│   │   ├── App.tsx         # Root layout: editor + file tree + chat
│   │   ├── store/          # Zustand global state
│   │   └── components/
│   │       ├── Chat/       # Streaming chat + Allow/Deny confirm bubbles
│   │       ├── FileExplorer/
│   │       ├── ModelControl/  # Ollama status dot + re-index button
│   │       ├── StatusBar/
│   │       └── Setup/      # First-launch progress screen
│   └── src-tauri/src/      # Rust backend
│       ├── main.rs         # Tauri command registry
│       ├── setup.rs        # Auto-install Ollama + pull model on first launch
│       ├── settings.rs     # Persist workspace to ~/.moses/settings.json
│       ├── agent/
│       │   ├── loop_.rs    # Core: context → model → extract code → confirm → write
│       │   └── tests.rs    # Unit tests (19 tests)
│       ├── llm/client.rs   # Ollama streaming HTTP client, special token stripping
│       ├── workspace/
│       │   ├── indexer.rs  # Declaration-aware code chunker
│       │   ├── vector_store.rs  # SQLite FTS5 semantic search (BM25)
│       │   └── watcher.rs  # File watcher — re-indexes on save (800ms debounce)
│       ├── memory/
│       │   ├── short_term.rs    # In-memory conversation history (128k token budget)
│       │   └── long_term.rs     # SQLite project knowledge base
│       └── patch/apply.rs  # Unified diff parser and applier
├── CHANGELOG.md
├── start.sh                # One-command dev launcher
└── README.md
```

---

## Contributing

Moses is built for every developer in the world — especially those who can't afford or don't want cloud AI tools. Contributions that serve that mission are always welcome.

### What we're looking for

- **Bug fixes** — anything that makes Moses more reliable on any platform
- **Model compatibility** — improvements that work with more Ollama models
- **Performance** — faster indexing, smarter context, lower memory usage
- **Platform support** — better Linux and Windows experience
- **Accessibility** — Moses should work well on modest hardware
- **Translations** — UI strings and documentation in other languages

### What Moses is (and should stay)

Moses is a **free, offline, privacy-respecting coding assistant**. Every contribution should serve that core purpose:

- ✅ Works without internet after first setup
- ✅ Never sends code or data to any server
- ✅ Runs on consumer hardware (8 GB RAM minimum)
- ✅ Free to use, forever, for anyone

### What we won't add

- Cloud sync, telemetry, or analytics of any kind
- Paywalls, feature tiers, or license checks
- Dependencies that require a paid account
- Features that only work with specific commercial models

### How to contribute

1. **Fork** the repo and create a branch: `git checkout -b fix/your-thing`
2. **Make your change** and add tests if applicable
3. **Run checks locally:**
   ```bash
   cd apps/desktop/src-tauri && cargo test && cargo clippy
   cd apps/desktop && pnpm run typecheck
   ```
4. **Open a PR** with a clear description of what changed and why

For larger changes, open an issue first to discuss the approach.

### Code style

- Rust: standard `rustfmt` formatting (`cargo fmt`)
- TypeScript: keep it simple, no unnecessary abstractions
- Keep the codebase tight and readable — no bloat

---

## License

MIT — do whatever you want with it. Just keep it free.
