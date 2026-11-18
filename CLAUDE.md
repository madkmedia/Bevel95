# Bevel95: notes for AI coding helpers

Bevel95 is a Windows 95 / 98 / 2000 style taskbar, Start menu, desktop and classic programs for
Windows 11: a standalone program that paints every pixel itself. No theme, no Explorer patch, no
injection. A working, stable taskbar matters more than completeness. Read
[CONTRIBUTING.md](CONTRIBUTING.md) first; the reasons behind every part are in
[docs/decisions.md](docs/decisions.md) (read the section for the area you change), measured
pixels and their sources in [docs/theme-spec.md](docs/theme-spec.md), what is not built yet in
[docs/roadmap.md](docs/roadmap.md).

## Rules
- Rust stable, MSVC, a statically linked C runtime (`.cargo/config.toml`),
  `windows_subsystem = "windows"`. The only runtime dependency is `windows-sys`, pinned with `=`;
  never guess one of its item paths: check the crate's source. No new crate without asking.
- The layers are crates: `core/` (`#![forbid(unsafe_code)]`, no Windows types, testable
  headless; golden pictures in `core/tests/golden/`, font fixtures in `core/tests/fixtures/`),
  `art/` (Windows' own pictures, sounds and fonts, data only, nowhere else: a test enforces it),
  `platform/winkit/` (the Windows side every program shares), `platform/win/` (the taskbar,
  `bevel95.exe`), `programs/<name>/` (one executable each). All `unsafe` stays in `platform/`
  and `programs/`, each block with a `// SAFETY:` comment.
- Never: `SetSysColors`, DLL injection, shell replacement, hooking Explorer. Registry: only the
  Run entry ("Start with Windows") and the installed-apps entry.
- Rendering: paint at 1x, enlarge by a whole number nearest-neighbour, copy 1:1; no smoothing.
  Per-monitor-v2 DPI aware. Text from glyph atlases filled by GDI at 96 DPI, not antialiased:
  MS Sans Serif 8pt (95, 98), Tahoma 8pt (2000).
- Each look stays pure: never mix one Windows' pictures or behaviour into another's look.
- Log Win32 calls that can fail silently (`%LOCALAPPDATA%\Bevel95\bevel95.log`), but never
  text from outside Bevel95 (window titles, file names, tooltips): a test guards it.
- Windows' taskbar must always come back: do not weaken the watchdog, the panic hook or
  `--restore`.

## How to work
- After a change: `cargo test --workspace` and `cargo clippy --workspace --all-targets`, both
  clean. Golden pictures are regenerated (`cargo regen-golden`) only for an intended change of
  the look. `cargo compare-refs` needs reference captures that are not public
  (`BEVEL95_PRIVATE`, `BEVEL95_REFS`).
- `tools\install.ps1` builds and installs the release programs into `%LOCALAPPDATA%\Bevel95`.
- `cargo fmt` would reflow many files: run rustfmt only on new files.
- Tracked files use LF line endings, except README.md (CRLF).
- Live tests must not take the user's mouse or keyboard for repetitions, and must not close or
  change the user's own windows and files: use test windows and scratch folders.
