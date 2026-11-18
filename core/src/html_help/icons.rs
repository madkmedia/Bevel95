//! The HTML Help viewer's small icon, Contents images and toolbar pictures.
//! The color and monochrome toolbar strips are resources 513 and 512 in each
//! edition's HHCTRL.OCX. Their cells measure 18×16 in 98 and 20×20 in 2000.
//! The etched 2000 history arrows were measured on the initially empty history.
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/html_help.rs`); this
//! module chooses and draws them.

use crate::art::Look;
use crate::fb::Icon;
use crate::theme::{Era, rgb};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolIcon {
    Hide,
    Show,
    Back,
    Forward,
    Options,
    WebHelp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeIcon {
    Topic,
    OpenBook,
    ClosedBook,
    Overview,
}

// Rows run across all cells of a strip in the order of the corresponding enum.
fn cell(rows: &[&str], width: usize, index: usize) -> Icon {
    let left = width * index;
    let pixels = rows
        .iter()
        .flat_map(|row| {
            row.as_bytes()[left..left + width].iter().map(|&value| match value {
                b'.' => 0,
                b'K' => rgb(0x000000),
                b'm' => rgb(0x800000),
                b'g' => rgb(0x008000),
                b'o' => rgb(0x808000),
                b'n' => rgb(0x000080),
                b'p' => rgb(0x800080),
                b't' => rgb(0x008080),
                b'L' => rgb(0xC0C0C0),
                b'S' => rgb(0x808080),
                b'R' => rgb(0xFF0000),
                b'G' => rgb(0x00FF00),
                b'Y' => rgb(0xFFFF00),
                b'B' => rgb(0x0000FF),
                b'P' => rgb(0xFF00FF),
                b'C' => rgb(0x00FFFF),
                b'W' => rgb(0xFFFFFF),
                _ => unreachable!("unknown HTML Help palette entry"),
            })
        })
        .collect();
    Icon { width: width as i32, height: rows.len() as i32, pixels }
}

pub fn caption(_era: Era) -> Icon {
    cell(CAPTION, 16, 0)
}

pub fn tree(_era: Era, node: TreeIcon) -> Icon {
    cell(TREE, 16, node as usize)
}

pub fn toolbar(era: Era, tool: ToolIcon, enabled: bool, hot: bool) -> Icon {
    if era == Era::Win2000 {
        if !enabled {
            match tool {
                ToolIcon::Back => return cell(DISABLED_ARROWS, 20, 0),
                ToolIcon::Forward => return cell(DISABLED_ARROWS, 20, 1),
                _ => {}
            }
        }
        cell(if enabled && hot { HOT_2000 } else { NORMAL_2000 }, 20, tool as usize)
    } else {
        // Windows 98 shows its ordinary arrows even on the first page.
        cell(if enabled && hot { HOT_98 } else { NORMAL_98 }, 18, tool as usize)
    }
}

const CAPTION: &[&str] = crate::art::rows("html-help/caption", Look::Win98).rows;

const TREE: &[&str] = crate::art::rows("html-help/tree", Look::Win98).rows;

const NORMAL_98: &[&str] = crate::art::rows("html-help/normal", Look::Win98).rows;

const HOT_98: &[&str] = crate::art::rows("html-help/hot", Look::Win98).rows;

const NORMAL_2000: &[&str] = crate::art::rows("html-help/normal", Look::Win2000).rows;

const HOT_2000: &[&str] = crate::art::rows("html-help/hot", Look::Win2000).rows;

const DISABLED_ARROWS: &[&str] = crate::art::rows("html-help/disabled-arrows", Look::Win2000).rows;
