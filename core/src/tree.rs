//! Windows' tree view: rows one step further in for each folder they are in, joined by dotted
//! lines, with a small box at each folder that shows a plus while its rows are put away and a
//! minus while they show. Drawn as the "Remove Shortcuts/Folders" window of Taskbar Properties
//! drew the Programs menu, read pixel for pixel on the Reference PCs (captures
//! `vm/*-truecolor-remove-tree-*.png`; docs/theme-spec.md, "Trees"). The three Windows agree
//! in every measure, and in how the tree begins: with an outermost row ("Start Menu") that
//! has neither lines nor a box. (A first reading of Windows 95's tree took its second row for
//! its first: that capture was scrolled by one row.)
//!
//! Bevel95 Settings' list of programs adds a check box to each row, between the lines and the
//! picture, where Windows' trees put theirs: Windows 98's and 2000's flat box of 13 (measured in
//! 98's Folder Options and 2000's Start Menu Settings), and for Windows 95, whose trees had no
//! boxes, the box of its one list that had them (Add/Remove Programs, Windows Setup).

use crate::dialog;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::paint::CHECK_ART;
use crate::programs::Tick;
use crate::text::Font;
use crate::theme::{Colors, Era, Theme, rgb};

/// A row's height.
pub const ROW_H: i32 = 16;
/// How much further in a row stands for each folder it is in (the tree view's "indent").
pub const STEP: i32 = 19;

/// A row's check box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    /// Windows 98's and 2000's: 13×13, a black frame of 2 around a white inside.
    Flat,
    /// Windows 95's (its Windows Setup list): 11×11, a gray frame of 1 around a white inside.
    Setup,
}

/// How a look draws its trees: the same in everything but the check boxes that Bevel95 adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeStyle {
    pub boxes: BoxKind,
}

impl TreeStyle {
    pub fn of(era: Era) -> TreeStyle {
        match era {
            Era::Win95 => TreeStyle { boxes: BoxKind::Setup },
            Era::Win98 | Era::Win2000 => TreeStyle { boxes: BoxKind::Flat },
        }
    }

    /// The middle of the plus/minus box of a row at `depth`, from the tree's left edge: under
    /// the picture of the folder it is in. `None` for an outermost row, which has no lines.
    pub fn button_x(self, depth: usize) -> Option<i32> {
        (depth >= 1).then(|| (depth as i32 - 1) * STEP + 8)
    }

    /// Where a row's content begins (its check box if it has one, else its picture).
    pub fn content_x(self, depth: usize) -> i32 {
        depth as i32 * STEP
    }
}

/// One row of a tree, as it is drawn.
#[derive(Clone, Debug)]
pub struct TreeRow<'a> {
    /// How many folders it is in.
    pub depth: usize,
    /// It has rows of its own: they show now (a minus in its box), or are put away (a plus).
    pub open: Option<bool>,
    /// For each folder it is in, from the outermost one, and last for the row itself: whether
    /// it is the last row of its level there (the line down that level ends at it).
    pub last: Vec<bool>,
    /// Its check box, if the tree has them.
    pub tick: Option<Tick>,
    pub icon: Option<&'a Icon>,
    pub label: &'a str,
    /// Selected: its label is highlighted, with the dotted focus rectangle around it, while
    /// the tree has the focus (`focused`). A tree that shows its selection without the focus
    /// too (that of Remove Shortcuts/Folders does) has the label on the window's gray then,
    /// in the usual text color and without the rectangle.
    pub selected: bool,
    pub focused: bool,
}

/// What of a row is at a place across it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// The plus/minus box.
    Button,
    /// The check box.
    Tick,
    /// The picture or the label.
    Item,
    /// Nothing: the lines, or the white right of the label.
    Blank,
}

/// The parts of a row from left to right: the check box (16 wide, if any), the picture (16),
/// then 3 further the label.
fn places(style: TreeStyle, depth: usize, boxes: bool) -> (i32, i32, i32) {
    let content = style.content_x(depth);
    let icon = content + if boxes { 16 } else { 0 };
    (content, icon, icon + 19)
}

