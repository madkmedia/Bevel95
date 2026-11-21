//! The three looks, as plain numbers. Each value notes where it came from; the full table,
//! and the places where the sources disagree, is in `docs/theme-spec.md`.
//!
//! Sources:
//! - RetroBar, github.com/dremin/RetroBar @ d7e1469 (`Themes/System.xaml`,
//!   `Themes/Windows 95-98.xaml`, `Themes/Windows 2000.xaml`, `Controls/TaskList.xaml.cs`).
//! - 98.css, github.com/jdan/98.css @ b1d7a90 (`style.css`), used as a cross-check.
//! - "WPF": RetroBar's text settings rendered with WPF on this PC, to see where its
//!   labels land (RetroBar positions text through WPF font metrics, not pixel numbers).
//! - "v86": real Windows 95 and 98 run in the v86 emulator (copy.sh/v86), their screen read
//!   pixel for pixel: the window menu, scroll arrows, shortcut texts, the check mark.
//! - PROVISIONAL: neither source has it (RetroBar opens the OS Start menu; 98.css has no
//!   menus). Check against a real screenshot before trusting.

use crate::banner::{Banner, WIN95_BANNER, WIN98_BANNER, WIN2000_BANNER};
pub use crate::colors16::Depth;

/// An opaque framebuffer pixel from a `0xRRGGBB` color.
pub const fn rgb(hex: u32) -> u32 {
    0xFF00_0000 | (hex & 0x00FF_FFFF)
}

/// Which look to draw. A config setting; the default is `Win95`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Era {
    #[default]
    Win95,
    /// Windows 98 as first released (98 Second Edition differs: see docs/theme-spec.md).
    Win98,
    Win2000,
}

impl Era {
    /// Every look, in the order the Theme menu lists them.
    pub const ALL: [Era; 3] = [Era::Win95, Era::Win98, Era::Win2000];

    /// The name in settings.txt.
    pub const fn setting(self) -> &'static str {
        match self {
            Era::Win95 => "win95",
            Era::Win98 => "win98",
            Era::Win2000 => "win2000",
        }
    }

    /// The name in the Theme menu.
    pub const fn title(self) -> &'static str {
        match self {
            Era::Win95 => "Windows 95",
            Era::Win98 => "Windows 98",
            Era::Win2000 => "Windows 2000",
        }
    }

    /// Windows 2000 hides access-key underlines until the keyboard is used; 95 and 98 always
    /// show them.
    pub const fn always_shows_keys(self) -> bool {
        !matches!(self, Era::Win2000)
    }
}

/// The theme for an era, in True Color.
pub fn theme(era: Era) -> &'static Theme {
    theme_in(era, Depth::TrueColor)
}

/// The theme for an era on a screen of `depth` colors (`colors16`).
pub fn theme_in(era: Era, depth: Depth) -> &'static Theme {
    match (era, depth) {
        (Era::Win95, Depth::TrueColor) => &WIN95,
        (Era::Win98, Depth::TrueColor) => &WIN98,
        (Era::Win2000, Depth::TrueColor) => &WIN2000,
        (Era::Win95, Depth::Sixteen) => &WIN95_16,
        (Era::Win98, Depth::Sixteen) => &WIN98_16,
        (Era::Win2000, Depth::Sixteen) => &WIN2000_16,
        (Era::Win95, Depth::TwoFiftySix) => &WIN95_256,
        (Era::Win98, Depth::TwoFiftySix) => &WIN98_256,
        (Era::Win2000, Depth::TwoFiftySix) => &WIN2000_256,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// Short name used in file names, e.g. "win95" (the same in 16 colors).
    pub slug: &'static str,
    /// The look this theme is (what a painter that holds only the theme needs to know).
    pub era: Era,
    /// True Color or 16 colors: a painter that draws something a 16-color screen showed
    /// differently asks this (`colors16`).
    pub depth: Depth,
    /// UI font. Rendered with GDI, non-antialiased, into a 1× glyph atlas.
    pub font_face: &'static str,
    /// Font size in pixels per em (8pt at 96 DPI = 11px). RetroBar `GlobalFontSize` = 11.
    pub font_px: i32,
    pub colors: Colors,
    pub bar: BarMetrics,
    pub start: StartMetrics,
    pub task: TaskMetrics,
    pub tray: TrayMetrics,
    pub text: TextMetrics,
    pub start_menu: StartMenuMetrics,
    pub popup: PopupMetrics,
}

