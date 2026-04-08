# CLAUDE.md

## Project Overview

Forge is a native macOS IDE built in pure Rust with GPUI (Anthropic's UI framework). It features a file explorer, syntax-highlighted editor, integrated terminal, Git panel with AI commit messages, and Claude Code agent sessions.

**Repo:** `melvin-viougea/forge` on GitHub

## Commands

```bash
# Dev build
cargo build

# Release build
cargo build --release

# Type check only
cargo check

# Run locally (dev)
cargo run

# Release a new version (bumps version, builds, creates DMG, publishes GitHub release)
./scripts/release.sh <version> "<description>"
# Example: ./scripts/release.sh 1.2.0 "Dark mode improvements, terminal fix"

# Clear quarantine after install
xattr -cr /Applications/Forge.app
```

## Architecture

**Cargo workspace** with 6 crates:

| Crate | Name | Purpose |
|-------|------|---------|
| `crates/app` | `forge` | Main binary: AppView, window, updater, session, settings |
| `crates/workspace` | `ide_workspace` | Layout (docks, pane, tabs), theme system (6 themes), wallpaper |
| `crates/file_explorer` | `ide_file_explorer` | File tree with real-time watching (notify) + git status (git2) |
| `crates/terminal` | `ide_terminal` | PTY terminal via portable-pty + alacritty_terminal |
| `crates/git_panel` | `ide_git_panel` | Git status, diff viewer, commit/push/pull, AI commit messages |
| `crates/agent` | `ide_agent` | Claude Code agent session management |

**Key dependencies:** `gpui 0.2`, `tokio`, `git2 0.19`, `portable-pty 0.8`, `alacritty_terminal 0.26.0-rc1`, `notify 7`

**UI layout:** Three-column (left dock: projects | center: editor + terminal tabs | right dock: git/files/runner/log)

**External tools used at runtime:**
- `git` — all git operations
- `claude` CLI — AI commit message generation
- `curl` — update checker + DMG download
- `hdiutil` — DMG mount/unmount during updates

## Version Management

**Single source of truth:** `Cargo.toml` (workspace root) field `[workspace.package] version`

All crates inherit via `version.workspace = true`. The version propagates automatically to:
- `updater.rs` via `env!("CARGO_PKG_VERSION")` — used for update comparison
- `workspace.rs` titlebar via `env!("CARGO_PKG_VERSION")` — displayed as "FORGE vX.Y.Z"
- `Info.plist` — injected by `release.sh` at build time

**To bump the version, only change `Cargo.toml` at the workspace root.** Everything else is derived automatically.

## Release Checklist

When asked to release a new version:

1. **Commit all pending changes** to `main`
2. **Bump version** in `/Cargo.toml` → `[workspace.package] version = "<new>"`
3. **Run** `./scripts/release.sh <version> "<description>"`
   - This builds, creates .app bundle, packages DMG, and publishes to GitHub
4. **Verify** the release at `https://github.com/melvin-viougea/forge/releases`

The script handles: `cargo build --release` -> `.app` bundle -> `Info.plist` generation -> DMG packaging -> `gh release create`

**Do NOT manually edit version in:**
- `crates/*/Cargo.toml` (inherited from workspace)
- `updater.rs` (uses `env!("CARGO_PKG_VERSION")`)
- `workspace.rs` titlebar (uses `env!("CARGO_PKG_VERSION")`)

## Auto-Update System

`crates/app/src/updater.rs`:
- Checks GitHub API (`/repos/melvin-viougea/forge/releases/latest`) 3 seconds after launch
- Compares semver: `remote > local` via `Vec<u32>` comparison
- Downloads DMG to `/tmp/forge-update.dmg`, mounts, copies to `/Applications/Forge.app`, relaunches
- Shows green "Update Available" button in titlebar when newer version exists

## User Data

- `~/.forge/session.json` — project list, active project, panel dimensions
- `~/.forge/settings.json` — theme, wallpaper path, opacity, crop coordinates
- `~/.forge/wallpaper-cache/` — cached wallpaper images

## Themes

6 built-in themes defined in `crates/workspace/src/lib.rs`: Forge Dark (default), VS Code, Catppuccin Mocha, Nord, Dracula, One Dark. All support wallpaper translucency blending.

## Build Config

- Rust edition 2021, minimum 1.80 (stable)
- `.cargo/config.toml`: `rustflags = ["-C", "target-cpu=native"]` for native CPU optimization
- macOS only (uses .app bundles, hdiutil, xattr)
