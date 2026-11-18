//! Paint WinHelp paragraphs in a scrollable article window.

use crate::fb::{Framebuffer, Icon, Rect};
use crate::help::table;
use crate::help::{Catalog, Style, Topic};
use crate::text::{Font, Fonts};
use crate::theme::{Era, rgb};

use Style::*;

/// Wrap at whitespace, keeping byte offsets at character boundaries.
fn lines<'a>(s: &'a str, font: &Font, width: i32) -> impl Iterator<Item = &'a str> {
    let mut rest = s.trim();
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let mut end = 0;
        let mut used = 0;
        for chunk in rest.split_inclusive(char::is_whitespace) {
            let word = chunk.trim_end();
            if end > 0 && !word.is_empty() && used + font.measure(word) > width {
                break;
            }
            end += chunk.len();
            used += font.measure(chunk);
        }
        let (line, tail) = rest.split_at(end);
        rest = tail.trim_start();
        Some(line.trim_end())
    })
}

fn layout(
    fb: Option<&mut Framebuffer>,
    fonts: &Fonts,
    catalog: &Catalog,
    topic: &Topic,
    width: i32,
    scroll: i32,
    mut links: Option<&mut Vec<(Rect, usize)>>,
) -> i32 {
    let mut fb = fb;
    let ink = rgb(0x000000);
    let mut y = 8;
    for paragraph in &topic.paragraphs {
        let style = &paragraph.style;
        if let Table { header } = style {
            let x = 16;
            let table = table::Layout::new(&paragraph.text, *header, *fonts, width - x - 11, 8, 1, 5);
            if let Some(canvas) = fb.as_deref_mut() {
                for cell in table.cells {
                    let font = if cell.header { fonts.bold } else { fonts.regular };
                    let mut top = y + cell.bounds.y + 1 - scroll;
                    for line in cell.lines {
                        font.draw(canvas, x + cell.bounds.x + 1, top, &paragraph.text[line], ink);
                        top += font.height;
                    }
                }
            }
            y += table.height + 3;
            continue;
        }
        let x = match style {
            Title | Subhead | Note | Body | Link(_) | Related(_) => 16,
            NestedBullet => 38,
            _ => 28,
        };
        if matches!(style, Subhead | Note) {
            y += 5;
        }
        let font = if matches!(style, Title | Subhead | Note) { fonts.bold } else { fonts.regular };
        if let Some(canvas) = fb.as_deref_mut() {
            match style {
                Step(n) => {
                    font.draw(canvas, 16, y - scroll, &n.to_string(), ink);
                }
                Bullet | NestedBullet => {
                    let left = if matches!(style, NestedBullet) { 28 } else { 16 };
                    canvas.fill(Rect::new(left, y + 4 - scroll, 3, 3), ink);
                }
                _ => {}
            }
        }
        let mut write = |text: &str, target: Option<usize>| {
            for para_line in text.lines() {
                for line in lines(para_line, font, width - 11 - x) {
                    let line_width = font.measure(line);
                    if let Some(canvas) = fb.as_deref_mut() {
                        let color = if target.is_some() { rgb(0x008000) } else { ink };
                        font.draw(canvas, x, y - scroll, line, color);
                        if target.is_some() {
                            canvas.hline(x, y + font.height - 1 - scroll, line_width, color);
                        }
                    }
                    if let (Some(target), Some(links)) = (target, links.as_deref_mut()) {
                        links.push((Rect::new(x, y, line_width, 15), target));
                    }
                    y += 15;
                }
            }
        };
        let target = match style {
            Link(i) if *i < catalog.topics.len() => Some(*i),
            _ => None,
        };
        write(&paragraph.text, target);
        if let Related(targets) = style {
            for &target in targets {
                if let Some(topic) = catalog.topics.get(target) {
                    write(&topic.title, Some(target));
                }
            }
        }
        y += match style {
            Title => 7,
            Subhead | Note => 5,
            _ => 3,
        };
    }
    y + 12
}

pub(super) fn measure(fonts: &Fonts, book: &Catalog, topic: &Topic, width: i32, links: &mut Vec<(Rect, usize)>) -> i32 {
    links.clear();
    layout(None, fonts, book, topic, width, 0, Some(links))
}

pub(super) fn paint(
    fb: &mut Framebuffer,
    fonts: &Fonts,
    era: Era,
    book: &Catalog,
    topic: &Topic,
    scroll: i32,
    viewport: Rect,
) {
    let paper = if era == Era::Win95 { rgb(0xFFFFE2) } else { rgb(0xFFFFFF) };
    let mut canvas = Framebuffer::new(viewport.w, viewport.h, paper);
    layout(Some(&mut canvas), fonts, book, topic, viewport.w, scroll, None);
    fb.blit(&canvas, viewport.x, viewport.y);
}

/// The WinHelp document icon in the title bar of the topic window.
pub(super) fn icon() -> Icon {
    const { crate::icons::ClassicIcon::art("winhelp/document", crate::art::Look::Win95) }.icon()
}