/// Colors. The names in backticks are RetroBar's resource keys.
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    /// `ButtonFace`: the bar and button surface.
    pub face: u32,
    /// `ButtonLight`: inner top/left line of a raised button; top row of the bar.
    pub light: u32,
    /// `ButtonHighlight`: outer top/left line of a raised button; second row of the bar.
    pub highlight: u32,
    /// `ButtonInnerHighlight`: the line across the top inside an active (pressed) task button.
    pub inner_highlight: u32,
    /// `ButtonShadow`: inner bottom/right line of a raised button.
    pub shadow: u32,
    /// `ButtonDarkShadow`: outer bottom/right line of a raised button.
    pub dark_shadow: u32,
    /// `ButtonForeground`: button and menu text.
    pub text: u32,
    /// `ButtonFlashingFace`: face of a flashing task button. RetroBar maps it to the system
    /// Highlight color, so it doubles as the menu selection color.
    pub flash_face: u32,
    /// `ButtonFlashingForeground`: text on a flashing button (= system HighlightText).
    pub flash_text: u32,
    /// `ClockForeground`.
    pub clock_text: u32,
    /// Selected menu item background (= `flash_face`, see above).
    pub menu_highlight: u32,
    /// Selected menu item text (= `flash_text`).
    pub menu_highlight_text: u32,
    /// Tooltips (system colors Info Background and Info Text; their 1px border is the Window
    /// Frame color). The same in every look. Screenshots S10 and S13.
    pub tip_face: u32,
    pub tip_text: u32,
    pub tip_border: u32,
    /// What a grayed-out label is etched with: a copy 1px down and right under its gray. White
    /// in True Color; on 95's and 98's 16-color screens the face color, so the label is plain gray
    /// there (the Reference PCs: Taskbar Properties' grayed Apply; the emulator's grayed menu entries).
    pub etch: u32,
    /// The desktop as each Windows first showed it (its system color Desktop): teal in
    /// Windows 95 and 98, blue in Windows 2000. The reference PCs' True Color captures.
    pub desktop: u32,
}

/// The bar itself. The buttons and the clock well all share one row.
#[derive(Clone, Copy, Debug)]
pub struct BarMetrics {
    /// Window height. RetroBar `TaskbarHeight` = 28.
    pub height: i32,
    /// Top edge rows: 1px `ButtonLight` then 1px `ButtonHighlight`. RetroBar `Taskbar`
    /// template (two nested borders, `BorderThickness="0,1,0,0"`).
    pub edge: i32,
    /// Space above and below the row of buttons. RetroBar StartButton `Padding="2,2,0,2"`,
    /// TaskButton `Padding="0,2"`, Tray `Margin="0,2"`.
    pub pad_y: i32,
}

impl BarMetrics {
    /// Top of the Start button, task buttons and clock well.
    pub const fn item_y(&self) -> i32 {
        self.edge + self.pad_y
    }
    /// Height of the Start button, task buttons and clock well (22 for a 28px bar).
    pub const fn item_h(&self) -> i32 {
        self.height - self.edge - 2 * self.pad_y
    }
}

/// The Start button. Its width follows from its contents: 2px bevel + `pad_x` + icon +
/// `icon_gap` + label + `label_margin_right` + `pad_x` + 2px bevel.
#[derive(Clone, Copy, Debug)]
pub struct StartMetrics {
    /// Gap from the left screen edge. RetroBar StartButton `Padding="2,2,0,2"`.
    pub margin_left: i32,
    /// Padding inside the bevel. RetroBar `ButtonInnerTopBorder Padding="2,1"`.
    pub pad_x: i32,
    pub pad_y: i32,
    /// Flag size. RetroBar `TaskIcon` 16×16 (StartIcon is based on it).
    pub icon: i32,
    /// Gap between flag and label. RetroBar StartIcon `Margin="0,0,2,0"`.
    pub icon_gap: i32,
    /// RetroBar TaskLabel `Margin="0,-1,1,0"`; the 2000 theme overrides StartLabel to
    /// `Margin="0,-1,0,0"`.
    pub label_margin_right: i32,
    /// The label, drawn in the bold font. RetroBar `start_text` (English) + StartLabel
    /// `FontWeight="Bold"`.
    pub label: &'static str,
    /// Content offset while pressed. RetroBar StartButton `Content Margin="1,1,-1,-1"`.
    pub pressed_shift: (i32, i32),
    /// While the Start menu is open, Windows 95 draws a dotted focus rectangle 3px inside the
    /// pressed button; Windows 2000 does not (GUIdebook screenshots).
    pub focus_rect: bool,
}

