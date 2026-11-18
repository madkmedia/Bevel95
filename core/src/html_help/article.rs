//! Reflow and paint the topic pane of the HTML Help viewer. A link's hit rectangles
//! are the same line fragments that are drawn, measured in unscrolled page coordinates.

use crate::fb::{Framebuffer, Rect};
use crate::help::table::{self, lines};
use crate::help::{Style, Topic};
use crate::text::{Font, Fonts};
use crate::theme::{Era, rgb};
use std::ops::Range;

use super::fonts as article_fonts;
#[path = "support.rs"]
mod support;
#[path = "welcome.rs"]
mod welcome;

/// The HTML Help toolbar has its own support page, separate from the book supplied
/// by the application. Its advice also works while the machine is offline.
pub(super) fn web_topics(era: Era, first: usize) -> Vec<Topic> {
    support::topics(era, first)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Face {
    Body,
    Bold,
    Heading,
    WelcomeHeading,
    WebHeading,
    Bullet,
}

impl Face {
    fn font(self) -> &'static Font {
        match self {
            Self::Body | Self::Bullet => article_fonts::body(),
            Self::Bold => article_fonts::bold(),
            Self::Heading => article_fonts::heading(),
            Self::WelcomeHeading => article_fonts::welcome_heading(),
            Self::WebHeading => article_fonts::web_heading(),
        }
    }
}

#[derive(Clone, Debug)]
struct Line {
    range: Range<usize>,
    face: Face,
    bounds: Rect,
    link: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ArticleLayout {
    pub height: i32,
    text: String,
    lines: Vec<Line>,
    welcome: bool,
    support_directory: bool,
    link_color: u32,
    highlight_color: u32,
    link_targets: Vec<Option<usize>>,
    decorations: Vec<(Decoration, i32, i32)>,
}

#[derive(Clone, Copy, Debug)]
enum Decoration {
    SupportBullet,
    Note,
    TableCell { width: i32, height: i32 },
}

impl Decoration {
    fn paint(self, fb: &mut Framebuffer, x: i32, y: i32) {
        match self {
            Self::SupportBullet => fb.fill(Rect::new(x, y, 8, 9), rgb(0xff3300)),
            Self::TableCell { width, height } => fb.fill(Rect::new(x, y, width, height), rgb(0xeeeeee)),
            Self::Note => {
                // The small note-and-pencil image that precedes a Windows 2000 note.
                const NOTE: &crate::art::Rows = crate::art::rows("html-help/note", crate::art::Look::Win2000);
                for (dy, row) in NOTE.rows.iter().enumerate() {
                    for (dx, index) in row.bytes().enumerate() {
                        let color = NOTE.palette.iter().find(|(c, _)| *c == index).map_or(0, |&(_, hex)| hex);
                        fb.set(x + dx as i32, y + dy as i32, rgb(color));
                    }
                }
            }
        }
    }
}

#[derive(Default)]
pub(super) struct PaintState<'a> {
    pub hover: Option<usize>,
    pub focus: Option<usize>,
    pub active: Option<usize>,
    pub visited: &'a [usize],
    pub selection: Option<Range<usize>>,
    pub highlights: &'a [Range<usize>],
}

impl ArticleLayout {
    pub(super) fn link_at(&self, x: i32, y: i32) -> Option<usize> {
        self.lines.iter().find_map(|line| line.link.filter(|_| line.bounds.contains(x, y)))
    }

