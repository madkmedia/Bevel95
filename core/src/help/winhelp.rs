//! Windows 95 WinHelp's topic picker and article window.

use crate::art::Look;
use crate::dialog::{self, TitleButtons};
use crate::fb::{Framebuffer, Icon, Rect};
use crate::help::Catalog;
use crate::help::EntryIcon;
use crate::kit::{self, Effect, Form, Key, Mods, Press};
use crate::properties;
use crate::text::Fonts;
use crate::theme::{Era, Theme, rgb};
use crate::window::{self, ButtonLook, Frame, TitleButton};

#[path = "winhelp_article.rs"]
mod help_article;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpTab {
    Contents,
    Index,
    Find,
}

impl HelpTab {
    const ALL: [Self; 3] = [Self::Contents, Self::Index, Self::Find];
    fn label(self) -> &'static str {
        match self {
            Self::Contents => "Contents",
            Self::Index => "Index",
            Self::Find => "Find",
        }
    }
    fn rect(self) -> Rect {
        match self {
            Self::Contents => Rect::new(11, 31, 54, 18),
            Self::Index => Rect::new(65, 31, 42, 18),
            Self::Find => Rect::new(107, 31, 42, 18),
        }
    }
    fn step(self, delta: i32) -> Self {
        Self::ALL[(self as i32 + delta).rem_euclid(3) as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Close,
    Cancel,
    Context,
    Tab(HelpTab),
    Query,
    List,
    Display,
    Print,
    Contents,
    Back,
    Options,
    Minimize,
    Maximize,
    ScrollUp,
    ScrollDown,
    Track,
    Thumb,
    Link(usize),
}

#[derive(Clone)]
pub struct HelpTopics {
    era: Era,
    catalog: Catalog,
    picker_title: String,
    tab: HelpTab,
    selected: Option<usize>,
    query: String,
    caret: usize,
    /// The selection's other end (bytes; equal to `caret`: nothing selected), and the query field
    /// itself: Windows' edit box (`edit::Edit`, brief 99), `query`/`caret`/`anchor` its copy.
    anchor: usize,
    edit: crate::edit::Edit,
    copied: Option<String>,
    blink: bool,
    article: Option<usize>,
    history: Vec<usize>,
    scroll: i32,
    max_scroll: i32,
    focus: Part,
    pressed: Option<Part>,
    show_keys: bool,
    options: bool,
    context: bool,
    tip: Option<(Part, i32, i32)>,
    active: bool,
    auto_width: i32,
    max_height: i32,
    auto_height: i32,
    manual_size: Option<(i32, i32)>,
    maximized_size: Option<(i32, i32)>,
    links: Vec<(Rect, usize)>,
    first_row: usize,
    rows: Vec<usize>,
    collapsed: Vec<usize>,
    query_positions: Vec<(usize, i32)>,
}

const INDEX_SIZE: (i32, i32) = (434, 428);
const MIN_ARTICLE: (i32, i32) = (200, 100);
const EDIT: Rect = Rect::new(39, 92, 359, 20);
const LIST: Rect = Rect::new(27, 98, 378, 275);
const SEARCH_LIST: Rect = Rect::new(39, 140, 359, 225);

const BOOK: &[&str] = crate::art::rows("winhelp/book", Look::Win95).rows;
const OPEN_BOOK: &[&str] = crate::art::rows("winhelp/open-book", Look::Win95).rows;
const PAGE: &[&str] = crate::art::rows("winhelp/page", Look::Win95).rows;

fn contents_icon(fb: &mut Framebuffer, rows: &[&str], x: i32, y: i32) {
    dialog::art(fb, rows, x, y, |b| {
        (b != b'.').then(|| {
            rgb(match b {
                b'P' => 0x800080,
                b'S' => 0x808080,
                b'Y' => 0xffff00,
                b'K' => 0,
                b'B' => 0x0000ff,
                b'G' => 0xc0c0c0,
                _ => 0xffffff,
            })
        })
    });
}

impl HelpTopics {
    pub fn new(era: Era, catalog: Catalog, tab: HelpTab) -> Self {
        let collapsed = catalog
            .contents
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| (entry.icon == EntryIcon::Book && entry.topic.is_none()).then_some(i))
            .collect();
        let mut help = Self {
            era,
            picker_title: format!("Help Topics: {}", catalog.title),
            catalog,
            tab,
            selected: None,
            query: String::new(),
            caret: 0,
            anchor: 0,
            edit: crate::edit::Edit::default().with_rule(Some(crate::edit::HELP_QUERY_LIMIT), crate::edit::Allow::Any),
            copied: None,
            blink: true,
            article: None,
            history: Vec::new(),
            scroll: 0,
            max_scroll: 0,
            focus: Part::List,
            pressed: None,
            show_keys: era.always_shows_keys(),
            options: false,
            context: false,
            active: true,
            // The headless default width is the books' 360/1024 of an 800px screen.
            // A window host supplies the actual width and space below the opening position.
            auto_width: 281,
            max_height: i32::MAX,
            auto_height: MIN_ARTICLE.1,
            manual_size: None,
            maximized_size: None,
            links: Vec::new(),
            first_row: 0,
            rows: Vec::new(),
            collapsed,
            query_positions: Vec::new(),
            tip: None,
        };
        help.choose_tab(tab);
        help
    }

    pub fn article(era: Era, catalog: Catalog, topic: usize) -> Option<Self> {
        catalog.topics.get(topic)?;
        let mut help = Self::new(era, catalog, HelpTab::Contents);
        help.article = Some(topic);
        help.focus = Part::Contents;
        Some(help)
    }

    /// The query field takes in the query and the selection as they are now (brief 99).
    fn pull(&mut self) {
        if self.edit.text != self.query {
            self.edit.set(&self.query);
        }
        let chars = |b: usize| self.query[..b.min(self.query.len())].chars().count();
        self.edit.caret = chars(self.caret);
        self.edit.anchor = chars(self.anchor);
    }

    /// What the query field holds is shown; a changed query finds afresh.
    fn push(&mut self) {
        let changed = self.query != self.edit.text;
        self.query = self.edit.text.clone();
        let bytes = |c: usize| self.query.char_indices().nth(c).map_or(self.query.len(), |(i, _)| i);
        self.caret = bytes(self.edit.caret);
        self.anchor = bytes(self.edit.anchor);
        self.blink = true;
        if changed {
            self.edited();
        }
    }

    /// The byte offset of the query nearest `x` (the window's pixels).
    fn query_at(&self, x: i32) -> usize {
        self.query_positions.iter().min_by_key(|(_, at)| (x - *at).abs()).map_or(self.caret, |&(i, _)| i)
    }

    fn choose_tab(&mut self, tab: HelpTab) {
        self.tab = tab;
        self.query.clear();
        self.caret = 0;
        self.anchor = 0;
        self.rows = self.results();
        self.selected = if tab == HelpTab::Index { None } else { self.rows.first().copied() };
        self.first_row = 0;
        self.focus = if tab == HelpTab::Contents { Part::List } else { Part::Query };
        self.context = false;
    }

    // A row identifies a contents entry, an index keyword, or a matching topic. Several
    // different keywords or contents entries may link to the same topic.
    fn label(&self, row: usize) -> &str {
        match self.tab {
            HelpTab::Contents => &self.catalog.contents[row].label,
            HelpTab::Index => &self.catalog.index[row].label,
            HelpTab::Find => &self.catalog.topics[row].title,
        }
    }
    fn row_topic(&self, row: usize) -> Option<usize> {
        match self.tab {
            HelpTab::Contents => self.catalog.contents.get(row)?.topic,
            HelpTab::Index => Some(self.catalog.index.get(row)?.topic),
            HelpTab::Find => Some(row),
        }
        .filter(|&i| i < self.catalog.topics.len())
    }
    fn results(&self) -> Vec<usize> {
        match self.tab {
            HelpTab::Contents => {
                let mut hide_children = None;
                self.catalog
                    .contents
                    .iter()
                    .enumerate()
                    .filter_map(|(i, entry)| {
                        if hide_children.is_some_and(|depth| entry.depth > depth) {
                            return None;
                        }
                        hide_children = self.collapsed.contains(&i).then_some(entry.depth);
                        Some(i)
                    })
                    .collect()
            }
            HelpTab::Index => {
                // A keyword registered for several topics is one line, which opens its first
                // topic (WinHelp's "Topics Found" chooser is not drawn yet: private/todo.md).
                let mut rows: Vec<_> = (0..self.catalog.index.len()).collect();
                rows.sort_by_cached_key(|&i| self.label(i).to_lowercase());
                rows.dedup_by(|a, b| self.label(*a).eq_ignore_ascii_case(self.label(*b)));
                rows
            }
            HelpTab::Find => {
                let query = self.query.to_lowercase();
                self.catalog
                    .topics
                    .iter()
                    .enumerate()
                    .filter_map(|(i, topic)| {
                        (query.is_empty()
                            || topic.title.to_lowercase().contains(&query)
                            || topic.paragraphs.iter().any(|p| p.text.to_lowercase().contains(&query)))
                        .then_some(i)
                    })
                    .collect()
            }
        }
    }
    fn edited(&mut self) {
        if self.tab == HelpTab::Find {
            self.rows = self.results();
        }
        self.selected = self.rows.iter().copied().find(|&i| {
            self.tab != HelpTab::Index || self.label(i).to_lowercase().starts_with(&self.query.to_lowercase())
        });
        self.first_row = 0;
        self.reveal_selection();
        self.blink = true;
    }
    fn click_row(&self, x: i32, y: i32) -> Option<usize> {
        let r = self.rect(Part::List)?.inset(2, 2, 2, 2);
        if !r.contains(x, y) {
            return None;
        }
        let row = (y - r.y) / if self.tab == HelpTab::Contents { 18 } else { 13 };
        self.rows.get(self.first_row + row as usize).copied()
    }
    pub fn is_article(&self) -> bool {
        self.article.is_some()
    }
    pub fn icon() -> Icon {
        help_article::icon()
    }
    pub fn is_maximized(&self) -> bool {
        self.maximized_size.is_some()
    }
    /// Outer width and available height of an automatically sized article, in 1× pixels.
    pub fn set_article_bounds(&mut self, width: i32, max_height: i32) {
        self.auto_width = width.max(MIN_ARTICLE.0);
        self.max_height = max_height.max(MIN_ARTICLE.1);
    }
    pub fn set_article_size(&mut self, size: Option<(i32, i32)>) {
        self.maximized_size = size.map(|(w, h)| (w.max(MIN_ARTICLE.0), h.max(MIN_ARTICLE.1)));
    }
    pub fn set_window_size(&mut self, size: (i32, i32), maximized: bool) {
        if !self.is_article() {
            return;
        }
        if maximized {
            self.set_article_size(Some(size));
        } else if self.maximized_size.take().is_none() && self.size() != size {
            // Creating or programmatically resizing a window also sends WM_SIZE.
            // Only an actual user resize replaces the article's automatic dimensions.
            self.manual_size = Some((size.0.max(MIN_ARTICLE.0), size.1.max(MIN_ARTICLE.1)));
        }
    }
    fn toolbar_button(width: i32, index: i32) -> Rect {
        let per_row = ((width - 12) / 65).max(1);
        Rect::new(6 + index % per_row * 65, 25 + index / per_row * 21, 65, 21)
    }
    fn viewport(&self) -> Rect {
        let (w, h) = self.size();
        let y = Self::toolbar_button(w, 2).bottom();
        Rect::new(6, y, w - 12, h - y - 6)
    }
    pub fn print_text(&self) -> String {
        if self.article.is_none()
            && let Some(book) = self.selected_book()
        {
            // WinHelp prints every topic in a book chosen in Contents.
            return self
                .book_topics(book)
                .into_iter()
                .filter_map(|i| self.catalog.topics.get(i))
                .map(|t| t.plain_text())
                .collect::<Vec<_>>()
                .join("\n\n");
        }
        self.article
            .or_else(|| self.selected.and_then(|row| self.row_topic(row)))
            .and_then(|i| self.catalog.topics.get(i))
            .map_or_else(String::new, |topic| topic.plain_text())
    }

    /// The Contents book chosen (an entry with no topic of its own), if one is.
    fn selected_book(&self) -> Option<usize> {
        let row = self.selected.filter(|_| self.tab == HelpTab::Contents)?;
        (self.catalog.contents.get(row)?.topic.is_none()).then_some(row)
    }

    /// The topics of the pages inside Contents book `book`, in order.
    fn book_topics(&self, book: usize) -> Vec<usize> {
        let depth = self.catalog.contents[book].depth;
        self.catalog.contents[book + 1..].iter().take_while(|e| e.depth > depth).filter_map(|e| e.topic).collect()
    }
    /// Returns true when the host should close the help window. The host handles Print.
    pub fn activate(&mut self, part: Part) -> bool {
        match part {
            Part::Close | Part::Cancel => return true,
            Part::Context => {
                self.context = !self.context;
                self.tip = None;
            }
            Part::Tab(tab) => self.choose_tab(tab),
            Part::Display => {
                if let Some(row) = self.selected.or_else(|| self.rows.first().copied()) {
                    self.selected = Some(row);
                    if let Some(topic) = self.row_topic(row) {
                        self.open_article(topic);
                    } else if self.tab == HelpTab::Contents {
                        if let Some(i) = self.collapsed.iter().position(|&i| i == row) {
                            self.collapsed.swap_remove(i);
                        } else {
                            self.collapsed.push(row);
                        }
                        self.rows = self.results();
                        self.first_row = self.first_row.min(self.rows.len().saturating_sub(self.visible_rows()));
                    }
                }
            }
            Part::Link(i) => {
                if let Some(&(_, target)) = self.links.get(i) {
                    self.open_article(target);
                }
            }
            Part::Contents => {
                if let Some(topic) = self.article.take() {
                    self.history.push(topic);
                }
                self.focus = Part::List;
                self.options = false;
            }
            Part::Back => {
                if let Some(topic) = self.history.pop() {
                    self.article = Some(topic);
                    self.focus = Part::Contents;
                }
                self.scroll = 0;
                self.options = false;
            }
            Part::Options => self.options = !self.options,
            Part::Print => self.options = false,
            _ => {}
        }
        false
    }

    pub fn open_article(&mut self, topic: usize) {
        if topic >= self.catalog.topics.len() {
            return;
        }
        if let Some(prior) = self.article.replace(topic) {
            if prior != topic {
                self.history.push(prior);
            }
        } else if self.history.last() == Some(&topic) {
            self.history.pop();
        }
        self.focus = Part::Contents;
        self.scroll = 0;
        self.options = false;
    }

    fn article_paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, topic: usize) {
        let c = &theme.colors;
        let (w, h) = self.size();
        let outer = Rect::new(0, 0, w, h);
        fb.fill(outer, c.face);
        fb.frame(outer, c.light, c.dark_shadow);
        fb.frame(outer.inset(1, 1, 1, 1), c.highlight, c.shadow);
        // The resizable WinHelp frame puts the standard caption one pixel farther in.
        let mut strip = Framebuffer::new(w - 2, 26, c.face);
        let buttons = [
            (TitleButton::Minimize, Part::Minimize),
            (TitleButton::Maximize, Part::Maximize),
            (TitleButton::Close, Part::Close),
        ]
        .map(|(button, part)| ButtonLook {
            button,
            rect: window::title_button_rect(w - 2, button),
            pressed: self.pressed == Some(part),
            enabled: true,
        });
        window::paint_frame(
            &mut strip,
            theme,
            fonts,
            &Frame {
                era: self.era,
                title: self.title(),
                icon: Some(&Self::icon()),
                active: self.active,
                buttons: &buttons,
            },
        );
        for y in 3..21 {
            let start = (y * strip.width + 3) as usize;
            let dest = ((y + 1) * w + 4) as usize;
            let len = (w - 8) as usize;
            fb.pixels[dest..dest + len].copy_from_slice(&strip.pixels[start..start + len]);
        }
        if self.is_maximized() {
            let b = self.rect(Part::Maximize).unwrap();
            let shift = i32::from(self.pressed == Some(Part::Maximize));
            fb.fill(b.inset(2, 2, 2, 2), c.face);
            for (dx, dy) in [(6, 2), (3, 5)] {
                let r = Rect::new(b.x + dx + shift, b.y + dy + shift, 7, 6);
                fb.fill(r, c.face);
                fb.frame(r, c.text, c.text);
                fb.hline(r.x, r.y + 1, r.w, c.text);
            }
        }
        dialog::well(fb, Rect::new(4, 23, w - 8, h - 27), c);
        let mut paper = self.viewport();
        fb.fill(Rect::new(6, 25, w - 12, paper.y - 25), c.face);
        fb.hline(6, paper.y - 1, w - 12, c.dark_shadow);
        for (p, title) in [(Part::Contents, "Help &Topics"), (Part::Back, "&Back"), (Part::Options, "&Options")] {
            let r = self.rect(p).unwrap();
            dialog::push_button(
                fb,
                theme,
                fonts.regular,
                r,
                title,
                false,
                self.pressed == Some(p),
                false,
                self.enabled(p),
                true,
            );
        }
        if self.max_scroll > 0 {
            paper.w -= 16;
        }
        help_article::paint(fb, theme, fonts, self.era,&self.catalog, &self.catalog.topics[topic], self.scroll, paper);
        self.paint_scrollbar(fb, theme);
        if self.options {
            let r = self.rect(Part::Print).unwrap();
            fb.fill(r, c.face);
            dialog::raised(fb, r, c);
            fonts.regular.draw_label(fb, (r.x + 8, r.y + 4), "&Print Topic...", 125, c.text, true);
        }
    }

    fn paint_scrollbar(&self, fb: &mut Framebuffer, theme: &Theme) {
        let c = &theme.colors;
        if let Some(track) = self.rect(Part::Track) {
            fb.fill(track, c.light);
            let thumb = self.rect(Part::Thumb).unwrap();
            fb.fill(thumb, c.face);
            dialog::raised(fb, thumb, c);
            for p in [Part::ScrollUp, Part::ScrollDown] {
                properties::scroll_arrow(fb, c, self.rect(p).unwrap(), p == Part::ScrollUp, self.pressed == Some(p));
            }
        }
    }

    fn picker_paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let c = &theme.colors;
        let f = fonts.regular;
        let (question, close) = dialog::title_button_rects(INDEX_SIZE.0);
        dialog::window_frame(
            fb,
            theme,
            fonts,
            self.era,
            self.title(),
            &TitleButtons {
                help: Some((question, self.pressed == Some(Part::Context))),
                close: Some((close, self.pressed == Some(Part::Close))),
            },
        );
        for t in HelpTab::ALL.into_iter().filter(|t| *t != self.tab) {
            let r = t.rect();
            properties::tab(fb, c, r);
            f.draw(fb, r.x + 6, r.y + 3, t.label(), c.text);
        }
        dialog::raised(fb, Rect::new(9, 49, 416, 341), c);
        let r = self.tab.rect();
        let active = r.inset(-2, -2, -2, -2);
        properties::tab(fb, c, active);
        let joined = active.x.max(11);
        fb.hline(joined, 50, active.right() - joined, c.face);
        f.draw(fb, r.x + 6, active.y + 3, self.tab.label(), c.text);
        if self.focus == Part::Tab(self.tab) {
            dialog::focus_dots(fb, active.inset(3, 3, 3, 1), (0, 0), c.text);
        }
        if self.tab == HelpTab::Contents {
            let hint = if self.selected_book().is_some() {
                "Click a book, and then click Open. Or click another tab, such as Index."
            } else {
                "Click a topic, and then click Display. Or click another tab, such as Index."
            };
            f.draw(fb, 27, 72, hint, c.text);
        } else {
            let (first, second) = if self.tab == HelpTab::Index {
                (
                    "&Type the first few letters of the word you're looking for.",
                    "&Click the index entry you want, and then click Display.",
                )
            } else {
                ("&Type the word or words you want to find.", "&Click a topic, and then click Display.")
            };
            for (x, y, n, instruction) in [(27, 72, "1", first), (26, 121, "2", second)] {
                f.draw(fb, x, y, n, c.text);
                f.draw_label(fb, (x + 12, y), instruction, 365, c.text, true);
            }
            dialog::well(fb, EDIT, c);
            let mut edit = Framebuffer::new(EDIT.w - 6, 16, rgb(0xffffff));
            let caret = f.measure(&self.query[..self.caret]);
            let x = 0.min(edit.width - 2 - caret);
            f.draw(&mut edit, x, 1, &self.query, c.text);
            // The selection, as Windows' edit box draws it (brief 99).
            let (s, e) = (self.caret.min(self.anchor).min(self.query.len()), self.caret.max(self.anchor).min(self.query.len()));
            if self.focus == Part::Query && s != e {
                let sx = x + f.measure(&self.query[..s]);
                let part = &self.query[s..e];
                edit.fill(Rect::new(sx, 1, f.measure(part), f.height), c.menu_highlight);
                f.draw(&mut edit, sx, 1, part, c.menu_highlight_text);
            }
            if self.focus == Part::Query && self.blink {
                edit.vline(x + caret, 1, f.height, c.text);
            }
            fb.blit(&edit, EDIT.x + 3, EDIT.y + 2);
        }
        let contents = self.tab == HelpTab::Contents;
        let list = self.rect(Part::List).unwrap();
        let inside = dialog::well(fb, list, c);
        let mut panel = Framebuffer::new(inside.w, inside.h, rgb(0xffffff));
        for (row, i) in self.rows.iter().copied().skip(self.first_row).take(self.visible_rows() + 1).enumerate() {
            let y = row as i32 * if contents { 18 } else { f.height };
            let indent = if contents { (self.catalog.contents[i].depth.min(30) * 16) as i32 } else { 0 };
            if contents {
                let image = match self.catalog.contents[i].icon {
                    EntryIcon::Book if self.collapsed.contains(&i) => &BOOK,
                    EntryIcon::Book => &OPEN_BOOK,
                    EntryIcon::Overview | EntryIcon::Page => &PAGE,
                };
                contents_icon(&mut panel, image, indent, y);
            }
            let x = if contents { 54 + indent - inside.x } else { 43 - inside.x };
            let label = self.label(i);
            let selected = self.selected == Some(i);
            if selected {
                let highlight = if contents {
                    Rect::new(x - 2, y, f.measure(label) + 4, 18)
                } else {
                    Rect::new(0, y, inside.w, f.height)
                };
                panel.fill(highlight, c.menu_highlight);
                if self.focus == Part::List {
                    dialog::focus_dots(&mut panel, highlight, (1 - inside.x, -inside.y), c.text);
                    dialog::focus_dots(
                        &mut panel,
                        highlight,
                        (-inside.x, -inside.y),
                        rgb(0xffffff ^ (c.menu_highlight & 0xffffff)),
                    );
                }
            }
            f.draw(
                &mut panel,
                x,
                y + if contents { 2 } else { 0 },
                label,
                if selected { c.menu_highlight_text } else { c.text },
            );
        }
        fb.blit(&panel, inside.x, inside.y);
        self.paint_scrollbar(fb, theme);
        // A chosen book's button opens it, or closes it when it is open (win95-truecolor-calc-help-topics).
        let display = match self.selected_book() {
            Some(book) if self.collapsed.contains(&book) => "&Open",
            Some(_) => "C&lose",
            None => "&Display",
        };
        for (p, name) in [(Part::Display, display), (Part::Print, "&Print..."), (Part::Cancel, "Cancel")] {
            let r = Self::button_rect(p);
            let default = if matches!(self.focus, Part::Print | Part::Cancel) { self.focus } else { Part::Display };
            dialog::push_button(
                fb,
                theme,
                f,
                r,
                name,
                p == default,
                self.pressed == Some(p),
                self.focus == p && self.show_keys,
                self.enabled(p),
                self.show_keys,
            );
        }
        if let Some((part, x, y)) = self.tip {
            let message = match part {
                Part::Query => "Type words from the topic you want to find.",
                Part::Tab(HelpTab::Find) => "Search for words in the Help topics.",
                Part::Tab(HelpTab::Index) => "Look up a topic by keyword.",
                Part::Print => "Print the selected Help topic.",
                Part::Close | Part::Cancel => "Close Help Topics.",
                _ => "Select a topic, then choose Display to read it.",
            };
            let (w, h) = crate::tooltip::size(f, message);
            let mut tip = Framebuffer::new(w, h, c.tip_face);
            crate::tooltip::paint(&mut tip, theme, f, message);
            fb.blit(&tip, x.min(fb.width - w).max(0), (y + 12).min(fb.height - h));
        }
    }
    fn button_rect(p: Part) -> Rect {
        Rect::new(
            match p {
                Part::Display => 188,
                Part::Print => 269,
                _ => 350,
            },
            396,
            75,
            23,
        )
    }
    fn scroll_metrics(&self) -> (i32, i32, i32) {
        if self.is_article() {
            (self.scroll, self.max_scroll, self.viewport().h)
        } else {
            let page = self.visible_rows();
            (self.first_row as i32, self.rows.len().saturating_sub(page) as i32, page as i32)
        }
    }
    fn scroll_to(&mut self, value: i32) {
        let value = value.clamp(0, self.scroll_metrics().1);
        if self.is_article() {
            self.scroll = value;
        } else {
            self.first_row = value as usize;
        }
    }
    fn step_row(&mut self, delta: i32) {
        let rows = &self.rows;
        if rows.is_empty() {
            return;
        }
        let row = rows.iter().position(|i| Some(*i) == self.selected).map_or(-1, |i| i as i32);
        self.selected = Some(rows[(row + delta).clamp(0, rows.len() as i32 - 1) as usize]);
        self.reveal_selection();
    }
    fn visible_rows(&self) -> usize {
        if self.tab == HelpTab::Contents { ((LIST.h - 4) / 18) as usize } else { ((SEARCH_LIST.h - 4) / 13) as usize }
    }
    fn reveal_selection(&mut self) {
        if let Some(row) = self.rows.iter().position(|i| Some(*i) == self.selected) {
            self.first_row = self.first_row.clamp((row + 1).saturating_sub(self.visible_rows()), row);
        }
    }
}