/// Task buttons (one per app group).
#[derive(Clone, Copy, Debug)]
pub struct TaskMetrics {
    /// Gap between the Start button and the first task button. RetroBar
    /// TaskListScrollViewer `Margin="4,0,0,0"`.
    pub gap_after_start: i32,
    /// Button width while there is room. RetroBar `TaskButtonWidth` = 160.
    pub width: i32,
    /// Narrowest a button gets before the list needs scrolling: just the icon. Windows 98
    /// in the v86 emulator (RetroBar's `TaskButtonMinWidth` is 45).
    pub min_width: i32,
    /// Width of the scroll arrows that appear when the buttons do not fit (Windows 98 in
    /// the v86 emulator: an up-down control 16 wide, the row's height split in two).
    pub scroll_w: i32,
    /// Gap after each button. RetroBar `TaskButtonMargin Right="3"`.
    pub spacing: i32,
    /// Padding inside the bevel. RetroBar `ButtonInnerTopBorder Padding="2,1"`.
    pub pad_x: i32,
    pub pad_y: i32,
    /// RetroBar `TaskIcon` 16×16.
    pub icon: i32,
    /// RetroBar TaskIcon `Margin="0,0,3,0"`.
    pub icon_gap: i32,
    /// RetroBar TaskLabel `Margin="0,-1,1,0"`: where a title starts to be cut short with
    /// "...", from the right edge of the padding (negative: into it).
    pub label_margin_right: i32,
    /// Extra rows above the content of an active button: the 1px `ButtonInnerHighlight`
    /// line. RetroBar TaskButtonActive (`BorderThickness="0,1,0,0"`, `Padding="2,1,2,0"`),
    /// which moves the content down 1px and not sideways.
    pub active_top_line: i32,
}

/// The sunken well on the right (the notification area): the programs' little icons, then
/// the clock.
#[derive(Clone, Copy, Debug)]
pub struct TrayMetrics {
    /// Gap between the well and its neighbours (left: last task slot; right: screen edge).
    /// RetroBar 95-98 Tray `InnerTrayBorder Padding="2,0"` (base theme: `Padding="2"`).
    pub margin_x: i32,
    /// Padding inside the 1px sunken border. RetroBar Tray inner `Border Padding="2"`.
    pub pad: i32,
    /// The clock's part of the well does not change width as the time does: it is as wide as
    /// the widest time of day plus `clock_extra`, and the time is centered in it (rounded to
    /// the left). Reference PCs and screenshots S2, S11, S14 (RetroBar instead puts fixed
    /// margins around the current time, 8 and 8, or 13 and 10).
    pub clock_extra: i32,
    /// Where the clock's part starts, from the inside of the padding, when there are no icons.
    pub clock_x: i32,
    /// Notification icons (16×16, centered vertically in the well): the first one's left edge
    /// from the inside of the padding, the step from one icon to the next, and the space from
    /// the last step to the clock's part.
    pub icon_x: i32,
    pub icon_step: i32,
    pub icons_gap: i32,
}

/// Where text sits vertically: the baseline row, counted from the top of the 16px content
/// box (the area inside bevel and padding that also holds the 16px icon). Text is centered
/// in the button, as Windows' DrawText(DT_VCENTER) does, which puts every label on the same
/// baseline as the clock. For Tahoma (2000) that is also exactly where RetroBar's labels land
/// (measured with WPF). For the 95 bitmap font it is 1px lower than RetroBar: RetroBar shifts
/// its labels up 1px to suit its TrueType stand-in, whose capitals are 1px shorter than the
/// bitmap font's 9-row ones (see docs/theme-spec.md, "Text").
#[derive(Clone, Copy, Debug)]
pub struct TextMetrics {
    /// Start label and task labels.
    pub label_baseline: i32,
    /// Clock text.
    pub clock_baseline: i32,
}

/// Menus with icons: the Start menu (32px icons, banner on the left) and the menus that
/// cascade from it (16px icons), which the window list of a task button also uses.
/// Measured on GUIdebook's Windows 95 and 2000 screenshots (docs/theme-spec.md, "Menus").
#[derive(Clone, Copy, Debug)]
pub struct StartMenuMetrics {
    /// Frame: 2px raised edge plus 1px of face color.
    pub frame: i32,
    /// Width of the vertical banner on the left of the Start menu.
    pub banner_w: i32,
    pub banner: &'static Banner,
    /// Items of the Start menu itself.
    pub large: IconItems,
    /// Items of the menus that cascade from it.
    pub small: IconItems,
    /// Items of the Start menu itself with "Show small icons in Start menu" on (no banner),
    /// and where its submenu arrows sit.
    pub start_small: IconItems,
    pub start_small_arrow: Arrow,
    /// Labels are centered in their item; when the rows left over are odd, Windows 95 puts
    /// the extra row below the text, Windows 2000 above it.
    pub text_round_up: bool,
    pub arrow: Arrow,
    pub sep: Separator,
    /// Extra face-colored rows above the first separator of the Start menu. Windows 2000
    /// leaves one between the Windows Update band and the rest.
    pub first_sep_extra: i32,
    pub submenu: SubmenuPlacement,
    /// Longer labels (long window titles in a window list) are cut off with "...".
    pub max_label_w: i32,
    /// Face-colored rows under the last entry, inside the frame (Windows 98: 1).
    pub bottom_pad: i32,
    /// Colors Windows 98 fills each icon's square with before drawing the icon (not
    /// selected, selected): one step off the menu's face and selection colors, BFBFBF and
    /// 00007F (the reference PC, and GUIdebook's full-color screenshot). None: the menu's own.
    pub icon_cell: Option<(u32, u32)>,
    /// Windows 95 sized its cascading menus by the labels' raw text, the access key's "&"
    /// counted as a letter, so its Settings and Find submenus are one "&" (6 pixels) wider
    /// than their labels need (the reference PC). Its Programs menus have no access keys.
    pub cascade_counts_ampersands: bool,
}

