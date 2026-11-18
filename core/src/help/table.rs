//! Text tables shared by WinHelp and HTML Help. Text ranges point into the original
//! paragraph, so selection, search and copying keep the book's reading order.

use std::ops::Range;

use crate::fb::Rect;
use crate::text::{Font, Fonts};

/// Wrap at word boundaries, splitting a word at glyph boundaries only if necessary.
pub(crate) fn lines(text: &str, font: &Font, width: i32) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut offset = 0;
    for paragraph in text.split('\n') {
        let mut start = 0;
        loop {
            let mut used = 0;
            let mut last_break = None;
            let mut end = paragraph.len();
            for (i, ch) in paragraph[start..].char_indices() {
                let advance = font.glyph(ch).advance;
                if i > 0 && used + advance > width {
                    end = if ch == ' ' { start + i } else { last_break.unwrap_or(start + i) };
                    break;
                }
                used += advance;
                if matches!(ch, ' ' | '-') {
                    last_break = Some(start + i + 1);
                }
            }
            let visible = paragraph[start..end].trim_end_matches(' ');
            out.push(offset + start..offset + start + visible.len());
            if end == paragraph.len() {
                break;
            }
            start = end + paragraph[end..].starts_with(' ') as usize;
        }
        offset += paragraph.len() + 1;
    }
    out
}

pub(crate) struct Cell {
    pub bounds: Rect,
    pub header: bool,
    pub lines: Vec<Range<usize>>,
}

pub(crate) struct Layout {
    pub cells: Vec<Cell>,
    pub height: i32,
}

#[derive(Default)]
struct Column {
    glyph: i32,
    word: i32,
    line: i32,
}

impl Layout {
    /// Prefer the width of each cell's text. If space is tight, preserve whole words
    /// first, then allow the widest words to break. A short key beside a long
    /// description therefore uses only the space needed for the key.
    pub(crate) fn new(
        text: &str,
        header: bool,
        fonts: Fonts,
        width: i32,
        gap: i32,
        padding: i32,
        row_gap: i32,
    ) -> Self {
        let mut columns: Vec<Column> = Vec::new();
        for (row, line) in text.split('\n').enumerate() {
            let font = if header && row == 0 { fonts.bold } else { fonts.regular };
            for (col, cell) in line.split('\t').enumerate() {
                if col == columns.len() {
                    columns.push(Column::default());
                }
                let column = &mut columns[col];
                column.glyph = column.glyph.max(cell.chars().map(|ch| font.glyph(ch).advance).max().unwrap_or(0));
                column.word = column
                    .word
                    .max(column.glyph)
                    .max(cell.split_whitespace().map(|word| font.measure(word)).max().unwrap_or(0));
                column.line = column.line.max(font.measure(cell));
            }
        }
        let count = columns.len() as i32;
        // For a very narrow pane, stack the cells in reading order instead of letting
        // columns narrower than a glyph collide. Resizing the pane restores the columns.
        let room = i64::from(width - count * 2 * padding - (count - 1) * gap);
        let glyph_total: i64 = columns.iter().map(|c| i64::from(c.glyph)).sum();
        let word_total: i64 = columns.iter().map(|c| i64::from(c.word)).sum();
        let line_total: i64 = columns.iter().map(|c| i64::from(c.line)).sum();
        let stacked = room < glyph_total;
        let widths: Vec<i32> = columns
            .iter()
            .map(|col| {
                let content = if stacked {
                    width - 2 * padding
                } else if room >= line_total {
                    col.line
                } else if room >= word_total {
                    col.word + ((room - word_total) * i64::from(col.line - col.word) / (line_total - word_total)) as i32
                } else {
                    col.glyph
                        + ((room - glyph_total) * i64::from(col.word - col.glyph) / (word_total - glyph_total)) as i32
                };
                content.max(0) + 2 * padding
            })
            .collect();
        let mut cells = Vec::new();
        let mut offset = 0;
        let mut y = 0;
        for (row, line) in text.split('\n').enumerate() {
            let heading = header && row == 0;
            let font = if heading { fonts.bold } else { fonts.regular };
            let first = cells.len();
            let mut x = 0;
            let mut tallest = 0;
            for (col, cell) in line.split('\t').enumerate() {
                let width = widths[col];
                let lines: Vec<_> = lines(cell, font, width - 2 * padding)
                    .into_iter()
                    .map(|r| offset + r.start..offset + r.end)
                    .collect();
                let height = lines.len() as i32 * font.height + 2 * padding;
                cells.push(Cell { bounds: Rect::new(x, y, width, height), header: heading, lines });
                offset += cell.len() + 1;
                if stacked {
                    y += height + row_gap;
                } else {
                    tallest = tallest.max(height);
                    x += width + gap;
                }
            }
            if !stacked {
                for cell in &mut cells[first..] {
                    cell.bounds.h = tallest;
                }
                y += tallest + row_gap;
            }
        }
        Self { cells, height: y - row_gap }
    }
}