impl Form for HelpTopics {
    type Part = Part;
    fn title(&self) -> &str {
        if self.is_article() { &self.catalog.title } else { &self.picker_title }
    }
    fn size(&self) -> (i32, i32) {
        if self.is_article() {
            self.maximized_size.or(self.manual_size).unwrap_or((self.auto_width, self.auto_height))
        } else {
            INDEX_SIZE
        }
    }
    fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    fn layout(&mut self, fonts: &Fonts) {
        if let Some(i) = self.article {
            let topic = &self.catalog.topics[i];
            let view = self.viewport();
            let mut content = help_article::measure(fonts, &self.catalog, topic, view.w, &mut self.links);
            if self.maximized_size.is_none() && self.manual_size.is_none() {
                self.auto_height = (view.y + content + 6).clamp(MIN_ARTICLE.1, self.max_height);
            }
            let view = self.viewport();
            if content > view.h {
                content = help_article::measure(fonts, &self.catalog, topic, view.w - 16, &mut self.links);
            }
            self.max_scroll = (content - view.h).max(0);
            self.scroll = self.scroll.clamp(0, self.max_scroll);
        } else if self.tab != HelpTab::Contents {
            let f = fonts.regular;
            let start = EDIT.x + 3 + 0.min(EDIT.w - 8 - f.measure(&self.query[..self.caret]));
            let mut x = start;
            self.query_positions.clear();
            for (i, ch) in self.query.char_indices() {
                self.query_positions.push((i, x));
                x += f.glyph(ch).advance;
            }
            self.query_positions.push((self.query.len(), x));
        }
    }
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        if let Some(i) = self.article {
            self.article_paint(fb, theme, fonts, i);
        } else {
            self.picker_paint(fb, theme, fonts);
        }
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        let parts = [
            Part::Close,
            Part::Minimize,
            Part::Maximize,
            Part::Cancel,
            Part::Context,
            Part::Print,
            Part::Contents,
            Part::Back,
            Part::Options,
            Part::ScrollUp,
            Part::ScrollDown,
            Part::Thumb,
            Part::Track,
            Part::Tab(HelpTab::Contents),
            Part::Tab(HelpTab::Index),
            Part::Tab(HelpTab::Find),
            Part::Query,
            Part::List,
            Part::Display,
        ];
        parts.into_iter().find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y))).or_else(|| {
            let view = self.viewport();
            if !self.is_article() || !view.contains(x, y) {
                return None;
            }
            self.links.iter().position(|(r, _)| r.contains(x - view.x, y - view.y + self.scroll)).map(Part::Link)
        })
    }
    fn rect(&self, p: Part) -> Option<Rect> {
        if matches!(p, Part::ScrollUp | Part::ScrollDown | Part::Thumb | Part::Track) {
            let (value, max, page) = self.scroll_metrics();
            if max == 0 {
                return None;
            }
            let v = if self.is_article() { self.viewport() } else { self.rect(Part::List)?.inset(2, 2, 2, 2) };
            let arrow = 16.min(v.h / 2);
            let track = Rect::new(v.right() - 16, v.y + arrow, 16, v.h - 2 * arrow);
            return Some(match p {
                Part::ScrollUp => Rect::new(track.x, v.y, 16, arrow),
                Part::ScrollDown => Rect::new(track.x, v.bottom() - arrow, 16, arrow),
                Part::Thumb => {
                    let h = (track.h as i64 * page as i64 / (page + max) as i64)
                        .clamp(8.min(track.h) as i64, track.h as i64) as i32;
                    let offset = ((track.h - h) as i64 * value as i64 / max as i64) as i32;
                    Rect::new(track.x, track.y + offset, 16, h)
                }
                _ => track,
            });
        }
        if self.is_article() {
            if let Some(button) = match p {
                Part::Close => Some(TitleButton::Close),
                Part::Minimize => Some(TitleButton::Minimize),
                Part::Maximize => Some(TitleButton::Maximize),
                _ => None,
            } {
                let r = window::title_button_rect(self.size().0 - 2, button);
                return Some(Rect::new(r.x + 1, r.y + 1, r.w, r.h));
            }
            let v = self.viewport();
            return Some(match p {
                Part::Contents => Self::toolbar_button(self.size().0, 0),
                Part::Back => Self::toolbar_button(self.size().0, 1),
                Part::Options => Self::toolbar_button(self.size().0, 2),
                Part::Print if self.options => {
                    let options = Self::toolbar_button(self.size().0, 2);
                    Rect::new(options.x.min(self.size().0 - 100), options.bottom() - 1, 100, 22)
                }
                Part::Link(i) => {
                    let (r, _) = self.links.get(i)?;
                    Rect::new(v.x + r.x, v.y + r.y - self.scroll, r.w, r.h)
                }
                _ => return None,
            });
        }
        Some(match p {
            Part::Context => dialog::title_button_rects(self.size().0).0,
            Part::Close => dialog::title_button_rects(self.size().0).1,
            Part::Tab(t) => t.rect(),
            Part::Query if self.tab != HelpTab::Contents => EDIT,
            Part::List => {
                if self.tab == HelpTab::Contents {
                    LIST
                } else {
                    SEARCH_LIST
                }
            }
            Part::Print | Part::Display | Part::Cancel => Self::button_rect(p),
            _ => return None,
        })
    }
    fn enabled(&self, p: Part) -> bool {
        match p {
            Part::Print => {
                self.selected.and_then(|row| self.row_topic(row)).is_some()
                    || self.is_article()
                    || self.selected_book().is_some_and(|book| !self.book_topics(book).is_empty())
            }
            Part::Display => !self.rows.is_empty(),
            Part::Back => !self.history.is_empty(),
            _ => true,
        }
    }
    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        self.focus = part;
        self.blink = true;
        if let Part::Link(i) = part
            && let Some(&(r, _)) = self.links.get(i)
        {
            self.scroll = self.scroll.clamp((r.bottom() - self.viewport().h).max(0), r.y);
        }
    }
    fn tab_stops(&self) -> Vec<Part> {
        if self.is_article() {
            let mut parts: Vec<_> =
                [Part::Contents, Part::Back, Part::Options].into_iter().filter(|&p| self.enabled(p)).collect();
            for (i, &(_, target)) in self.links.iter().enumerate() {
                if i == 0 || self.links[i - 1].1 != target {
                    parts.push(Part::Link(i));
                }
            }
            return parts;
        }
        let mut v = vec![Part::Tab(self.tab)];
        if self.tab != HelpTab::Contents {
            v.push(Part::Query);
        }
        v.extend([Part::List, Part::Display, Part::Print, Part::Cancel].into_iter().filter(|&p| self.enabled(p)));
        v
    }
    fn set_pressed(&mut self, p: Option<Part>) {
        self.pressed = p;
    }
    fn show_keys(&mut self) {
        self.show_keys = true;
    }
    fn enter(&self) -> Part {
        if self.is_article() {
            return self.focus;
        }
        if matches!(self.focus, Part::Print | Part::Cancel) { self.focus } else { Part::Display }
    }
    fn escape(&self) -> Part {
        if self.options { Part::Options } else { Part::Close }
    }
    fn key(&mut self, k: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        if self.tip.take().is_some() && k == Key::Escape {
            return Some(Vec::new());
        }
        if self.context && k == Key::Escape {
            self.context = false;
            return Some(Vec::new());
        }
        if !self.is_article() && mods.shift && k == Key::Other(0x70) {
            self.context = true;
            return Some(Vec::new());
        }
        if self.is_article() {
            let y = match k {
                Key::Up => self.scroll - 13,
                Key::Down => self.scroll + 13,
                Key::PageUp => self.scroll - self.viewport().h,
                Key::PageDown => self.scroll + self.viewport().h,
                Key::Home => 0,
                Key::End => self.max_scroll,
                _ => return None,
            };
            self.scroll = y.clamp(0, self.max_scroll);
        } else {
            // The query field: Windows' edit box (caret keys, words, Backspace, Delete, Cut, Copy,
            // Paste, Undo, Ctrl+A, Shift+F10; brief 99). Up and Down still move the list.
            if self.focus == Part::Query && !matches!(k, Key::Up | Key::Down | Key::Tab | Key::PageUp | Key::PageDown) {
                self.pull();
                let done = kit::field_key(&mut self.edit, k, mods, &mut self.copied)?;
                self.push();
                return Some(done.effects);
            }
            match (k, self.focus) {
                (Key::Tab, _) if mods.ctrl => self.choose_tab(self.tab.step(if mods.shift { -1 } else { 1 })),
                (Key::Left | Key::Right, Part::Tab(_)) => {
                    self.choose_tab(self.tab.step(if k == Key::Left { -1 } else { 1 }));
                    self.focus = Part::Tab(self.tab);
                }
                (Key::Up | Key::Down, Part::List | Part::Query) => self.step_row(if k == Key::Up { -1 } else { 1 }),
                (Key::PageUp | Key::PageDown, Part::List) => {
                    self.step_row(self.visible_rows() as i32 * if k == Key::PageUp { -1 } else { 1 })
                }
                _ => return None,
            }
        }
        Some(Vec::new())
    }
    fn press(&mut self, part: Option<Part>, x: i32, y: i32) -> Press<Part> {
        if std::mem::take(&mut self.context) {
            self.tip = part.filter(|p| *p != Part::Context).map(|p| (p, x, y));
            return Press::Done;
        }
        self.tip = None;
        let Some(p) = part else {
            self.options = false;
            return Press::Nothing;
        };
        if !self.enabled(p) {
            return Press::Done;
        }
        self.focus = p;
        match p {
            Part::Tab(t) => {
                self.choose_tab(t);
                Press::Done
            }
            Part::List => {
                if let Some(i) = self.click_row(x, y) {
                    self.selected = Some(i);
                }
                Press::Done
            }
            Part::Query => {
                // The caret there; held, a drag selects (brief 99).
                let at = self.query_at(x);
                self.caret = at;
                self.anchor = at;
                self.blink = true;
                Press::Grab(Part::Query, 0)
            }
            Part::ScrollUp | Part::ScrollDown => {
                let step = if self.is_article() { 13 } else { 1 };
                self.scroll_to(self.scroll_metrics().0 + if p == Part::ScrollUp { -step } else { step });
                Press::Done
            }
            Part::Track => {
                let (value, _, page) = self.scroll_metrics();
                self.scroll_to(value + if y < self.rect(Part::Thumb).unwrap().y { -page } else { page });
                Press::Done
            }
            Part::Thumb => Press::Grab(p, self.scroll_metrics().0),
            _ => Press::Hold(p),
        }
    }
    fn double_click(&mut self, x: i32, y: i32) -> Option<Vec<Effect<Part>>> {
        // In the query field: a word selected (brief 99).
        if !self.is_article() && self.hit(x, y) == Some(Part::Query) {
            self.focus = Part::Query;
            self.pull();
            let at = self.query[..self.query_at(x)].chars().count();
            self.edit.double_click(at);
            self.push();
            return Some(Vec::new());
        }
        if self.is_article() && (4..22).contains(&y) && self.hit(x, y).is_none() {
            return Some(vec![Effect::Activate(Part::Maximize)]);
        }
        self.selected = Some(self.click_row(x, y)?);
        Some(vec![Effect::Activate(Part::Display)])
    }
    fn drag(&mut self, p: Part, grab_y: i32, value: i32, x: i32, y: i32) -> bool {
        if p == Part::Query {
            let at = self.query_at(x);
            let changed = at != self.caret;
            self.caret = at;
            self.blink = true;
            return changed;
        }
        let max = self.scroll_metrics().1;
        if p != Part::Thumb || max == 0 {
            return false;
        }
        let travel = self.rect(Part::Track).unwrap().h - self.rect(Part::Thumb).unwrap().h;
        self.scroll_to(
            (value as i64 + (y - grab_y) as i64 * max as i64 / travel.max(1) as i64).clamp(0, max as i64) as i32
        );
        true
    }
    /// Backspace comes as its key (`key`); its character is left out here.
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        if self.focus == Part::Query {
            self.pull();
            if self.edit.type_char(ch) {
                self.push();
            }
        }
        Vec::new()
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        let p = match (self.is_article(), ch.to_ascii_lowercase()) {
            (true, 't') => Part::Contents,
            (true, 'b') => Part::Back,
            (true, 'o') => Part::Options,
            (_, 'p') => Part::Print,
            (false, 'o' | 'l') if self.selected_book().is_some() => Part::Display,
            (false, 'd') if self.selected_book().is_none() => Part::Display,
            (false, 't') if self.tab != HelpTab::Contents => Part::Query,
            (false, 'c') if self.tab != HelpTab::Contents => Part::List,
            _ => return Vec::new(),
        };
        if matches!(p, Part::Query | Part::List) {
            self.set_focus(p);
            Vec::new()
        } else if self.enabled(p) {
            vec![Effect::Activate(p)]
        } else {
            Vec::new()
        }
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        if matches!(self.focus, Part::Query | Part::List | Part::Tab(_)) {
            Vec::new()
        } else {
            vec![Effect::Activate(self.focus)]
        }
    }
    /// Text from the clipboard: its first line, cut to the field's limit (brief 99; before, every
    /// line was run together).
    fn paste(&mut self, text: &str) {
        if self.focus == Part::Query {
            self.pull();
            if self.edit.paste(text) {
                self.push();
            }
        }
    }
    fn take_copy(&mut self) -> Option<String> {
        self.copied.take()
    }
    fn field_menu(&self, clipboard_has_text: bool) -> Option<crate::paint::Menu> {
        if self.focus != Part::Query || self.is_article() {
            return None;
        }
        let mut edit = self.edit.clone();
        if edit.text != self.query {
            edit.set(&self.query);
        }
        edit.caret = self.query[..self.caret.min(self.query.len())].chars().count();
        edit.anchor = self.query[..self.anchor.min(self.query.len())].chars().count();
        Some(edit.menu(clipboard_has_text))
    }
    fn field_menu_at(&self) -> Option<(i32, i32)> {
        crate::edit::middle(EDIT)
    }
    fn field_menu_chosen(&mut self, index: usize) -> Vec<Effect<Part>> {
        self.pull();
        let done = kit::field_menu_chosen(&mut self.edit, index, &mut self.copied);
        self.push();
        done.effects
    }
    /// The query field's right button: its edit menu as it comes up (brief 99).
    fn button(&mut self, button: kit::Button, down: bool, x: i32, y: i32) -> Vec<Effect<Part>> {
        if button == kit::Button::Right && !self.is_article() && self.hit(x, y) == Some(Part::Query) {
            self.focus = Part::Query;
            return if down { Vec::new() } else { vec![Effect::FieldMenu] };
        }
        Vec::new()
    }
    fn wheel(&mut self, notches: i32) {
        if self.is_article() {
            self.scroll = (self.scroll - notches * 39).clamp(0, self.max_scroll);
        } else {
            self.first_row = self
                .first_row
                .saturating_add_signed(-3 * notches as isize)
                .min(self.rows.len().saturating_sub(self.visible_rows()));
        }
    }
    fn has_caret(&self) -> bool {
        self.focus == Part::Query && !self.is_article()
    }
    fn context_help(&self) -> bool {
        self.context
    }
    fn blink(&mut self, on: bool) {
        self.blink = on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::help::{Entry, Keyword, Paragraph, Style, Topic};
    use crate::kit::Controller;

    fn book() -> Catalog {
        let topics = ["Getting started", "Saving files"].map(|title| Topic {
            title: title.into(),
            paragraphs: vec![Paragraph { style: Style::Body, text: format!("Read about {title}") }],
            welcome: false,
        });
        Catalog {
            title: "Editor Help".into(),
            home: 0,
            contents: topics
                .iter()
                .enumerate()
                .map(|(i, t)| Entry { label: t.title.clone(), depth: 0, topic: Some(i), icon: EntryIcon::Page })
                .collect(),
            index: vec![
                Keyword { label: "start".into(), topic: 0 },
                Keyword { label: "files".into(), topic: 1 },
                Keyword { label: "save".into(), topic: 1 },
            ],
            topics: topics.into(),
        }
    }

    /// Calculator's 118 keywords, several of them for more than one topic: one line each,
    /// opening the keyword's first topic.
    #[test]
    fn a_keyword_of_several_topics_is_one_line_of_the_index() {
        let book = crate::calculator::help::catalog(Era::Win95);
        let help = HelpTopics::new(Era::Win95, book.clone(), HelpTab::Index);
        assert_eq!(help.rows.len(), 118);
        let calculations = help.rows.iter().position(|&r| help.label(r) == "calculations").unwrap();
        let first = book.index.iter().find(|k| k.label == "calculations").unwrap().topic;
        assert_eq!(help.row_topic(help.rows[calculations]), Some(first));
    }

    /// A chosen book (Calculator's "Performing Calculations"): Open, and Print prints its pages.
    #[test]
    fn a_chosen_book_opens_and_prints_its_pages() {
        let book = crate::calculator::help::catalog(Era::Win95);
        let mut help = HelpTopics::new(Era::Win95, book.clone(), HelpTab::Contents);
        assert_eq!(help.selected_book(), Some(0));
        assert!(help.enabled(Part::Print));
        let pages = [0, 2, 3, 4].map(|i| book.topics[i].plain_text()).join("\n\n");
        assert_eq!(help.print_text(), pages);
        help.activate(Part::Display);
        assert!(!help.collapsed.contains(&0));
        assert_eq!(help.rows.len(), 6);
    }

    #[test]
    fn find_opens_and_prints_the_selected_topic() {
        let mut help = HelpTopics::new(Era::Win95, book(), HelpTab::Find);
        let mut controller = Controller::default();
        assert_eq!(help.title(), "Help Topics: Editor Help");
        help.paste("files");
        assert_eq!(help.rows, [1]);
        assert_eq!(controller.key(&mut help, Key::Enter, Mods::default()), [Effect::Activate(Part::Display)]);
        help.activate(Part::Display);
        assert_eq!(help.print_text(), "Read about Saving files");
        assert_eq!(help.title(), "Editor Help");
        help.activate(Part::Contents);
        help.choose_tab(HelpTab::Index);
        assert_eq!(help.rows, [1, 2, 0]);
        help.paste("star");
        help.activate(Part::Display);
        assert_eq!(help.print_text(), "Read about Getting started");
        help.activate(Part::Back);
        assert_eq!(help.print_text(), "Read about Saving files");
    }

    #[test]
    fn double_clicks_open_only_rows_and_maximize_only_the_topic_caption() {
        let mut help = HelpTopics::new(Era::Win95, book(), HelpTab::Contents);
        assert_eq!(help.double_click(70, 109), Some(vec![Effect::Activate(Part::Display)]));
        assert_eq!(help.double_click(70, 302), None);
        help.activate(Part::Display);
        assert_eq!(help.double_click(80, 12), Some(vec![Effect::Activate(Part::Maximize)]));
        help.set_article_size(Some((640, 480)));
        assert!(help.is_maximized());
        assert_eq!(help.viewport(), Rect::new(6, 46, 628, 428));
        help.set_article_size(None);
        assert!(!help.is_maximized());
        help.set_window_size((500, 300), false);
        assert_eq!(help.size(), (500, 300));
        assert!(!help.is_maximized());
    }

    #[test]
    fn a_book_expands_and_a_long_list_scrolls_to_the_selected_row() {
        let mut catalog = book();
        let folder = Entry { label: "Manual".into(), topic: None, depth: 0, icon: EntryIcon::Book };
        catalog.contents = std::iter::once(folder)
            .chain((0..40).map(|i| Entry {
                label: format!("Section {i}"),
                topic: Some(i % 2),
                depth: 1,
                icon: EntryIcon::Page,
            }))
            .collect();
        let mut help = HelpTopics::new(Era::Win95, catalog, HelpTab::Contents);
        assert_eq!(help.rows, [0]);
        // The chosen book prints its pages (WinHelp's Print on a book).
        assert!(help.enabled(Part::Print));
        help.activate(Part::Display);
        help.step_row(40);
        assert_eq!(help.print_text(), "Read about Saving files");
        assert!(help.first_row > 0 && help.rect(Part::Thumb).is_some());
        help.step_row(-40);
        help.activate(Part::Display);
        assert_eq!(help.rows, [0]);
        assert_eq!(help.first_row, 0);
    }

    #[test]
    fn links_in_the_supplied_article_open_the_target_and_back_returns() {
        let mut catalog = book();
        catalog.topics[0].paragraphs.push(Paragraph { style: Style::Related(vec![1]), text: "See also".into() });
        let mut help = HelpTopics::article(Era::Win95, catalog, 0).unwrap();
        let font = crate::text::Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let fonts = Fonts { regular: &font, bold: &font };
        help.layout(&fonts);
        let r = help.rect(Part::Link(0)).unwrap();
        assert_eq!(help.hit(r.x, r.y), Some(Part::Link(0)));
        help.activate(Part::Link(0));
        assert_eq!(help.article, Some(1));
        help.activate(Part::Back);
        assert_eq!(help.article, Some(0));
        assert!(!help.enabled(Part::Back));
    }

    #[test]
    fn article_fits_the_wrapped_text_until_it_reaches_the_available_height() {
        let regular = crate::text::Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let bold = crate::text::Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap();
        let fonts = Fonts { regular: &regular, bold: &bold };
        let mut help = HelpTopics::article(Era::Win95, crate::solitaire::help::catalog(Era::Win95), 0).unwrap();
        help.set_article_bounds(281, 508);
        help.layout(&fonts);
        let normal = help.size();
        assert!((363..=508).contains(&normal.1), "the original 362px cap cut off this topic");
        assert!(help.rect(Part::Track).is_none());

        // WinHelp keeps the top edge in place. With less room below it the same topic scrolls.
        help.set_window_size(normal, false); // initial WM_SIZE from Windows
        help.set_article_bounds(281, 250);
        help.layout(&fonts);
        assert_eq!(help.size(), (281, 250));
        assert!(help.rect(Part::Track).is_some());
        help.key(Key::End, Mods::default());
        assert_eq!(help.scroll, help.max_scroll);

        // A narrow window wraps both the text and the toolbar, as in the overflow capture.
        help.set_article_bounds(200, 508);
        help.layout(&fonts);
        assert_eq!(help.size(), (200, 508));
        assert_eq!(help.viewport().y, 67);
        assert_eq!(help.hit(20, 55), Some(Part::Options));
        assert!(help.rect(Part::Track).is_some());
        help.set_article_bounds(420, 508);
        help.layout(&fonts);
        assert!(help.size().1 < normal.1);
        assert!(help.rect(Part::Track).is_none());
        assert_eq!(help.scroll, 0);
    }

    #[test]
    fn maximize_and_restore_preserve_automatic_and_user_sized_articles() {
        let font = crate::text::Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let fonts = Fonts { regular: &font, bold: &font };
        let mut help = HelpTopics::article(Era::Win95, crate::solitaire::help::catalog(Era::Win95), 0).unwrap();
        help.set_article_bounds(281, 508);
        help.layout(&fonts);
        let original = help.size();
        help.set_window_size((800, 572), true);
        help.open_article(1);
        help.layout(&fonts);
        assert_eq!(help.size(), (800, 572));
        help.set_window_size(original, false);
        help.layout(&fonts);
        assert_ne!(help.size().1, original.1);
        assert_eq!(help.size().0, original.0);

        help.set_window_size((200, 100), false);
        help.set_window_size((800, 572), true);
        help.open_article(0);
        help.layout(&fonts);
        help.set_window_size((200, 100), false);
        help.layout(&fonts);
        assert_eq!(help.size(), (200, 100));
        assert_eq!(help.rect(Part::Thumb).unwrap().h, 1);
        assert!(help.rect(Part::Track).is_some());
    }
}