/// The items of one kind of icon menu.
#[derive(Clone, Copy, Debug)]
pub struct IconItems {
    pub h: i32,
    /// Rows at the top and at the bottom of an item that the selection color leaves out
    /// (Windows 95's cascading menus: 1, so selections have a gap between them).
    pub highlight_inset: i32,
    pub icon: i32,
    /// Icon's left edge, from the item's left edge.
    pub icon_x: i32,
    /// Gap between the icon and the label.
    pub text_gap: i32,
    /// Space after the longest label; the submenu arrow sits in it.
    pub pad_right: i32,
    /// The same in a menu without submenus: Windows 98 leaves out the arrow's room there.
    pub pad_right_no_arrows: i32,
}

/// Ordinary menus, as Windows itself draws them: the bar's right-click menu and its Size
/// submenu. Windows 95 measured on Toasty Tech's screenshots (taskbar and desktop
/// right-click menus); the Windows 2000 values are PROVISIONAL (no screenshot found).
#[derive(Clone, Copy, Debug)]
pub struct PopupMetrics {
    /// Frame: 2px raised edge plus 1px of face color.
    pub frame: i32,
    pub item_h: i32,
    /// Top of the text cell, from the top of the item.
    pub text_y: i32,
    /// Label left edge, from the left of the item (the column for a check mark).
    pub text_x: i32,
    /// The 7×7 check mark's left edge, from the left of the item.
    pub check_x: i32,
    /// Space after the longest label; the submenu arrow sits in it.
    pub text_pad_right: i32,
    /// Shortcut texts ("Alt+F4") start this far after the longest label...
    pub shortcut_gap: i32,
    /// ...and this much space follows the widest of them, measured in the regular font even
    /// when the default entry draws it bold.
    pub shortcut_pad_right: i32,
    /// Menus with little pictures (the window menu of Windows 98 and 2000: Restore, Minimize,
    /// Maximize, Close) start their labels here instead of at `text_x`, but leave room as if
    /// the labels came `glyph_reserve` after `text_x`.
    pub glyph_text_x: i32,
    pub glyph_reserve: i32,
    /// Grayed-out labels and pictures are etched (a white copy 1px down-right under the gray),
    /// as all three draw them in True Color (the reference PCs; with 16 or 256 colors Windows
    /// 95 and 98 drew them plain gray). On the selection they are plain gray in every look.
    pub etched_disabled: bool,
    /// Windows 2000: a grayed-out entry's access key is underlined with every other pixel
    /// (Calculator's Edit > Paste on the reference PC).
    pub dotted_disabled_keys: bool,
    pub arrow: Arrow,
    pub sep: Separator,
    pub submenu: SubmenuPlacement,
}

/// The submenu arrow: a 4×7 triangle.
#[derive(Clone, Copy, Debug)]
pub struct Arrow {
    /// Columns between the arrow and the right edge of the item.
    pub right: i32,
    /// The arrow is centered as if it were this tall: its top is (item height - `box_h`) / 2
    /// rows below the top of the item.
    pub box_h: i32,
}

/// A separator: an etched line (shadow, then highlight below it).
#[derive(Clone, Copy, Debug)]
pub struct Separator {
    pub h: i32,
    /// Row of the shadow line, from the top of the separator.
    pub line_y: i32,
    /// Columns the lines leave free at both ends.
    pub inset: i32,
}

