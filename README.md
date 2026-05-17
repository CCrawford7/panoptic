# ◉ panoptic

**The all-seeing project dashboard for your development folders.**

Point it at any directory full of projects. Panoptic scans everything, detects project types, gathers git state, parses your `CLAUDE.md`/`AGENTS.md` files, and presents a beautiful bird's-eye view — either in the terminal (TUI) or as a web dashboard.

```bash
# TUI mode (default)
panoptic ~/projects

# Web dashboard
panoptic --web ~/projects
# → Opens http://localhost:3173
```

## Features

### 🔍 Universal Project Discovery
Auto-detects 10+ project types: Rust, TypeScript, JavaScript, Python, Godot, Go, Nix, Docker, Chrome Extensions, and more. Plugin-ready for custom detectors.

### 📂 Multi-Root Scanning
Scan multiple directories at once. Add, remove, or toggle scan roots at runtime via the web dashboard — no restart needed. Roots are persisted in `~/.config/panoptic/roots.toml` and survive across sessions.

```bash
# Scan multiple directories
panoptic --web ~/projects ~/work ~/games

# Roots persist — add more from the UI later
```

### 🔄 Git Awareness at a Glance
For every project, see branch, dirty status, staged/unstaged/untracked counts, ahead/behind remote, last commit message, total commits, and stash count.

### 📖 Project Documentation Reading
Automatically reads each project's `README.md`, `brief.md`, and `summary.md` to extract a one-line description of what the project *is*. No more guessing what an unfamiliar directory contains — the description is right there on the card.

### 📋 Agent Context Parsing
Reads your `CLAUDE.md`, `AGENTS.md`, `brief.md`, `PLAN.md`, and other project management files. Extracts current phase, task, next steps, blockers, checklist progress, and recent decisions.

### 🖥️ Dual Interface

**Terminal UI (TUI)** — Keyboard-driven grid with:
- Color-coded project cards with activity indicators
- Filter by activity, type, or search
- Detail view with full git and agent context
- Vim-style keybindings (`j/k` to navigate, `/` to search)

**Web Dashboard** — SPA with:
- Responsive card grid with live search and filtering
- Click-through detail modal with full git and agent context
- REST API at `/api/projects`, `/api/stats`, `/api/roots`
- **Multi-root management**: add, remove, enable/disable scan roots at runtime
- Auto-refresh every 60 seconds

### 📊 Exportable
```bash
panoptic --json    # Output project data as JSON for scripting
```

## Quick Start

### Install

```bash
# Via Cargo (from source — requires Rust)
cargo install --git https://github.com/CCrawford7/panoptic

# Or download a pre-built binary from GitHub Releases
```

### Usage

```bash
# Scan current directory
panoptic

# Scan a specific directory
panoptic ~/code

# Scan multiple directories (merged into one dashboard)
panoptic ~/projects ~/work ~/games

# Web dashboard
panoptic --web ~/projects

# Web dashboard with multiple scan roots
panoptic --web ~/projects ~/work

# Custom port
panoptic --web -p 8080 ~/projects

# JSON output for scripting
panoptic --json ~/projects > projects.json

# Custom scan depth
panoptic --max-depth 5 ~/projects

# Show hidden directories
panoptic --show-hidden ~/projects

# Custom config file
panoptic -c ~/.config/panoptic/config.toml ~/projects
```

### Keybindings (TUI)

| Key | Action |
|-----|--------|
| `↑`/`k`, `↓`/`j` | Navigate projects |
| `←`/`→` | Navigate grid |
| `Enter` | Open detail view |
| `Tab` | Cycle filter |
| `1`-`7` | Quick filter (All, Active, Stable, Stale, Game, Tool, Web) |
| `/` | Search |
| `r` | Refresh scan |
| `?`/`h` | Toggle help |
| `q`/`Esc` | Quit |

## Configuration

Panoptic looks for `panoptic.toml` in the scanned directory, or you can specify a path with `-c`.

Scan roots are persisted globally in `~/.config/panoptic/roots.toml` and survive across restarts. You can manage roots from the web dashboard or by editing the file directly.

```toml
# panoptic.toml
max_depth = 3
web_port = 3173
web_open_browser = true
show_hidden = false
ignore_dirs = ["node_modules", "target", ".git", ".next"]
```

## How It Works

1. **Roots** — you specify one or more scan directories. These are persisted in `~/.config/panoptic/roots.toml` and can be managed at runtime from the web dashboard.
2. **Walk** — traverses each root up to `max_depth`, looking for project indicators (`.git`, `Cargo.toml`, `package.json`, etc.). Sub-projects nested inside a parent project are collapsed.
3. **Detect** — identifies project type, calculates size, counts files
4. **Git status** — opens each repo with libgit2, gathers full state
5. **Parse** — reads CLAUDE.md/AGENTS.md/brief.md/PLAN.md for structured context
6. **Present** — renders in TUI or web dashboard

## Architecture

```
panoptic/
├── src/
│   ├── main.rs      # CLI entry point
│   ├── lib.rs       # Library root
│   ├── config.rs    # Configuration
│   ├── project.rs   # Data model
│   ├── roots.rs     # Multi-root persistence & management
│   ├── scanner.rs   # Project discovery & detection
│   ├── git.rs       # Git state via libgit2
│   ├── parser.rs    # Agent file parsing
│   ├── tui/         # Terminal UI (ratatui)
│   │   └── app.rs   # TUI application
│   └── web/         # Web server (axum)
│       ├── mod.rs   # Server setup
│       └── routes.rs # REST API + static files
├── static/          # Web frontend (HTML/CSS/JS)
└── Cargo.toml
```

### Tech Stack

- **Language:** Rust
- **TUI:** ratatui + crossterm
- **Web:** axum + rust-embed
- **Git:** git2 (libgit2 bindings)
- **Scanning:** walkdir + rayon (parallel)

## Roadmap

- [x] Multi-root scanning with runtime management
- [ ] Dependency parsing (Cargo.toml, package.json, pyproject.toml, go.mod)
- [ ] Quick actions (open in editor/terminal/file manager/GitHub)
- [ ] Tags, notes, and user-set project status
- [ ] Context export for AI sessions (`panoptic export`)
- [ ] File watching / live updates
- [ ] Full-text search across all project docs (tantivy)
- [ ] Project health scoring
- [ ] File composition breakdown (file types, LOC)
- [ ] Activity heatmaps and commit frequency charts
- [ ] Project scaffolding (`panoptic new <type> <name>`)
- [ ] Plugin system for custom detectors/parsers
- [ ] Homebrew and npm distribution

## License

MIT
