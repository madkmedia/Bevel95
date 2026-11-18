# Contributing to Bevel95

Thank you for wanting to help. Bevel95 aims to be a *working*, stable taskbar first and complete
second, and to be exact to the pixel. These notes say how the project is put together and what a
change needs before it can go in. Open an issue to talk about a larger change before you start.

## The layers, and what may go where

The Cargo workspace's crates are its layers:

- `core/` (`bevel-core`): the looks, layout, painting into a picture at 1x, state machines, text
  from glyph atlases, the games' and the Calculator's rules. `#![forbid(unsafe_code)]`, **no
  Windows types**, and no dependency except `art/`. Everything here is tested without a window.
- `art/` (`bevel-art`): **the only place for pictures, sounds, logos and screen fonts taken from
  Windows**, as data and nothing else (they are Microsoft's, not covered by Bevel95's licence:
  [README.md](README.md#license)). A test (`art/tests/one_place.rs`) fails when such material
  appears anywhere else.
- `platform/winkit/` (`bevel-winkit`): the Windows side every program shares (fonts, the window
  host for forms, menus, the log).
- `platform/win/` (`bevel95`, the taskbar's program) and `programs/<name>/` (one program each):
  the Windows glue. **All `unsafe` code lives in `platform/` and `programs/`**, each block with a
  `// SAFETY:` comment.

The only runtime dependency is `windows-sys`, pinned with `=`. **Do not add a dependency without
discussing it in an issue first**; dev-dependencies for tests are easier to agree on. Never guess
a `windows-sys` item: check that it exists in that version's source.

Things Bevel95 never does: inject code into other programs, hook or patch Explorer, replace the
shell, call `SetSysColors`, or write to the registry beyond its Run entry and its line in the list
of installed apps. There is no code signing.

## Each look stays pure

Windows 95, 98 and 2000 each keep their own pictures and behaviour. Never mix one Windows'
pictures or behaviour into another's look. Where a Windows had nothing for a thing, use what that
Windows did have (its own controls, its own icons), not something borrowed from another.

## What pixel-exact means here, and how it is checked

Every look is drawn at 1x into a picture and enlarged by a whole number, nearest neighbour; there
is no smoothing anywhere. Text comes from glyph atlases filled by GDI at 96 DPI without
antialiasing (MS Sans Serif 8pt for 95 and 98, Tahoma 8pt for 2000).

- **Golden pictures** (`core/tests/golden/*.png`) are fixed states drawn at 1x and compared pixel
  for pixel by `cargo test`. When one differs, the test writes `<name>.actual.png` and
  `<name>.diff.png` beside it. Regenerate them (`cargo regen-golden`) only for an intended change
  of the look, and say why in the pull request.
- **Font fixtures** (`core/tests/fixtures/*.font`) hold the Windows fonts as text, so the core's
  tests need no Windows; a Windows-only test checks them against GDI.
- **The comparison against real Windows** (`cargo compare-refs`) draws the same states as
  captures of real Windows 95, 98 and 2000 and compares them pixel for pixel. Most of those
  captures come from the maintainers' own virtual PCs and are **not public**, so a contributor
  cannot run it in full. Without them, `cargo test --workspace` checks everything else. Point
  `BEVEL95_PRIVATE` at a folder laid out like the maintainers' (captures in its
  `reference/refs`; see `docs/decisions.md`, "The public copy") if you have your own captures.
- **A measured value needs its source named**: which Windows, which capture or which program
  file it was read from, in the code's comment and in `docs/theme-spec.md`. A guessed value is
  marked as provisional.

## Trying a change

```
cargo test --workspace
cargo clippy --workspace --all-targets
```

Both must be clean. To use your build every day, `powershell -NoProfile -ExecutionPolicy Bypass
-File tools\install.ps1` builds the release programs and installs them into
`%LOCALAPPDATA%\Bevel95` with the two desktop shortcuts, restarting a running Bevel95.
`cargo selftest` starts the real taskbar and checks that Windows' taskbar always comes back (it
hides Windows' taskbar for a few seconds).

Please keep the style of the code around your change. `cargo fmt` would reflow many existing
files: run rustfmt only on new files. A change that the person using Bevel95 can see also updates
the docs (`docs/decisions.md` for why, `docs/theme-spec.md` for measured values).

## Your contribution's licence

By sending a contribution you agree that it is given under the project's licence, the GNU General
Public License version 3 or later ([LICENSE](LICENSE)). Do not send pictures, sounds or text
taken from Windows or other people's work outside `art/`, and only with their source named.
