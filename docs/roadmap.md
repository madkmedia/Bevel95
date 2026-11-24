# What is not built yet

Bevel95 works as a daily taskbar, but much of the three Windows is still missing. This is the
short list, grouped; the reasons behind what exists are in [decisions.md](decisions.md). Nothing
here is promised, and an issue is the place to say what matters to you.

## The taskbar and Start menu
- A taskbar on every screen (today one taskbar on the main screen lists the windows of all
  screens, as Windows 98 and 2000 did), the taskbar on the other edges.
- Quick Launch's rest: Show Text, Show Title, large icons, dragging shortcuts onto it, the other
  entries of an icon's menu (Send To, Cut, Copy, Rename).
- Balloon messages in the classic style (today Windows 11 shows them as its own notifications),
  the notification area by keyboard (Win+B).
- Windows 98's menus sliding open; Windows 2000's personalized menus and five of its Start Menu
  settings.
- Programs that ask where the taskbar is get the hidden Windows taskbar's place, so a few may
  put their pop-ups slightly too high.

## The looks
- Windows 98 Second Edition, and the first 1995 release of Windows 95.
- In 16 and 256 Colors a few pictures are still converted by rule instead of drawn as the Windows
  drew them, and Help's pages at 256 colors are not yet dotted as the browser drew them.
- Other languages of Windows than English. Names in Latin, Greek and Cyrillic letters show as
  they are; Chinese, Japanese, Korean, Hebrew, Arabic, Thai and right-to-left text still show
  as "?".

## Folder windows and Explorer
- Thumbnails, Choose Columns, Customize this Folder, the other Explorer bars (Search, Favorites,
  History), Find, Send To, Format, Rename in the tree, dragging folders in the tree.
- The web view's picture preview; Control Panel, Printers, the Recycle Bin and network places
  open Windows' own windows.

## Programs
- More classic programs: Notepad, Paint and others. Today there are Minesweeper, Solitaire and
  the Calculator (with their Help books, whose words are written anew for Bevel95).
- Windows 95's small "Topics Found" window in Help's index.

## Platforms
- Windows 10 is reported to work by one user and has not been tried here; Windows 7 and 8.1 would
  need Rust's Windows 7 target and fallbacks for a few newer Windows calls.
- Linux: the core (every look, every rule) has no Windows in it by design, but the Windows layers
  would need counterparts there.