/// Where a submenu opens beside the entry that opened it: to the right, its first item level
/// with that entry, overlapping the parent menu by `overlap` (both 3px frames).
#[derive(Clone, Copy, Debug)]
pub struct SubmenuPlacement {
    pub overlap: i32,
    /// When it does not fit below: Windows 95 opens it upward (its bottom level with the
    /// entry's bottom) if that fits; Windows 2000's Start menu moves it up just enough.
    pub flip_up: bool,
    /// Rows kept free at the bottom of the screen when it has to move up (2000: 1).
    pub margin_bottom: i32,
}
/// Windows 95 (RetroBar's "Windows 95-98" theme).
pub const WIN95: Theme = Theme {
    slug: "win95",
    era: Era::Win95,
    depth: Depth::TrueColor,
    font_face: "MS Sans Serif",
    font_px: 11,
    colors: Colors {
        face: rgb(0xC0C0C0),
        light: rgb(0xDFDFDF),
        highlight: rgb(0xFFFFFF),
        inner_highlight: rgb(0xFFFFFF),
        shadow: rgb(0x808080),
        dark_shadow: rgb(0x000000),
        text: rgb(0x000000),
        flash_face: rgb(0x000080),
        flash_text: rgb(0xFFFFFF),
        clock_text: rgb(0x000000),
        menu_highlight: rgb(0x000080),
        menu_highlight_text: rgb(0xFFFFFF),
        tip_face: rgb(0xFFFFE1),
        tip_text: rgb(0x000000),
        tip_border: rgb(0x000000),
        etch: rgb(0xFFFFFF),
        desktop: rgb(0x008080),
    },
    bar: BarMetrics { height: 28, edge: 2, pad_y: 2 },
    start: StartMetrics {
        margin_left: 2,
        pad_x: 2,
        pad_y: 1,
        icon: 16,
        icon_gap: 2,
        label_margin_right: 1,
        label: "Start",
        pressed_shift: (1, 1),
        focus_rect: true,
    },
    task: TaskMetrics {
        gap_after_start: 4,
        width: 160,
        min_width: 22,
        scroll_w: 16,
        spacing: 3,
        pad_x: 2,
        pad_y: 1,
        icon: 16,
        icon_gap: 2,
        label_margin_right: 1,
        active_top_line: 1,
    },
    // Windows 98 on the reference PC with no, one and two icons, at three times of day, and
    // screenshots S2 (95, no icons) and S14 (98, one icon): icons 17 apart from the inside of
    // the padding, the clock's part 2 after the last (1 in without icons) and 46 + 10 wide.
    tray: TrayMetrics { margin_x: 2, pad: 2, clock_extra: 10, clock_x: 1, icon_x: 0, icon_step: 17, icons_gap: 2 },
    text: TextMetrics { label_baseline: 12, clock_baseline: 13 },
    start_menu: StartMenuMetrics {
        frame: 3,
        banner_w: 21,
        banner: &WIN95_BANNER,
        large: IconItems {
            h: 32,
            highlight_inset: 0,
            icon: 32,
            icon_x: 6,
            text_gap: 6,
            pad_right: 31,
            pad_right_no_arrows: 31,
        },
        // A menu without submenus is just as wide (Multimedia in GUIdebook's screenshot).
        small: IconItems {
            h: 22,
            highlight_inset: 1,
            icon: 16,
            icon_x: 6,
            text_gap: 6,
            pad_right: 25,
            pad_right_no_arrows: 25,
        },
        // PROVISIONAL: no screenshot of Windows 95's own Start menu with small icons (the
        // emulator's has Internet Explorer 4's); laid out like its cascading menus.
        start_small: IconItems {
            h: 22,
            highlight_inset: 1,
            icon: 16,
            icon_x: 6,
            text_gap: 6,
            pad_right: 25,
            pad_right_no_arrows: 25,
        },
        start_small_arrow: Arrow { right: 6, box_h: 9 },
        text_round_up: false,
        arrow: Arrow { right: 6, box_h: 9 },
        sep: Separator { h: 9, line_y: 2, inset: 0 },
        first_sep_extra: 0,
        submenu: SubmenuPlacement { overlap: 6, flip_up: true, margin_bottom: 0 },
        max_label_w: 300,
        bottom_pad: 0,
        icon_cell: None,
        cascade_counts_ampersands: true,
    },
    popup: PopupMetrics {
        frame: 3,
        item_h: 17,
        text_y: 1,
        text_x: 21,
        // Windows 98's Notepad "Word Wrap" in the v86 emulator; the real Minesweeper's Game
        // menu on the Windows 95 reference PC.
        check_x: 6,
        text_pad_right: 21,
        // Run in the v86 emulator: Windows 95's window menu and Windows 98's Notepad Edit menu.
        shortcut_gap: 8,
        shortcut_pad_right: 19,
        // Windows 98's window menu in v86 (Windows 95's has no pictures).
        glyph_text_x: 17,
        glyph_reserve: 17,
        // The reference PCs in True Color: Minesweeper's window menu on 95 (Restore, Size,
        // Maximize), Calculator's Edit > Paste on 98. (The emulator's 16-color 98 drew its
        // Notepad's Cut and Copy plain gray.)
        etched_disabled: true,
        dotted_disabled_keys: false,
        arrow: Arrow { right: 6, box_h: 9 },
        sep: Separator { h: 9, line_y: 3, inset: 0 },
        submenu: SubmenuPlacement { overlap: 6, flip_up: true, margin_bottom: 0 },
    },
};

