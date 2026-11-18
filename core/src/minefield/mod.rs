//! Minesweeper for the Windows 95, 98 and 2000 looks.
//!
//! `crate::window` paints the frame, title and menu bar. The board has its own pixel art
//! and geometry for each edition. A square occupies 16×16 pixels at native resolution.
//!
//! - `game`: the rules, without a window.
//! - `art`: the pictures, pixel by pixel.
//! - `board`: where everything goes inside the window, and painting it.
//! - `form`: the main window as a control kit form (mouse, keys, menu bar).
//! - `dialogs`: Custom Field, Best Times, the new-record name prompt and About Minesweeper.
//! - `scores`: the settings and best times as a text file.

pub mod art;
pub mod audio;
pub mod board;
pub mod dialogs;
pub mod form;
pub mod game;
pub mod html_help_catalog;
pub mod scores;
