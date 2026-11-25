# Magpie

A local-first bookmark manager for Windows. Import from Chrome, Edge or an HTML file, organise links into colour-coded categories, and get warned about unsafe sites before you open them. Optional AI suggestions use your own API key.

Built with **Tauri 2**, **Rust**, **React + TypeScript** and **SQLite**.

## Status

Milestone 1 (core bookmark manager, browser import, previews, search, local URL checks) is in progress.

## Design

The UI follows the Figma file *bookmark* — page **Bookmark Manager - version 2**. Colour tokens in `src/index.css` mirror the Figma variable collection *Magpie Tokens*.

## Development

Requirements: Windows 10 1803+ or Windows 11, [Rust](https://rustup.rs), Node 20+, and the Visual Studio C++ build tools (see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)).

```powershell
npm install
npm run tauri dev
```

Other commands:

| Command | What it does |
| --- | --- |
| `npm run typecheck` | TypeScript type-check |
| `npm test` | Frontend unit tests (Vitest) |
| `cargo test` (in `src-tauri`) | Rust unit tests |
| `npm run tauri build` | Build the NSIS and MSI installers |

## Project layout

```
src/             React frontend
src-tauri/       Rust backend (commands, database, services)
design/          Design assets exported from Figma
.github/         CI workflow (Windows build + tests on every PR)
```

## Privacy

Your library is stored only on your PC. There is no account, no cloud sync and no telemetry. API keys are kept in Windows Credential Manager, never in the database.