/// Windows 98 (first edition): Windows 95's bar, menus and colors, with the new Start menu
/// that came with Internet Explorer 4. Measured on GUIdebook's Windows 98 screenshots.
pub const WIN98: Theme = Theme {
    slug: "win98",
    era: Era::Win98,
    // A title is cut short only where it would pass the bevel: "Drive Converter (FAT32)" fills
    // a 121px button in GUIdebook's screenshot right up to the shadow line.
    task: TaskMetrics { label_margin_right: -2, ..WIN95.task },
    start_menu: StartMenuMetrics {
        banner: &WIN98_BANNER,
        large: IconItems {
            h: 36,
            highlight_inset: 0,
            icon: 32,
            icon_x: 5,
            text_gap: 2,
            pad_right: 20,
            // PROVISIONAL: a Start menu always has submenus; same rule as the small items.
            pad_right_no_arrows: 3,
        },
        small: IconItems {
            h: 20,
            highlight_inset: 0,
            icon: 16,
            icon_x: 5,
            text_gap: 2,
            pad_right: 16,
            pad_right_no_arrows: 3,
        },
        // The Start menu with small icons in the v86 emulator: 136 wide with "Windows Update"
        // the longest label, rows of 20, the icon 2 in and the label 9 after it (unlike its
        // cascading menus), arrows 5 from the edge, no row under the last entry.
        start_small: IconItems {
            h: 20,
            highlight_inset: 0,
            icon: 16,
            icon_x: 2,
            text_gap: 9,
            pad_right: 21,
            pad_right_no_arrows: 21,
        },
        start_small_arrow: Arrow { right: 5, box_h: 7 },
        text_round_up: true,
        arrow: Arrow { right: 4, box_h: 7 },
        sep: Separator { h: 8, line_y: 3, inset: 2 },
        first_sep_extra: 0,
        // PROVISIONAL below the first row: no screenshot shows a menu that had to move up;
        // Windows 2000's rule (same Start menu code, Internet Explorer 4's) is assumed.
        submenu: SubmenuPlacement { overlap: 5, flip_up: false, margin_bottom: 0 },
        max_label_w: 300,
        bottom_pad: 1,
        icon_cell: Some((rgb(0xBFBFBF), rgb(0x00007F))),
        cascade_counts_ampersands: false,
        ..WIN95.start_menu
    },
    // Windows' own menus as 95's, but their separator lines stop 1 short of each end
    // (Calculator's window menu on the reference PC), as in Windows 2000.
    popup: PopupMetrics { sep: Separator { h: 9, line_y: 3, inset: 1 }, ..WIN95.popup },
    ..WIN95
};