    pub(super) fn link_rect(&self, paragraph: usize) -> Option<Rect> {
        self.lines.iter().find(|line| line.link == Some(paragraph)).map(|line| line.bounds)
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    /// The nearest caret position in reading order. All positions handed to the
    /// caller are UTF-8 byte boundaries, including both ends of the article.
    pub(super) fn position_at(&self, x: i32, y: i32) -> usize {
        let gap = |p: i32, lo: i32, hi: i32| (lo - p).max(p - hi).max(0);
        let Some(line) = self.lines.iter().min_by_key(|line| {
            let r = line.bounds;
            (gap(y, r.y, r.bottom() - 1), gap(x, r.x, r.right()))
        }) else {
            return 0;
        };
        if y < self.lines[0].bounds.y {
            return 0;
        }
        if self.lines.iter().all(|line| y >= line.bounds.bottom()) {
            return self.text.len();
        }
        let font = line.face.font();
        let mut pen = line.bounds.x;
        for (i, ch) in self.text[line.range.clone()].char_indices() {
            let advance = font.glyph(ch).advance;
            if x < pen + (advance + 1) / 2 {
                return line.range.start + i;
            }
            pen += advance;
        }
        line.range.end
    }

    pub(super) fn matches(&self, query: &str, match_case: bool, whole_word: bool) -> Vec<Range<usize>> {
        if query.is_empty() {
            return Vec::new();
        }
        let word_char = |c: char| c.is_alphanumeric() || c == '_';
        let folded = query.to_lowercase();
        let mut found = Vec::new();
        let mut previous_end = 0;
        for (start, _) in self.text.char_indices() {
            if start < previous_end {
                continue;
            }
            let rest = &self.text[start..];
            let len = if match_case {
                if !rest.starts_with(query) {
                    continue;
                }
                query.len()
            } else {
                // Compare Unicode lowercase characters while retaining the byte offsets
                // of the original text (lowercasing may change a character's byte length).
                let mut needle = folded.chars();
                let mut len = 0;
                let mut matched = true;
                for c in rest.chars() {
                    for lower in c.to_lowercase() {
                        if needle.next() != Some(lower) {
                            matched = false;
                            break;
                        }
                    }
                    len += c.len_utf8();
                    if !matched || needle.as_str().is_empty() {
                        break;
                    }
                }
                if !matched || !needle.as_str().is_empty() {
                    continue;
                }
                len
            };
            let end = start + len;
            if whole_word
                && (self.text[..start].chars().next_back().is_some_and(word_char)
                    || self.text[end..].chars().next().is_some_and(word_char))
            {
                continue;
            }
            found.push(start..end);
            previous_end = end;
        }
        found
    }

    fn fragment(&self, line: &Line, range: &Range<usize>) -> Option<(Rect, &str)> {
        let start = range.start.max(line.range.start);
        let end = range.end.min(line.range.end);
        if start >= end {
            return None;
        }
        let prefix = self.text.get(line.range.start..start)?;
        let fragment = self.text.get(start..end)?;
        let font = line.face.font();
        let x = line.bounds.x + font.measure(prefix);
        Some((Rect::new(x, line.bounds.y, font.measure(fragment), line.bounds.h), fragment))
    }

    pub(super) fn range_rect(&self, range: &Range<usize>) -> Option<Rect> {
        self.lines.iter().find_map(|line| self.fragment(line, range).map(|(rect, _)| rect))
    }

    pub(super) fn paint(&self, fb: &mut Framebuffer, viewport: Rect, scroll: i32, state: &PaintState<'_>) {
        if viewport.w <= 0 || viewport.h <= 0 {
            return;
        }
        // Render onto the pane itself to clip partial glyphs at every edge, including
        // the top of the first and the bottom of the last visible line after scrolling.
        let mut pane = Framebuffer::new(viewport.w, viewport.h, rgb(0xffffff));
        if self.welcome {
            welcome::paint(&mut pane, scroll);
        }
        if self.support_directory {
            support::paint_heading(&mut pane, scroll);
        }
        for &(decoration, x, y) in &self.decorations {
            decoration.paint(&mut pane, x, y - scroll);
        }
        for line in &self.lines {
            let r = line.bounds;
            let y = r.y - scroll;
            if y >= viewport.h || y + r.h <= 0 {
                continue;
            }
            let color = match line.link {
                Some(id)
                    if self.link_color == rgb(0x0000cc)
                        && (state.active == Some(id)
                            || self.link_targets[id].is_some_and(|target| state.visited.contains(&target))) =>
                {
                    rgb(0x996699)
                }
                Some(id)
                    if self.link_color == rgb(0x0000ff) && (state.hover == Some(id) || state.active == Some(id)) =>
                {
                    rgb(0xff0000)
                }
                Some(_) => self.link_color,
                None => rgb(0),
            };
            let draw = |pane: &mut Framebuffer, x, text: &str, ink| {
                if line.face == Face::Bullet {
                    pane.hline(x + 1, y + 5, 3, ink);
                    pane.fill(Rect::new(x, y + 6, 5, 3), ink);
                    pane.hline(x + 1, y + 9, 3, ink);
                } else {
                    line.face.font().draw(pane, x, y, text, ink);
                }
            };
            draw(&mut pane, r.x, &self.text[line.range.clone()], color);
            if let Some(link) = line.link {
                if !self.support_directory || state.hover == Some(link) || state.active == Some(link) {
                    pane.hline(r.x, y + r.h - 1, r.w, color);
                }
                if line.link == state.focus {
                    crate::dialog::focus_dots(&mut pane, Rect::new(r.x - 1, y - 1, r.w + 2, r.h + 2), (0, 0), rgb(0));
                }
            }
            let mut highlight = |range: &Range<usize>, paper, ink| {
                if let Some((mark, text)) = self.fragment(line, range) {
                    pane.fill(Rect::new(mark.x, y, mark.w, mark.h), paper);
                    draw(&mut pane, mark.x, text, ink);
                    if line.link.is_some() {
                        pane.hline(mark.x, y + mark.h - 1, mark.w, ink);
                    }
                }
            };
            for range in state.highlights {
                highlight(range, self.highlight_color, rgb(0xffffff));
            }
            if let Some(range) = &state.selection {
                highlight(range, self.highlight_color, rgb(0xffffff));
            }
        }
        fb.blit(&pane, viewport.x, viewport.y);
    }

    fn line(&mut self, range: Range<usize>, face: Face, x: i32, y: i32, link: Option<usize>) {
        let font = face.font();
        let bounds = Rect::new(x, y, font.measure(&self.text[range.clone()]), font.height);
        self.lines.push(Line { range, face, bounds, link });
    }
}

pub(super) fn layout(era: Era, topic: &Topic, width: i32) -> ArticleLayout {
    layout_topic(era, topic, width, false)
}

pub(super) fn layout_web(era: Era, topic: &Topic, width: i32) -> ArticleLayout {
    layout_topic(era, topic, width, era == Era::Win2000)
}

fn layout_topic(era: Era, topic: &Topic, width: i32, support_directory: bool) -> ArticleLayout {
    let welcome = era == Era::Win98 && topic.welcome;
    let margin = if welcome { 35 } else { 10 };
    let mut page = ArticleLayout {
        welcome,
        support_directory,
        link_color: rgb(if support_directory {
            0x003366
        } else if era == Era::Win98 {
            0x0000cc
        } else {
            0x0000ff
        }),
        highlight_color: rgb(if era == Era::Win2000 { 0x0a246a } else { 0x000080 }),
        link_targets: topic
            .paragraphs
            .iter()
            .map(|p| match p.style {
                Style::Link(target) => Some(target),
                _ => None,
            })
            .collect(),
        ..ArticleLayout::default()
    };
    let mut y = if welcome {
        if topic.paragraphs.first().is_some_and(|p| p.style == Style::Subhead) { 80 } else { 93 }
    } else if support_directory {
        83
    } else {
        15
    };
    for (index, paragraph) in topic.paragraphs.iter().enumerate() {
        if index > 0 {
            page.text.push_str("\n\n");
        }
        if let Style::Table { header } = paragraph.style {
            let start = page.text.len();
            page.text.push_str(&paragraph.text);
            // The HTML tables have 1px cell padding and 2px between cells and at the edges.
            let fonts = Fonts { regular: article_fonts::body(), bold: article_fonts::bold() };
            let table = table::Layout::new(&paragraph.text, header, fonts, width - 2 * (margin + 2), 2, 1, 2);
            // The 98 keyboard page leaves 25px from the paragraph's last line box
            // to the first shaded cell (the paragraph already supplied 7px).
            y += if era == Era::Win98 { 18 } else { 2 };
            for cell in table.cells {
                let x = margin + 2 + cell.bounds.x;
                let top = y + cell.bounds.y;
                page.decorations.push((Decoration::TableCell { width: cell.bounds.w, height: cell.bounds.h }, x, top));
                let face = if cell.header { Face::Bold } else { Face::Body };
                for (i, line) in cell.lines.into_iter().enumerate() {
                    page.line(
                        start + line.start..start + line.end,
                        face,
                        x + 1,
                        top + 1 + i as i32 * face.font().height,
                        None,
                    );
                }
            }
            y += table.height + 7;
            continue;
        }
        let face = match paragraph.style {
            Style::Subhead if welcome && index == 0 => Face::WebHeading,
            Style::Title if welcome => Face::WelcomeHeading,
            Style::Title if era == Era::Win2000 => Face::Heading,
            Style::Title | Style::Subhead | Style::Note => Face::Bold,
            Style::Link(_) if support_directory => Face::Bold,
            _ => Face::Body,
        };
        let mut left = if support_directory { 80 } else { margin };
        let mut right = if support_directory { 5 } else { margin };
        let mut space_after = 7;
        match paragraph.style {
            Style::Step(n) => {
                left += if era == Era::Win2000 { 24 } else { 22 };
                if era == Era::Win2000 {
                    right += 24;
                }
                let number = format!("{n}.");
                let marker_x = left - 12 - face.font().measure(&number);
                let start = page.text.len();
                page.text.push_str(&number);
                page.line(start..page.text.len(), face, marker_x, y, None);
                page.text.push(' ');
                space_after = 8;
            }
            Style::Bullet | Style::NestedBullet => {
                let indent = if matches!(paragraph.style, Style::NestedBullet) { 34 } else { 17 };
                left += indent;
                if era == Era::Win2000 {
                    right += indent;
                }
                let start = page.text.len();
                page.text.push('•');
                page.line(start..page.text.len(), Face::Bullet, left - 15, y, None);
                page.text.push(' ');
                space_after = 8;
            }
            Style::Continuation => left += 24,
            Style::Note if era == Era::Win2000 => {
                y += 3;
                page.decorations.push((Decoration::Note, left, y + 1));
                left += 14;
                space_after = 4;
            }
            Style::Link(_) if welcome => {
                // The first HTML Help welcome page puts its footer six blank lines below the list.
                y += 6 * article_fonts::body().height + 3;
            }
            _ => {}
        }
        let link = matches!(paragraph.style, Style::Link(_) | Style::Related(_)).then_some(index);
        let start = page.text.len();
        for (i, block) in paragraph.text.split('\n').enumerate() {
            if i > 0 {
                page.text.push('\n');
            }
            for (i, word) in block.split_whitespace().enumerate() {
                if i > 0 {
                    page.text.push(' ');
                }
                page.text.push_str(word);
            }
        }
        let wrapped = lines(&page.text[start..], face.font(), (width - left - right).max(1));
        let mut line_y = y;
        if support_directory {
            // Directory table cells have a 15px minimum height and 6px row gap.
            // Vertically center the label when it occupies a single line.
            page.decorations.push((Decoration::SupportBullet, left - 15, y));
            let lines_height = wrapped.len() as i32 * face.font().height;
            let row_height = lines_height.max(15);
            line_y += (row_height - lines_height) / 2 - 2;
            y += row_height + 6;
        }
        for line in wrapped {
            page.line(start + line.start..start + line.end, face, left, line_y, link);
            line_y += face.font().height;
        }
        if !support_directory {
            y = line_y + space_after;
        }
    }
    page.height = y + if welcome { 74 } else { 3 };
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_reflow_without_losing_cell_positions_or_copy_text() {
        let text = "Button\tKey\nBack\tBACKSPACE\nOr\t|(pipe)";
        let topic = Topic::from_paragraphs("Keys", &[(Style::Table { header: true }, text)]);
        let wide = layout(Era::Win98, &topic, 280);
        let narrow = layout(Era::Win98, &topic, 85);
        assert_eq!(wide.text(), topic.plain_text());
        assert_eq!(narrow.text(), wide.text());
        assert!(narrow.height > wide.height);
        let rect = |s: &str| wide.range_rect(&wide.matches(s, true, true)[0]).unwrap();
        assert_eq!(rect("Back").x, rect("Button").x);
        assert_eq!(rect("Key").x, rect("BACKSPACE").x);
        assert_eq!(rect("Back").y, rect("BACKSPACE").y);
        let key = narrow.matches("BACKSPACE", true, true).pop().unwrap();
        let r = narrow.range_rect(&key).unwrap();
        assert_eq!(narrow.position_at(r.x, r.y), key.start);
    }

    #[test]
    fn visited_and_active_links_use_the_era_ink() {
        let topic = Topic::from_paragraphs("Home", &[(Style::Link(4), "Open the guide")]);
        let viewport = Rect::new(0, 0, 110, 40);
        for (era, state, ink) in [
            (Era::Win98, PaintState::default(), 0x0000cc),
            (Era::Win98, PaintState { visited: &[4], ..PaintState::default() }, 0x996699),
            (Era::Win98, PaintState { active: Some(0), ..PaintState::default() }, 0x996699),
            (Era::Win2000, PaintState { visited: &[4], ..PaintState::default() }, 0x0000ff),
            (Era::Win2000, PaintState { hover: Some(0), ..PaintState::default() }, 0xff0000),
        ] {
            let layout = layout(era, &topic, viewport.w);
            let rect = layout.link_rect(0).unwrap();
            let mut fb = Framebuffer::new(viewport.w, viewport.h, 0);
            layout.paint(&mut fb, viewport, 0, &state);
            assert_eq!(fb.get(rect.x, rect.bottom() - 1), rgb(ink));
        }
    }

    #[test]
    fn web_help_links_reflow_and_open_local_guidance() {
        let first = 12;
        let pages = web_topics(Era::Win2000, first);
        let directory = &pages[0];
        let page = layout_web(Era::Win2000, directory, 268);
        for (index, link) in directory.paragraphs.iter().enumerate() {
            let Style::Link(target) = link.style else { panic!("missing support link") };
            let rect = page.link_rect(index).unwrap();
            assert_eq!(page.link_at(rect.x, rect.y), Some(index));
            let guidance = &pages[target - first];
            assert_eq!(guidance.title, link.text);
            assert_eq!(guidance.paragraphs.last().unwrap().style, Style::Link(first));
        }
        let wide = layout_web(Era::Win2000, directory, 450);
        assert!(wide.height < page.height);
        assert_eq!(web_topics(Era::Win98, first).len(), 1);
    }

    #[test]
    fn find_and_select_use_the_printable_text_across_wrapped_lines() {
        let topic = Topic::from_paragraphs(
            "Guide",
            &[
                (Style::Title, "Guide"),
                (Style::Bullet, "Use CAFÉ, then caféine."),
                (Style::Link(0), "Return to the Guide"),
            ],
        );
        let page = layout(Era::Win2000, &topic, 91);
        assert_eq!(page.text(), topic.plain_text());
        let found = page.matches("café", false, true);
        assert_eq!(found.len(), 1);
        assert_eq!(&page.text()[found[0].clone()], "CAFÉ");
        assert!(page.matches("café", true, true).is_empty());
        assert_eq!(page.matches("café", false, false).len(), 2);
        let r = page.range_rect(&found[0]).unwrap();
        assert_eq!(page.position_at(r.x, r.y), found[0].start);
        assert_eq!(page.position_at(r.right(), r.y), found[0].end);
        assert_eq!(page.position_at(1, -10), 0);
        assert_eq!(page.position_at(1, page.height), page.text().len());

        let fragments: Vec<_> = page.lines.iter().filter(|line| line.link == Some(2)).collect();
        assert!(fragments.len() > 1);
        assert_eq!(page.link_rect(2), Some(fragments[0].bounds));
        for line in fragments {
            assert_eq!(page.link_at(line.bounds.x, line.bounds.y), Some(2));
            assert_eq!(page.link_at(line.bounds.right(), line.bounds.y), None);
        }
    }
}