/// What is at `x` (from the tree's left edge) in a row at `depth` whose label is `label_w` wide.
pub fn zone(style: TreeStyle, depth: usize, has_rows: bool, boxes: bool, label_w: i32, x: i32) -> Zone {
    let (content, icon, label) = places(style, depth, boxes);
    match style.button_x(depth) {
        Some(middle) if has_rows && (middle - 4..=middle + 4).contains(&x) => Zone::Button,
        _ if boxes && (content..icon).contains(&x) => Zone::Tick,
        _ if (icon..label + label_w + 4).contains(&x) => Zone::Item,
        _ => Zone::Blank,
    }
}

/// Where a row's label is drawn from, and how wide it may be in a tree `width` wide.
pub fn label_room(style: TreeStyle, depth: usize, boxes: bool, width: i32) -> (i32, i32) {
    let (_, _, label) = places(style, depth, boxes);
    (label + 2, width - label - 4)
}

/// The flat check box of Windows 98's and 2000's lists and trees: a frame of 2 in `ink` around
/// its `inside`, with Windows' check mark while `checked`.
pub(crate) fn flat_box(fb: &mut Framebuffer, x: i32, y: i32, checked: bool, ink: u32, inside: u32) {
    fb.fill(Rect::new(x, y, 13, 13), ink);
    fb.fill(Rect::new(x + 2, y + 2, 9, 9), inside);
    if checked {
        dialog::art(fb, CHECK_ART, x + 3, y + 3, |b| (b == b'#').then_some(ink));
    }
}

/// The check box of Windows 95's Windows Setup list: a gray frame of 1 around its `inside`.
fn setup_box(fb: &mut Framebuffer, c: &Colors, x: i32, y: i32, checked: bool, inside: u32) {
    fb.fill(Rect::new(x, y, 11, 11), c.shadow);
    fb.fill(Rect::new(x + 1, y + 1, 9, 9), inside);
    if checked {
        dialog::art(fb, CHECK_ART, x + 2, y + 2, |b| (b == b'#').then_some(c.text));
    }
}

/// A look's check box with its top-left corner at (x, y), showing `tick`. A folder with only
/// some of its programs ticked has its tick on gray: the shaded box of Windows Setup ("only
/// part of the component will be installed"), measured in Windows 95's and 98's.
pub fn paint_box(fb: &mut Framebuffer, c: &Colors, kind: BoxKind, x: i32, y: i32, tick: Tick) {
    let (checked, inside) = (tick != Tick::Off, if tick == Tick::Mixed { c.face } else { rgb(0xFFFFFF) });
    match kind {
        BoxKind::Flat => flat_box(fb, x, y, checked, c.text, inside),
        BoxKind::Setup => setup_box(fb, c, x, y, checked, inside),
    }
}