/// Windows 2000 (RetroBar's "Windows 2000" theme, which builds on the base theme).
pub const WIN2000: Theme = Theme {
    slug: "win2000",
    era: Era::Win2000,
    font_face: "Tahoma",
    font_px: 11,
    colors: Colors {
        face: rgb(0xD4D0C8),
        light: rgb(0xD4D0C8),
        highlight: rgb(0xFFFFFF),
        inner_highlight: rgb(0xFFFFFF),
        shadow: rgb(0x808080),
        dark_shadow: rgb(0x404040),
        text: rgb(0x000000),
        flash_face: rgb(0x0A246A),
        flash_text: rgb(0xFFFFFF),
        clock_text: rgb(0x000000),
        menu_highlight: rgb(0x0A246A),
        menu_highlight_text: rgb(0xFFFFFF),
        tip_face: rgb(0xFFFFE1),
        tip_text: rgb(0x000000),
        tip_border: rgb(0x000000),
        etch: rgb(0xFFFFFF),
        desktop: rgb(0x3A6EA5),
    },
    start: StartMetrics { label_margin_right: 0, focus_rect: false, ..WIN95.start },
    // The reference PC with no icons and screenshot S11 with one: the clock's part 46 + 19
    // wide, flush with the padding without icons; the volume icon 1 inside the padding and
    // the clock's part 23 in. PROVISIONAL: the step between several icons (22, 0 after).
    tray: TrayMetrics { clock_extra: 19, clock_x: 0, icon_x: 1, icon_step: 22, icons_gap: 0, ..WIN95.tray },
    text: TextMetrics { label_baseline: 12, clock_baseline: 13 },
    start_menu: StartMenuMetrics {
        banner: &WIN2000_BANNER,
        // PROVISIONAL `pad_right_no_arrows`: no Windows 2000 screenshot shows a menu without
        // submenus; Windows 98 (same Start menu code) narrows those.
        large: IconItems {
            h: 32,
            highlight_inset: 0,
            icon: 32,
            icon_x: 2,
            text_gap: 9,
            pad_right: 25,
            pad_right_no_arrows: 25,
        },
        small: IconItems {
            h: 20,
            highlight_inset: 0,
            icon: 16,
            icon_x: 2,
            text_gap: 9,
            pad_right: 21,
            pad_right_no_arrows: 21,
        },
        // The Start menu with small icons in the v86 emulator: laid out exactly like these
        // cascading menus (and like Windows 98's Start menu with small icons).
        start_small: IconItems {
            h: 20,
            highlight_inset: 0,
            icon: 16,
            icon_x: 2,
            text_gap: 9,
            pad_right: 21,
            pad_right_no_arrows: 21,
        },
        start_small_arrow: Arrow { right: 5, box_h: 7 },
        text_round_up: true,
        arrow: Arrow { right: 5, box_h: 7 },
        sep: Separator { h: 8, line_y: 3, inset: 2 },
        first_sep_extra: 1,
        submenu: SubmenuPlacement { overlap: 6, flip_up: false, margin_bottom: 1 },
        cascade_counts_ampersands: false,
        ..WIN95.start_menu
    },
    // Windows' own menus, measured on the reference PC (its taskbar and speaker menus): labels
    // 17 in and 1 down, separators 1 short of each end, grayed-out labels etched; items, arrows
    // and separators otherwise as in 95. PROVISIONAL: the space after the labels (21, as in 95;
    // that Windows 2000's Tahoma is a pixel narrower or wider than today's in a few letters, so
    // its menus cannot be compared pixel for pixel) and the window menu's shortcut column (kept
    // where it was: 17 + 21).
    popup: PopupMetrics {
        text_y: 1,
        text_x: 17,
        // The real Minesweeper's Game menu on the reference PC: 2 further left than 95's.
        check_x: 4,
        glyph_reserve: 21,
        etched_disabled: true,
        // Its grayed-out entries underline their access key with every other pixel.
        dotted_disabled_keys: true,
        sep: Separator { h: 9, line_y: 3, inset: 1 },
        ..WIN95.popup
    },
    ..WIN95
};

// The three looks on a 16-color screen: the same sizes, the colors each Windows took there
// (the Reference PCs in 16 colors: private/probes/colors16/survey.txt, docs/theme-spec.md "16
// Colors"). A color outside the 16 that is kept here (the tooltip's) is an area that 95 and 98
// fill with a pattern of two of the 16 (`colors16::fill`).

/// Windows 95 in 16 colors: only the 3D light color DFDFDF is not one of the 16; it becomes
/// silver, so a raised edge's inner light line vanishes into the face.
/// Grayed-out entries of Windows' own menus are plain gray (not etched as in True Color): the
/// emulator's 16-color Windows 98 (its Notepad's Cut and Copy).
pub const WIN95_16: Theme = Theme {
    depth: Depth::Sixteen,
    colors: Colors { light: rgb(0xC0C0C0), etch: rgb(0xC0C0C0), ..WIN95.colors },
    popup: PopupMetrics { etched_disabled: false, ..WIN95.popup },
    ..WIN95
};

/// Windows 98 in 16 colors: 95's colors (the light edge silver) and grayed-out menu entries;
/// its Start menu's icon squares (BFBFBF, 00007F) become the menu's own silver and navy.
pub const WIN98_16: Theme = Theme {
    depth: Depth::Sixteen,
    colors: WIN95_16.colors,
    start_menu: StartMenuMetrics { icon_cell: None, ..WIN98.start_menu },
    popup: PopupMetrics { etched_disabled: false, ..WIN98.popup },
    ..WIN98
};

/// Windows 2000 in 16 colors: every color by the nearest of the 16, areas included (2000 never
/// dots one): the face and light D4D0C8 silver, the dark shadow 404040 black, the selection
/// 0A246A navy, the desktop 3A6EA5 teal, the tooltip FFFFE1 white.
pub const WIN2000_16: Theme = Theme {
    depth: Depth::Sixteen,
    colors: Colors {
        face: rgb(0xC0C0C0),
        light: rgb(0xC0C0C0),
        dark_shadow: rgb(0x000000),
        flash_face: rgb(0x000080),
        menu_highlight: rgb(0x000080),
        tip_face: rgb(0xFFFFFF),
        desktop: rgb(0x008080),
        ..WIN2000.colors
    },
    ..WIN2000
};