/// Paints one row with its top edge at `y` into a tree whose left edge is `fb`'s and whose
/// rows start at multiples of `ROW_H` (the dotted lines keep to the even rows). `width` is the
/// tree's; `screen` is where `fb`'s top-left corner is on the screen (the focus rectangle's
/// dots line up with the screen).
#[allow(clippy::too_many_arguments)]
pub fn paint_row(
    fb: &mut Framebuffer,
    theme: &Theme,
    font: &Font,
    style: TreeStyle,
    row: &TreeRow,
    y: i32,
    width: i32,
    screen: (i32, i32),
) {
    let c = &theme.colors;
    let (white, middle_y) = (rgb(0xFFFFFF), y + 8);
    // The lines: down each level that goes on below this row; at the row's own level from the
    // top to its middle, then across to its content.
    for level in 0..=row.depth {
        let Some(x) = style.button_x(level) else { continue };
        let ends = row.last.get(level).copied().unwrap_or(true);
        let own = level == row.depth;
        let bottom = if !ends {
            y + ROW_H
        } else if own {
            middle_y + 1
        } else {
            y
        };
        for dot in (y..bottom).filter(|dot| dot % 2 == 0) {
            fb.set(x, dot, c.shadow);
        }
        if own {
            for dot in (x..=x + 10).step_by(2) {
                fb.set(dot, middle_y, c.shadow);
            }
            if let Some(open) = row.open {
                fb.fill(Rect::new(x - 4, middle_y - 4, 9, 9), c.shadow);
                fb.fill(Rect::new(x - 3, middle_y - 3, 7, 7), white);
                fb.hline(x - 2, middle_y, 5, c.text);
                if !open {
                    fb.vline(x, middle_y - 2, 5, c.text);
                }
            }
        }
    }
    let (content, icon_x, _) = places(style, row.depth, row.tick.is_some());
    if let Some(tick) = row.tick {
        // In its place of 16: the box of 13 at 2 (measured: Windows 98's Folder Options, 2000's
        // Start Menu Settings), the box of 11 at 3 (ours: level with the plus and minus boxes).
        let at = if style.boxes == BoxKind::Flat { 2 } else { 3 };
        paint_box(fb, c, style.boxes, content + at, y + at, tick);
    }
    if let Some(icon) = row.icon {
        crate::colors16::draw_picture(fb, theme, icon, icon_x, y);
    }
    let (text_x, room) = label_room(style, row.depth, row.tick.is_some(), width);
    let label = font.elide(row.label, room);
    let mut color = c.text;
    if row.selected {
        let r = Rect::new(text_x - 2, y, font.measure(&label) + 4, ROW_H);
        if row.focused {
            fb.fill(r, c.menu_highlight);
            crate::datetime::item_focus(fb, r, screen, (white ^ c.menu_highlight) | rgb(0));
            color = c.menu_highlight_text;
        } else {
            fb.fill(r, c.face);
        }
    }
    font.draw(fb, text_x, y + 1, &label, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_look_steps_in_as_its_windows_did() {
        // In every look the outermost row has no box; a row one folder in has its box under
        // that row's picture, and its own content one step in.
        for era in Era::ALL {
            let style = TreeStyle::of(era);
            assert_eq!((style.button_x(0), style.content_x(0)), (None, 0));
            assert_eq!((style.button_x(1), style.content_x(1)), (Some(8), 19));
            assert_eq!((style.button_x(3), style.content_x(3)), (Some(46), 57));
        }
        // The looks differ in their check boxes only.
        assert_eq!(TreeStyle::of(Era::Win2000), TreeStyle::of(Era::Win98));
        assert_eq!((TreeStyle::of(Era::Win95).boxes, TreeStyle::of(Era::Win98).boxes), (BoxKind::Setup, BoxKind::Flat));
    }

    #[test]
    fn a_row_tells_its_parts_apart() {
        let style = TreeStyle::of(Era::Win98);
        // One folder in, with boxes: the plus/minus box at 4..=12, the check box's place from
        // 19, the picture from 35, the label (30 wide) from 54 to 88.
        let at = |x: i32| zone(style, 1, true, true, 30, x);
        assert_eq!([3, 4, 12, 13].map(at), [Zone::Blank, Zone::Button, Zone::Button, Zone::Blank]);
        assert_eq!(
            [18, 19, 34, 35, 87, 88].map(at),
            [Zone::Blank, Zone::Tick, Zone::Tick, Zone::Item, Zone::Item, Zone::Blank]
        );
        assert_eq!(zone(style, 1, false, true, 30, 8), Zone::Blank, "no rows of its own: no box to press");
        assert_eq!(zone(style, 0, true, true, 30, 8), Zone::Tick, "the outermost row begins with its check box");
        // Without check boxes the picture comes right after the lines, as in Windows' window.
        assert_eq!(zone(style, 1, true, false, 30, 20), Zone::Item);
        assert_eq!(label_room(style, 1, true, 252), (56, 194));
        assert_eq!(label_room(style, 0, false, 252).0, 21, "Windows' outermost label: 2 after the picture's 19");
    }
}