// The three looks on a 256-color screen (after the restart Windows asked for): their 16-color
// look in the shell, with what each showed richer there (the Reference PCs at 256 colors:
// private/probes/colors256/survey.md, docs/theme-spec.md "256 Colors").

/// Windows 95 in 256 colors: as its 16 Colors (the light edge silver), but a grayed label is
/// etched with white as in True Color (Taskbar Properties' grayed Apply and Run's grayed OK on its
/// PC at 256 colors; grayed menu entries were not seen there: PROVISIONAL, etched likewise).
pub const WIN95_256: Theme = Theme {
    depth: Depth::TwoFiftySix,
    colors: Colors { etch: rgb(0xFFFFFF), ..WIN95_16.colors },
    popup: WIN95.popup,
    ..WIN95_16
};

/// Windows 98 in 256 colors: as its 16 Colors (the light edge silver, the Start menu's icon squares
/// the menu's own), its grayed labels etched as 95's at 256 colors.
pub const WIN98_256: Theme = Theme {
    depth: Depth::TwoFiftySix,
    colors: Colors { etch: rgb(0xFFFFFF), ..WIN98_16.colors },
    popup: WIN98.popup,
    ..WIN98_16
};

/// Windows 2000 in 256 colors: its 16 Colors, but its own face and desktop stay (D4D0C8, 3A6EA5;
/// every window and the desktop of its PC at 256 colors), and its tooltip's pale yellow is an
/// area it dots white and yellow (`colors256::dotted`). The dark shadow is black and the selection
/// navy, as in 16 colors (Run, the message boxes, the menus there).
pub const WIN2000_256: Theme = Theme {
    depth: Depth::TwoFiftySix,
    colors: Colors {
        face: rgb(0xD4D0C8),
        light: rgb(0xD4D0C8),
        tip_face: rgb(0xFFFFE1),
        desktop: rgb(0x3A6EA5),
        ..WIN2000_16.colors
    },
    ..WIN2000_16
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_row_is_22px_at_y4() {
        for t in [&WIN95, &WIN98, &WIN2000] {
            assert_eq!((t.bar.item_y(), t.bar.item_h()), (4, 22), "{}", t.slug);
        }
    }

    #[test]
    fn every_era_has_its_theme_and_names() {
        for era in Era::ALL {
            assert_eq!(theme(era).slug, era.setting());
            assert!(era.title().starts_with("Windows "));
            for depth in Depth::ALL {
                let t = theme_in(era, depth);
                assert_eq!((t.era, t.depth, t.slug), (era, depth, era.setting()));
            }
        }
    }

    #[test]
    fn the_16_color_looks_use_only_the_16_but_for_areas() {
        use crate::colors16::index;
        for era in Era::ALL {
            let c = &theme_in(era, Depth::Sixteen).colors;
            let lines = [c.face, c.light, c.highlight, c.inner_highlight, c.shadow, c.dark_shadow, c.text];
            let more = [c.flash_face, c.flash_text, c.clock_text, c.menu_highlight, c.menu_highlight_text];
            for color in lines.into_iter().chain(more).chain([c.tip_text, c.tip_border, c.desktop]) {
                assert!(index(color).is_some(), "{era:?} {color:08X}");
            }
            // The tooltip is an area: 95 and 98 dot it, 2000 takes white.
            assert_eq!(index(c.tip_face).is_some(), era == Era::Win2000);
        }
        // Sizes are the same as in True Color.
        assert_eq!(WIN98_16.start_menu.large.h, WIN98.start_menu.large.h);
        assert_eq!(WIN98_256.start_menu.large.h, WIN98.start_menu.large.h);
        assert_eq!(WIN2000_256.font_face, "Tahoma");
        assert_eq!(WIN2000_16.font_face, "Tahoma");
    }

    #[test]
    fn each_look_has_its_first_desktop_color() {
        assert_eq!(WIN95.colors.desktop, rgb(0x008080));
        assert_eq!(WIN98.colors.desktop, WIN95.colors.desktop, "Windows 98 kept 95's teal");
        assert_eq!(WIN2000.colors.desktop, rgb(0x3A6EA5));
    }

    #[test]
    fn baselines_fit_in_the_16px_content_box() {
        for t in [&WIN95, &WIN98, &WIN2000] {
            assert!((1..16).contains(&t.text.label_baseline), "{}", t.slug);
            assert!((1..16).contains(&t.text.clock_baseline), "{}", t.slug);
        }
    }
}
