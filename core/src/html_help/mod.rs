//! The two-pane HTML Help window used by Windows 98 and Windows 2000.
//! The caller supplies the contents tree, keyword index, and articles as a Help catalog.

mod article;
pub mod chrome;
pub mod commands;
mod fonts;
mod icons;
#[cfg(test)]
mod tests;
pub mod topics;

use crate::fb::{Framebuffer, Icon, Rect};
use crate::help::{Catalog, EntryIcon, Style};
use crate::kit::{self, Effect, Form, Key, Mods, Press};
use crate::paint::{self, Menu, MenuEntry, MenuLayout, MenuStyle};
use crate::text::Fonts;
use crate::theme::{Era, Theme, theme};
use chrome::{Chrome, Hit, Layout, Row};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Contents,
    Index,
    Search,
}

impl Tab {
    const ALL: [Self; 3] = [Self::Contents, Self::Index, Self::Search];
    fn step(self, backwards: bool) -> Self {
        Self::ALL[(self as usize + if backwards { 2 } else { 1 }) % 3]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Close,
    Minimize,
    Maximize,
    Print,
    Find,
    Copy,
    WindowMenu,
    InternetOptions,
    TopicsFound,
    Control(Hit),
    Link(usize),
    MenuItem(usize),
    Dismiss,
}

#[derive(Clone, Copy)]
enum Action {
    Part(Part),
    Topic(usize),
    Home,
    Refresh,
    Highlight,
    SelectAll,
}

struct Popup {
    items: Vec<(MenuEntry, Option<Action>)>,
    anchor: (i32, i32),
    layout: Option<(Rect, MenuLayout)>,
    hover: Option<usize>,
    options: bool,
    keys: bool,
}

impl Popup {
    fn menu(&self) -> Menu {
        Menu {
            entries: self.items.iter().map(|(entry, _)| entry.clone()).collect(),
            hover: self.hover,
            show_keys: self.keys,
            ..Default::default()
        }
    }
}

struct IndexEntry {
    label: String,
    topics: Vec<usize>,
}

struct Visit {
    topic: usize,
    scroll: i32,
    search_terms: Vec<String>,
}

pub struct HtmlHelp {
    era: Era,
    catalog: Catalog,
    index: Vec<IndexEntry>,
    expanded: Vec<bool>,
    tree: Vec<usize>,
    tab: Tab,
    nav: bool,
    restored_navigation: bool,
    nav_focused: bool,
    size: (i32, i32),
    split: i32,
    active: bool,
    maximized: bool,
    keys: bool,
    keyboard_input: bool,
    focus: Part,
    pressed: Option<Part>,
    hover: Option<Part>,
    selected: Option<usize>,
    first: usize,
    /// How far the Contents tree is scrolled to the left, and where a drag of its thumb began.
    tree_left: i32,
    tree_drag_from: i32,
    history: Vec<Visit>,
    visit: usize,
    visited: Vec<usize>,
    article: article::ArticleLayout,
    page_width: i32,
    page_topic: usize,
    query: String,
    caret: usize,
    anchor: Option<usize>,
    /// The query field: Windows' edit box (`edit::Edit`, brief 99: keys, words, Cut, Copy, Paste,
    /// Undo, its menu); `query`, `caret` and `anchor` (bytes) are its copy for drawing (`push`).
    edit: crate::edit::Edit,
    copied: Option<String>,
    query_positions: Vec<(usize, i32)>,
    blink: bool,
    results: Vec<usize>,
    highlight: bool,
    search_terms: Vec<String>,
    web: usize,
    query_by_tab: [String; 3],
    selection: Option<Range<usize>>,
    selection_start: usize,
    matches: Vec<Range<usize>>,
    last_find_text: String,
    popup: Option<Popup>,
}

impl HtmlHelp {
    pub fn new(era: Era, mut catalog: Catalog, tab: Tab) -> Self {
        let size = chrome::native_size(era);
        let split = chrome::native_split(era);
        let width = chrome::layout(era, size, split, true, tab).article.w;
        let home = catalog.home;
        let web = catalog.topics.len();
        catalog.topics.extend(article::web_topics(era, web));
        let article = article::layout(era, &catalog.topics[home], width);
        // HTML Help lists one keyword once. Display asks which topic when the keyword
        // was registered for several pages.
        let mut keys: Vec<_> = catalog.index.iter().collect();
        keys.sort_by_cached_key(|key| key.label.to_lowercase());
        let mut index: Vec<IndexEntry> = Vec::new();
        for key in keys {
            if let Some(last) = index.last_mut().filter(|item| item.label.eq_ignore_ascii_case(&key.label)) {
                if !last.topics.contains(&key.topic) {
                    last.topics.push(key.topic);
                }
            } else {
                index.push(IndexEntry { label: key.label.clone(), topics: vec![key.topic] });
            }
        }
        // HTML Help opens with the first entry of the tree chosen and, if it is a book, open;
        // the other books stay closed (98's Calculator book has two at the top).
        let expanded = catalog.contents.iter().enumerate().map(|(i, entry)| i == 0 && entry.depth == 0).collect();
        let mut help = Self {
            era,
            catalog,
            index,
            expanded,
            tree: Vec::new(),
            tab,
            nav: true,
            restored_navigation: false,
            nav_focused: true,
            size,
            split,
            active: true,
            maximized: false,
            keys: true,
            keyboard_input: false,
            focus: Part::Control(Hit::Article),
            pressed: None,
            hover: None,
            selected: None,
            first: 0,
            tree_left: 0,
            tree_drag_from: 0,
            history: vec![Visit { topic: home, scroll: 0, search_terms: Vec::new() }],
            visit: 0,
            visited: vec![home],
            article,
            page_width: width,
            page_topic: home,
            query: String::new(),
            caret: 0,
            anchor: None,
            edit: crate::edit::Edit::default().with_rule(Some(crate::edit::HELP_QUERY_LIMIT), crate::edit::Allow::Any),
            copied: None,
            query_positions: Vec::new(),
            blink: true,
            results: Vec::new(),
            highlight: true,
            search_terms: Vec::new(),
            web,
            query_by_tab: Default::default(),
            popup: None,
            selection: None,
            selection_start: 0,
            matches: Vec::new(),
            last_find_text: String::new(),
        };
        help.rebuild_tree();
        help.choose_tab(tab);
        help
    }

    pub fn icon(era: Era) -> Icon {
        chrome::icon(era)
    }
    pub fn era(&self) -> Era {
        self.era
    }

    pub fn set_size(&mut self, size: (i32, i32)) {
        let size = (size.0.max(215), size.1.max(200));
        if self.size != size {
            self.restored_navigation = false;
        }
        self.size = size;
    }

    pub fn set_maximized(&mut self, maximized: bool) {
        self.maximized = maximized;
    }

    fn layout_now(&self) -> Layout {
        let mut layout = chrome::layout(self.era, self.size, self.split, self.nav, self.tab);
        if self.nav && self.restored_navigation && self.era == Era::Win98 {
            layout.restored_navigation();
        }
        if self.tab == Tab::Contents {
            let font = chrome::tree_font(self.era);
            layout.tree_rows = self.tree.len();
            layout.tree_width = self
                .tree
                .iter()
                .map(|&i| {
                    let entry = &self.catalog.contents[i];
                    19 + 19 * entry.depth as i32 + font.measure(&entry.label) + 4
                })
                .max()
                .unwrap_or(0);
            layout.tree_left = self.tree_left.clamp(0, layout.tree_max_left());
        }
        layout
    }

    /// Scrolls the Contents tree sideways to `left` (kept within what there is to see).
    fn scroll_tree_to(&mut self, left: i32) {
        self.tree_left = left.clamp(0, self.layout_now().tree_max_left());
    }
    fn topic(&self) -> usize {
        self.history[self.visit].topic
    }
    fn scroll(&self) -> i32 {
        self.history[self.visit].scroll
    }
    fn max_scroll(&self) -> i32 {
        (self.article.height - self.layout_now().article.h).max(0)
    }
    fn scroll_to(&mut self, value: i32) {
        self.history[self.visit].scroll = value.clamp(0, self.max_scroll());
    }
    pub fn print_text(&self) -> String {
        self.catalog.topics[self.topic()].plain_text()
    }

    pub fn print_uses_chooser(&self) -> bool {
        self.nav && self.nav_focused && self.tab == Tab::Contents
    }
    pub fn last_find_text(&self) -> &str {
        &self.last_find_text
    }
    pub fn remember_find_text(&mut self, text: String) {
        self.last_find_text = text;
    }

    pub fn print_has_children(&self) -> bool {
        self.print_uses_chooser() && self.selected.and_then(|i| self.tree.get(i)).is_some_and(|&i| self.has_children(i))
    }

    pub fn print_selection(&self, include_subtopics: bool) -> String {
        if !self.print_uses_chooser() {
            return self.print_text();
        }
        let Some(&first) = self.selected.and_then(|row| self.tree.get(row)) else {
            return self.print_text();
        };
        let entries = &self.catalog.contents;
        if !include_subtopics || !self.has_children(first) {
            // Windows 98 offers the current page; Windows 2000 offers the selected topic.
            let topic = if self.era == Era::Win2000 { entries[first].topic } else { None };
            return self.catalog.topics[topic.unwrap_or(self.topic())].plain_text();
        }
        let mut topics = Vec::new();
        for (i, e) in entries.iter().enumerate().skip(first) {
            if i != first && e.depth <= entries[first].depth {
                break;
            }
            if let Some(topic) = e.topic && !topics.contains(&topic) {
                topics.push(topic);
            }
        }
        topics.iter().map(|&i| self.catalog.topics[i].plain_text()).collect::<Vec<_>>().join("\n\n")
    }

    pub fn find_next(&mut self, text: &str, opts: commands::FindOptions) -> bool {
        let matches = self.article.matches(text, opts.match_case, opts.whole_word);
        if matches.is_empty() {
            return false;
        }
        let found = if opts.down {
            let start = self.selection.as_ref().map_or(0, |r| r.end);
            matches.iter().find(|r| r.start >= start)
        } else {
            let end = self.selection.as_ref().map_or(self.article.text().len(), |r| r.start);
            matches.iter().rfind(|r| r.end <= end)
        };
        let Some(found) = found else {
            return false;
        };
        self.selection = Some(found.clone());
        if let Some(r) = self.article.range_rect(found) {
            if r.y < self.scroll() {
                self.scroll_to(r.y);
            } else if r.bottom() > self.scroll() + self.layout_now().article.h {
                self.scroll_to(r.bottom() - self.layout_now().article.h);
            }
        }
        true
    }

    pub fn copy_text(&self) -> String {
        if self.has_caret() {
            self.query_selection().and_then(|r| self.query.get(r)).unwrap_or("").into()
        } else {
            self.selection.clone().and_then(|r| self.article.text().get(r)).unwrap_or("").into()
        }
    }

    /// The query field takes in the query and the selection as they are now (brief 99).
    fn pull(&mut self) {
        if self.edit.text != self.query {
            self.edit.set(&self.query);
        }
        let chars = |b: usize| self.query[..b.min(self.query.len())].chars().count();
        self.edit.caret = chars(self.caret);
        self.edit.anchor = chars(self.anchor.unwrap_or(self.caret));
    }

    /// What the query field holds is shown; a changed query finds afresh (`edit_complete`).
    fn push(&mut self) {
        let changed = self.query != self.edit.text;
        self.query = self.edit.text.clone();
        let bytes = |c: usize| self.query.char_indices().nth(c).map_or(self.query.len(), |(i, _)| i);
        self.caret = bytes(self.edit.caret);
        let anchor = bytes(self.edit.anchor);
        self.anchor = (anchor != self.caret).then_some(anchor);
        self.blink = true;
        if changed {
            self.edit_complete();
        }
    }

    fn query_selection(&self) -> Option<Range<usize>> {
        let anchor = self.anchor.filter(|&p| p != self.caret)?;
        Some(anchor.min(self.caret)..anchor.max(self.caret))
    }

    fn remove_selected_query(&mut self) -> bool {
        let range = self.query_selection();
        self.anchor = None;
        if let Some(range) = range {
            self.caret = range.start;
            self.query.replace_range(range, "");
            true
        } else {
            false
        }
    }

    fn select_all(&mut self) {
        if self.has_caret() {
            self.anchor = Some(0);
            self.caret = self.query.len();
        } else {
            self.focus = Part::Control(Hit::Article);
            self.selection = Some(0..self.article.text().len());
        }
    }

    fn update_highlights(&mut self) {
        self.matches = self.history[self.visit]
            .search_terms
            .iter()
            .flat_map(|term| self.article.matches(term, false, true))
            .collect();
    }

    fn article_point(&self, x: i32, y: i32) -> usize {
        let r = self.layout_now().article;
        self.article.position_at(x - r.x, y - r.y + self.scroll())
    }

    fn select_word(&mut self, x: i32, y: i32) {
        let text = self.article.text();
        let point = self.article_point(x, y);
        let word = |ch: char| ch.is_alphanumeric() || ch == '_';
        let start = text[..point].trim_end_matches(word).len();
        let end = text.len() - text[point..].trim_start_matches(word).len();
        self.selection = (end > start).then_some(start..end);
        self.tab_focus(Part::Control(Hit::Article));
    }

    fn query_point(&self, x: i32) -> usize {
        self.query_positions.iter().min_by_key(|(_, pos)| x.abs_diff(*pos)).map_or(0, |&(i, _)| i)
    }

    /// A topic can be opened directly by the caller, by a link, or by any navigation tab.
    pub fn show_topic(&mut self, topic: usize) -> bool {
        if topic >= self.catalog.topics.len() {
            return false;
        }
        self.popup = None;
        self.selection = None;
        self.matches.clear();
        if matches!(self.focus, Part::Link(_)) {
            self.focus = Part::Control(Hit::Article);
        }
        if !self.visited.contains(&topic) {
            self.visited.push(topic);
        }
        if self.topic() != topic {
            self.history.truncate(self.visit + 1);
            self.history.push(Visit { topic, scroll: 0, search_terms: Vec::new() });
            self.visit += 1;
        } else {
            self.scroll_to(0);
            self.history[self.visit].search_terms.clear();
        }
        if self.tab == Tab::Contents {
            self.sync_tree_selection();
        }
        true
    }

    fn rebuild_tree(&mut self) {
        self.tree.clear();
        let mut hidden_below = usize::MAX;
        for (i, entry) in self.catalog.contents.iter().enumerate() {
            if entry.depth >= hidden_below {
                continue;
            }
            self.tree.push(i);
            hidden_below = if self.expanded[i] { usize::MAX } else { entry.depth + 1 };
        }
    }

    fn has_children(&self, entry: usize) -> bool {
        self.catalog.contents.get(entry + 1).is_some_and(|e| e.depth > self.catalog.contents[entry].depth)
    }

    fn sync_tree_selection(&mut self) {
        let topic = self.topic();
        let entries = &self.catalog.contents;
        if let Some(target) = entries.iter().position(|e| e.topic == Some(topic)) {
            let mut depth = entries[target].depth;
            for i in (0..target).rev() {
                if entries[i].depth < depth {
                    self.expanded[i] = true;
                    depth = entries[i].depth;
                    if depth == 0 {
                        break;
                    }
                }
            }
            self.rebuild_tree();
            self.selected = self.tree.iter().position(|&i| i == target);
        } else {
            self.selected = (!self.tree.is_empty()).then_some(0);
        }
        self.keep_selected_visible();
    }

    fn count(&self) -> usize {
        match self.tab {
            Tab::Contents => self.tree.len(),
            Tab::Index => self.index.len(),
            Tab::Search => self.results.len(),
        }
    }

    fn keep_selected_visible(&mut self) {
        let layout = self.layout_now();
        let visible = layout.visible_rows();
        self.first = self.first.min(self.count().saturating_sub(visible));
        if let Some(selected) = self.selected {
            self.first = self.first.min(selected).max((selected + 1).saturating_sub(visible));
            if matches!(self.focus, Part::Control(Hit::Row(_))) {
                self.focus = Part::Control(Hit::Row(selected));
            }
        }
    }

    fn choose_tab(&mut self, tab: Tab) {
        self.query_by_tab[self.tab as usize] = std::mem::take(&mut self.query);
        self.tab = tab;
        self.nav_focused = true;
        self.first = 0;
        self.query = std::mem::take(&mut self.query_by_tab[tab as usize]);
        self.caret = self.query.len();
        self.anchor = None;
        self.blink = true;
        self.selected = (self.count() != 0).then_some(0);
        self.focus = match tab {
            Tab::Contents => Part::Control(Hit::Row(0)),
            _ => Part::Control(Hit::Query),
        };
        if tab == Tab::Contents {
            // The opening book may intentionally display a welcome page not in the tree.
            if self.visit != 0 {
                self.sync_tree_selection();
            }
        } else if tab == Tab::Index {
            self.edit_complete();
        }
    }

    pub fn show_tab(&mut self, tab: Tab) {
        if !self.nav {
            self.activate(Part::Control(Hit::NavToggle));
        }
        self.choose_tab(tab);
    }

    fn navigate_history(&mut self, forward: bool) {
        let next = if forward { (self.visit + 1).min(self.history.len() - 1) } else { self.visit.saturating_sub(1) };
        if self.visit != next {
            self.visit = next;
            self.selection = None;
            if matches!(self.focus, Part::Link(_)) {
                self.focus = Part::Control(Hit::Article);
            }
            if self.tab == Tab::Contents {
                self.sync_tree_selection();
            }
        }
    }

    fn select(&mut self, row: usize, display: bool) {
        if row >= self.count() {
            return;
        }
        self.selected = Some(row);
        self.nav_focused = true;
        self.focus = Part::Control(Hit::Row(row));
        self.keep_selected_visible();
        match self.tab {
            Tab::Contents => {
                let entry = self.tree[row];
                if let Some(topic) = self.catalog.contents[entry].topic {
                    self.show_topic(topic);
                    // Two different contents entries may point at the same article.
                    self.selected = self.tree.iter().position(|&i| i == entry);
                } else if display {
                    self.toggle_entry(entry);
                }
            }
            Tab::Index if display => {
                if let [topic] = self.index[row].topics.as_slice() {
                    self.show_topic(*topic);
                }
            }
            Tab::Search if display => {
                self.show_topic(self.results[row]);
                self.history[self.visit].search_terms.clone_from(&self.search_terms);
                if self.page_topic == self.topic() {
                    self.update_highlights();
                }
            }
            _ => {}
        }
    }

    fn toggle_entry(&mut self, entry: usize) {
        if self.has_children(entry) {
            self.expanded[entry] = !self.expanded[entry];
            self.rebuild_tree();
            self.selected = self.tree.iter().position(|&i| i == entry);
            self.keep_selected_visible();
        }
    }

    fn edit_complete(&mut self) {
        self.blink = true;
        if self.tab == Tab::Index {
            let query = self.query.to_lowercase();
            self.selected = self
                .index
                .iter()
                .position(|i| i.label.to_lowercase().as_str() >= query.as_str())
                .or_else(|| self.index.len().checked_sub(1));
            self.keep_selected_visible();
        }
    }

    fn search(&mut self) {
        let words: Vec<_> =
            self.query.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
        if words.is_empty() {
            return;
        }
        self.results.clear();
        for (i, topic) in self.catalog.topics[..self.web].iter().enumerate() {
            let mut text = topic.title.to_lowercase();
            text.push(' ');
            text.push_str(&topic.plain_text().to_lowercase());
            let tokens: Vec<_> = text.split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()).collect();
            if words.iter().all(|w| tokens.iter().any(|t| singular(w) == singular(t))) {
                self.results.push(i);
            }
        }
        self.results.sort_by_cached_key(|&i| self.catalog.topics[i].title.to_lowercase());
        self.search_terms.clear();
        for w in words {
            let word = singular(&w);
            self.search_terms.push(word.to_owned());
            self.search_terms.push(format!("{word}s"));
        }
        self.history[self.visit].search_terms.clear();
        self.matches.clear();
        self.first = 0;
        self.selected = (!self.results.is_empty()).then_some(0);
    }

    fn menu_at(&mut self, items: Vec<(MenuEntry, Option<Action>)>, anchor: Rect) {
        self.popup = Some(Popup {
            items,
            anchor: (anchor.x, anchor.bottom()),
            layout: None,
            hover: None,
            options: false,
            keys: self.era == Era::Win98 || self.keyboard_input,
        });
    }

    fn select_topics(&mut self, topics: &[usize], at: Rect) {
        if let [topic] = topics {
            self.show_topic(*topic);
            return;
        }
        let items: Vec<_> = topics
            .iter()
            .filter_map(|&i| {
                let title = &self.catalog.topics.get(i)?.title;
                Some((MenuEntry::item(&title.replace('&', "&&"), None), Some(Action::Topic(i))))
            })
            .collect();
        if !items.is_empty() {
            self.menu_at(items, at);
        }
    }

    pub fn keyword_topics(&self) -> Vec<(usize, String)> {
        if self.tab != Tab::Index {
            return Vec::new();
        }
        self.selected
            .and_then(|row| self.index.get(row))
            .into_iter()
            .flat_map(|entry| &entry.topics)
            .map(|&id| (id, self.catalog.topics[id].title.clone()))
            .collect()
    }

    pub fn topics_anchor(&self) -> (Rect, bool) {
        let layout = self.layout_now();
        let anchor = match self.focus {
            Part::Control(Hit::Row(_)) => layout.list,
            Part::Control(Hit::Query) => layout.query,
            _ => layout.display,
        };
        (anchor, self.keyboard_input)
    }

    fn display_part(&self) -> Part {
        if self.tab == Tab::Index
            && self.selected.and_then(|row| self.index.get(row)).is_some_and(|entry| entry.topics.len() > 1)
        {
            Part::TopicsFound
        } else {
            Part::Control(Hit::Display)
        }
    }

    fn options(&mut self, at: Rect) {
        let command = |label: &str, action: Action, enabled: bool| {
            let mut entry = MenuEntry::item(label, None);
            entry.disabled = !enabled;
            (entry, Some(action))
        };
        let mut items = vec![
            command(
                if self.nav { "Hide &Tabs" } else { "Show &Tabs" },
                Action::Part(Part::Control(Hit::NavToggle)),
                true,
            ),
            command("&Back", Action::Part(Part::Control(Hit::Back)), self.era == Era::Win98 || self.visit > 0),
            command(
                "&Forward",
                Action::Part(Part::Control(Hit::Forward)),
                self.era == Era::Win98 || self.visit + 1 < self.history.len(),
            ),
        ];
        if self.era == Era::Win2000 {
            items.push(command("&Home", Action::Home, true));
        }
        items.extend([command("&Stop", Action::Refresh, true), command("&Refresh", Action::Refresh, true)]);
        if self.era == Era::Win2000 {
            items.push(command("&Internet Options...", Action::Part(Part::InternetOptions), true));
        }
        items.extend([
            (MenuEntry::separator(), None),
            command(if self.era == Era::Win2000 { "&2 Web Help" } else { "&Web Help" }, Action::Topic(self.web), true),
            (MenuEntry::separator(), None),
        ]);
        if self.era == Era::Win98 {
            items.push(command("&Customize...", Action::Refresh, false));
        }
        items.push(command("&Print...", Action::Part(Part::Print), true));
        let highlight_label = match (self.era, self.highlight) {
            (Era::Win2000, true) => "Search Highlight &Off",
            (Era::Win2000, false) => "Search Highlight &On",
            (_, true) => "Highlighting &Off",
            (_, false) => "Highlighting &On",
        };
        items.push(command(highlight_label, Action::Highlight, true));
        self.menu_at(items, at);
        if let Some(popup) = &mut self.popup {
            popup.options = true;
            popup.anchor.1 = if self.era == Era::Win98 { 59 } else { 63 };
        }
    }

    fn link_on_screen(&self, paragraph: usize) -> Option<Rect> {
        let page = self.layout_now().article;
        let r = self.article.link_rect(paragraph)?;
        Some(Rect::new(page.x + r.x, page.y + r.y - self.scroll(), r.w, r.h))
    }

    fn tab_focus(&mut self, part: Part) {
        self.focus = part;
        if matches!(part, Part::Control(Hit::Row(_) | Hit::Query | Hit::Tab(_) | Hit::Display | Hit::ListTopics)) {
            self.nav_focused = true;
        } else if matches!(part, Part::Control(Hit::Article) | Part::Link(_)) {
            self.nav_focused = false;
        }
        if let Part::Link(i) = part {
            let page = self.layout_now().article;
            if let Some(link) = self.link_on_screen(i) {
                if link.y < page.y {
                    self.scroll_to(self.scroll() + link.y - page.y);
                } else if link.bottom() > page.bottom() {
                    self.scroll_to(self.scroll() + link.bottom() - page.bottom());
                }
            }
        }
    }

    pub fn activate(&mut self, part: Part) {
        match part {
            Part::Link(i) => {
                let Some(paragraph) = self.catalog.topics[self.topic()].paragraphs.get(i) else { return };
                match &paragraph.style {
                    Style::Link(target) => {
                        self.show_topic(*target);
                    }
                    Style::Related(topics) => {
                        let topics = topics.clone();
                        if let Some(r) = self.link_on_screen(i) {
                            self.select_topics(&topics, r);
                        }
                    }
                    _ => {}
                }
            }
            Part::MenuItem(i) => {
                let item = self.popup.take().and_then(|p| p.items.get(i).cloned());
                match item {
                    Some((entry, action)) if !entry.disabled => match action {
                        Some(Action::Topic(topic)) => {
                            self.show_topic(topic);
                        }
                        Some(Action::Part(part)) => self.activate(part),
                        Some(Action::Home) => {
                            self.show_topic(self.catalog.home);
                        }
                        Some(Action::Refresh) => {
                            self.page_width = -1;
                        }
                        Some(Action::Highlight) => self.highlight = !self.highlight,
                        Some(Action::SelectAll) => self.select_all(),
                        None => {}
                    },
                    _ => {}
                }
            }
            Part::Control(hit) => match hit {
                Hit::NavToggle => {
                    let layout = self.layout_now();
                    if !self.maximized {
                        let split = if self.nav { layout.splitter.x } else { self.split };
                        let offset = split - if self.era == Era::Win98 { 4 } else { 0 };
                        self.size.0 += if self.nav { -offset } else { offset };
                    }
                    self.nav = !self.nav;
                    self.restored_navigation = self.nav && !self.maximized;
                    self.focus = Part::Control(Hit::NavToggle);
                }
                Hit::Back | Hit::Forward => self.navigate_history(hit == Hit::Forward),
                Hit::WebHelp => {
                    self.show_topic(self.web);
                }
                Hit::Tab(tab) => self.choose_tab(tab),
                Hit::Row(i) => self.select(i, false),
                Hit::Display => {
                    if let Some(i) = self.selected {
                        self.select(i, true);
                    }
                }
                Hit::ListTopics => self.search(),
                Hit::NavUp => self.first = self.first.saturating_sub(1),
                Hit::NavDown => self.first = self.first.saturating_add(1),
                // Windows' tree view moves 5 pixels a click (its MAGIC_HORZLINE).
                Hit::NavLeft => self.scroll_tree_to(self.tree_left - 5),
                Hit::NavRight => self.scroll_tree_to(self.tree_left + 5),
                Hit::ScrollUp => self.scroll_to(self.scroll() - 16),
                Hit::ScrollDown => self.scroll_to(self.scroll() + 16),
                Hit::Options => {
                    if let Some(r) = self.rect(part) {
                        self.options(r);
                    }
                }
                _ => {}
            },
            Part::Dismiss | Part::Print | Part::Find | Part::Copy => self.popup = None,
            _ => {}
        }
    }

    fn control(part: Part) -> Option<Hit> {
        match part {
            Part::Close => Some(Hit::Close),
            Part::Minimize => Some(Hit::Minimize),
            Part::Maximize => Some(Hit::Maximize),
            Part::WindowMenu => Some(Hit::Icon),
            Part::Control(hit) => Some(hit),
            _ => None,
        }
    }

    fn part(hit: Hit) -> Part {
        match hit {
            Hit::Close => Part::Close,
            Hit::Minimize => Part::Minimize,
            Hit::Maximize => Part::Maximize,
            Hit::Icon => Part::WindowMenu,
            other => Part::Control(other),
        }
    }

    fn invoke(&mut self, part: Part) -> Vec<Effect<Part>> {
        let part = if part == Part::Control(Hit::Display) { self.display_part() } else { part };
        if let Part::MenuItem(i) = part
            && let Some((entry, action)) = self.popup.as_ref().and_then(|p| p.items.get(i))
        {
            if entry.disabled {
                return Vec::new();
            }
            if let Some(Action::Part(command)) = *action {
                self.popup = None;
                return vec![Effect::Activate(command)];
            }
        }
        if self.enabled(part) { vec![Effect::Activate(part)] } else { Vec::new() }
    }

    fn move_caret(&mut self, left: bool) {
        self.caret = if left {
            self.query[..self.caret].char_indices().next_back().map_or(0, |(i, _)| i)
        } else {
            self.caret + self.query[self.caret..].chars().next().map_or(0, char::len_utf8)
        };
        self.blink = true;
    }

    fn select_relative(&mut self, delta: isize) {
        if self.count() == 0 {
            return;
        }
        let row = self.selected.unwrap_or(0).saturating_add_signed(delta).min(self.count() - 1);
        self.select(row, false);
    }

    fn tree_arrow(&mut self, right: bool) {
        let Some(row) = self.selected else { return };
        let Some(&entry) = self.tree.get(row) else { return };
        let depth = self.catalog.contents[entry].depth;
        if self.has_children(entry) && self.expanded[entry] != right {
            self.toggle_entry(entry);
        } else if right && self.has_children(entry) {
            self.select_relative(1);
        } else if !right && depth > 0
            && let Some(parent) = self.tree[..row].iter().rposition(|&i| self.catalog.contents[i].depth < depth)
        {
            self.select(parent, false);
        }
    }
}

impl Form for HtmlHelp {
    type Part = Part;

    fn title(&self) -> &str {
        &self.catalog.title
    }
    fn size(&self) -> (i32, i32) {
        self.size
    }

    fn layout(&mut self, fonts: &Fonts) {
        let layout = self.layout_now();
        let topic = self.topic();
        if self.page_width != layout.article.w || self.page_topic != topic {
            let compose = if topic == self.web { article::layout_web } else { article::layout };
            self.article = compose(self.era, &self.catalog.topics[topic], layout.article.w);
            self.page_topic = topic;
            self.page_width = layout.article.w;
            self.update_highlights();
        }
        self.scroll_to(self.scroll());
        let visible = layout.visible_rows();
        self.first = self.first.min(self.count().saturating_sub(visible));
        self.query_positions.clear();
        let font = chrome::control_font(fonts, self.era);
        let prefix_width = font.measure(&self.query[..self.caret]);
        let inset = if self.era == Era::Win98 { 3 } else { 1 };
        let mut x = layout.query.x + 2 + (layout.query.w - 5 - prefix_width).min(inset);
        for (i, c) in self.query.char_indices() {
            self.query_positions.push((i, x));
            x += chrome::control_font(fonts, self.era).glyph(c).advance;
        }
        self.query_positions.push((self.query.len(), x));
        if let Some(popup) = &mut self.popup {
            let menu = popup.menu();
            let menu_layout = paint::layout_menu(theme(self.era), fonts, &menu, MenuStyle::Popup, self.size.1);
            let x = popup.anchor.0.min(self.size.0 - menu_layout.width).max(0);
            let y = popup.anchor.1.min(self.size.1 - menu_layout.height).max(0);
            popup.layout = Some((Rect::new(x, y, menu_layout.width, menu_layout.height), menu_layout));
        }
    }

    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let layout = self.layout_now();
        let rows = match self.tab {
            Tab::Contents => self
                .tree
                .iter()
                .map(|&i| {
                    let e = &self.catalog.contents[i];
                    Row {
                        label: &e.label,
                        depth: e.depth,
                        icon: e.icon,
                        open: self.has_children(i) && self.expanded[i],
                    }
                })
                .collect::<Vec<_>>(),
            Tab::Index => self
                .index
                .iter()
                .map(|e| Row { label: &e.label, depth: 0, icon: EntryIcon::Page, open: false })
                .collect(),
            Tab::Search => self
                .results
                .iter()
                .map(|&i| Row { label: &self.catalog.topics[i].title, depth: 0, icon: EntryIcon::Page, open: false })
                .collect(),
        };
        let view = Chrome {
            title: self.title(),
            active: self.active,
            maximized: self.maximized,
            keys: self.keys,
            back: self.visit > 0,
            forward: self.visit + 1 < self.history.len(),
            pressed: self.pressed.and_then(Self::control),
            hover: if self.popup.as_ref().is_some_and(|p| p.options) {
                Some(Hit::Options)
            } else {
                self.hover.and_then(Self::control)
            },
            focus: Self::control(self.focus),
            rows: &rows,
            first: self.first,
            selected: self.selected,
            query: &self.query,
            caret: self.caret,
            blink: self.blink,
            selection: self.query_selection().map(|r| (r.start, r.end)),
            scroll: self.scroll(),
            document_height: self.article.height,
        };
        chrome::paint(fb, theme, fonts, &layout, &view);
        let link = |part| match part {
            Part::Link(i) => Some(i),
            _ => None,
        };
        self.article.paint(
            fb,
            layout.article,
            self.scroll(),
            &article::PaintState {
                hover: self.hover.and_then(link),
                active: self.pressed.and_then(link),
                visited: &self.visited,
                focus: link(self.focus),
                selection: self.selection.clone(),
                highlights: if self.highlight { &self.matches } else { &[] },
            },
        );
        if theme.depth.reduced() {
            // The page as Internet Explorer showed it on a 16-color screen: its colors and pictures
            // (none with a 16-color drawing) each pixel by the look's rule. (Explorer dithered the
            // pictures; PROVISIONAL.)
            let a = layout.article;
            for y in a.y.max(0)..a.bottom().min(fb.height) {
                for x in a.x.max(0)..a.right().min(fb.width) {
                    let p = fb.get(x, y);
                    fb.set(x, y, crate::colors16::convert(theme.era, p));
                }
            }
        }
        if let Some(popup) = &self.popup && let Some((r, layout)) = &popup.layout {
            let mut menu = Framebuffer::new(r.w, r.h, theme.colors.face);
            paint::paint_menu(&mut menu, theme, fonts, &popup.menu(), MenuStyle::Popup, layout);
            fb.blit(&menu, r.x, r.y);
        }
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        if let Some(popup) = &self.popup {
            if let Some((r, menu)) = &popup.layout
                && let Some(i) = menu.rects.iter().position(|item| item.contains(x - r.x, y - r.y))
            {
                return Some(Part::MenuItem(i));
            }
            return Some(Part::Dismiss);
        }
        let layout = self.layout_now();
        if layout.article.contains(x, y)
            && let Some(i) = self.article.link_at(x - layout.article.x, y - layout.article.y + self.scroll())
        {
            return Some(Part::Link(i));
        }
        layout.hit(x, y, self.first, self.count(), self.scroll(), self.article.height).map(Self::part)
    }

    fn rect(&self, part: Part) -> Option<Rect> {
        match part {
            Part::Link(i) => self.link_on_screen(i),
            Part::MenuItem(i) => {
                let (r, layout) = self.popup.as_ref()?.layout.as_ref()?;
                let item = layout.rects.get(i)?;
                Some(Rect::new(r.x + item.x, r.y + item.y, item.w, item.h))
            }
            _ => self.layout_now().rect(
                Self::control(part)?,
                self.first,
                self.count(),
                self.scroll(),
                self.article.height,
            ),
        }
    }

    fn enabled(&self, part: Part) -> bool {
        match part {
            Part::Control(Hit::Back) => self.visit > 0,
            Part::Control(Hit::Forward) => self.visit + 1 < self.history.len(),
            Part::Control(Hit::Display) => self.selected.is_some(),
            Part::Control(Hit::ListTopics) => !self.query.trim().is_empty(),
            Part::MenuItem(i) => {
                self.popup.as_ref().and_then(|p| p.items.get(i)).is_some_and(|(e, a)| a.is_some() && !e.disabled)
            }
            _ => true,
        }
    }

    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        self.tab_focus(part);
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }
    fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.popup = None;
        }
    }
    fn show_keys(&mut self) {
        self.keys = true;
    }
    fn has_caret(&self) -> bool {
        self.nav && self.focus == Part::Control(Hit::Query)
    }
    fn blink(&mut self, on: bool) {
        self.blink = on;
    }
    fn tab_stops(&self) -> Vec<Part> {
        let mut stops = Vec::new();
        if self.nav {
            stops.push(Part::Control(Hit::Tab(self.tab)));
            if self.tab != Tab::Contents {
                stops.push(Part::Control(Hit::Query));
            }
            if self.tab == Tab::Search {
                stops.push(Part::Control(Hit::ListTopics));
            }
            if let Some(row) = self.selected {
                stops.push(Part::Control(Hit::Row(row)));
            }
            if self.tab != Tab::Contents {
                stops.push(Part::Control(Hit::Display));
            }
        }
        stops.push(Part::Control(Hit::Article));
        for (i, p) in self.catalog.topics[self.topic()].paragraphs.iter().enumerate() {
            if matches!(p.style, Style::Link(_) | Style::Related(_)) {
                stops.push(Part::Link(i));
            }
        }
        stops
    }

    fn enter(&self) -> Part {
        match self.focus {
            Part::Control(Hit::Query) if self.tab == Tab::Search => Part::Control(Hit::ListTopics),
            Part::Control(Hit::Query | Hit::Row(_) | Hit::Display) => self.display_part(),
            part => part,
        }
    }
    fn escape(&self) -> Part {
        if self.popup.is_some() { Part::Dismiss } else { Part::Close }
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        self.keyboard_input = true;
        if self.popup.is_some() {
            return self.typed(ch);
        }
        let old = self.era == Era::Win98;
        match ch.to_ascii_lowercase() {
            'o' => return self.invoke(Part::Control(Hit::Options)),
            'c' if self.nav => self.choose_tab(Tab::Contents),
            ch if self.nav && ch == if old { 'i' } else { 'n' } => self.choose_tab(Tab::Index),
            's' if self.nav => self.choose_tab(Tab::Search),
            'd' if self.nav && self.tab != Tab::Contents => return self.invoke(Part::Control(Hit::Display)),
            'l' if self.nav && self.tab == Tab::Search => return self.invoke(Part::Control(Hit::ListTopics)),
            ch if self.nav && self.tab != Tab::Contents && ch == if old { 'k' } else { 'w' } => {
                self.tab_focus(Part::Control(Hit::Query));
            }
            ch if self.nav && self.tab == Tab::Search && ch == if old { 'p' } else { 't' } => {
                if let Some(i) = self.selected {
                    self.tab_focus(Part::Control(Hit::Row(i)));
                }
            }
            'b' => self.navigate_history(false),
            _ => {}
        }
        Vec::new()
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        if self.has_caret() { Vec::new() } else { self.invoke(self.enter()) }
    }

    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        self.keyboard_input = true;
        if let Some(popup) = &mut self.popup {
            let n = popup.items.len();
            match key {
                Key::Escape => self.popup = None,
                Key::Up | Key::Down if n > 0 => {
                    let step = if key == Key::Down { 1 } else { n - 1 };
                    let mut i = popup.hover.unwrap_or(if key == Key::Down { n - 1 } else { 0 });
                    for _ in 0..n {
                        i = (i + step) % n;
                        if popup.items[i].1.is_some() && !popup.items[i].0.disabled {
                            popup.hover = Some(i);
                            break;
                        }
                    }
                }
                Key::Enter | Key::Space => {
                    if let Some(i) = popup.hover {
                        return Some(self.invoke(Part::MenuItem(i)));
                    }
                }
                _ => return None,
            }
            return Some(Vec::new());
        }
        // The query field: Windows' edit box (caret keys, words, Backspace, Delete, Cut, Copy,
        // Paste, Undo, Ctrl+A, Shift+F10; brief 99). Up and Down still move the list; Ctrl+Tab,
        // Ctrl+F and Ctrl+P stay the window's.
        if self.has_caret()
            && !matches!(key, Key::Up | Key::Down | Key::Tab | Key::PageUp | Key::PageDown)
            && !(mods.ctrl && matches!(key, Key::Other(0x46) | Key::Other(0x50)))
        {
            self.pull();
            let done = kit::field_key(&mut self.edit, key, mods, &mut self.copied)?;
            self.push();
            return Some(done.effects);
        }
        if mods.ctrl {
            match key {
                Key::Tab => {
                    self.show_tab(self.tab.step(mods.shift));
                }
                Key::Other(0x46) => return Some(vec![Effect::Activate(Part::Find)]),
                Key::Other(0x50) => return Some(vec![Effect::Activate(Part::Print)]),
                Key::Other(0x43) => return Some(vec![Effect::Activate(Part::Copy)]),
                Key::Other(0x41) => self.select_all(),
                Key::Other(0x56) if self.has_caret() => return Some(vec![Effect::Paste]),
                _ => return None,
            }
            return Some(Vec::new());
        }
        if self.has_caret() {
            let old = self.caret;
            match key {
                Key::Left | Key::Right | Key::Home | Key::End => {
                    if key == Key::Home {
                        self.caret = 0;
                    } else if key == Key::End {
                        self.caret = self.query.len();
                    } else if !mods.shift && self.query_selection().is_some() {
                        let r = self.query_selection().unwrap();
                        self.caret = if key == Key::Left { r.start } else { r.end };
                    } else {
                        self.move_caret(key == Key::Left);
                    }
                    if mods.shift {
                        self.anchor.get_or_insert(old);
                    } else {
                        self.anchor = None;
                    }
                    self.blink = true;
                }
                Key::Delete => {
                    if !self.remove_selected_query() && self.caret < self.query.len() {
                        self.query.remove(self.caret);
                    }
                    self.edit_complete();
                }
                Key::Up | Key::Down => {
                    self.select_relative(if key == Key::Up { -1 } else { 1 });
                }
                _ => return None,
            }
            return Some(Vec::new());
        }
        if matches!(self.focus, Part::Control(Hit::Tab(_))) && matches!(key, Key::Left | Key::Right) {
            self.choose_tab(self.tab.step(key == Key::Left));
            self.focus = Part::Control(Hit::Tab(self.tab));
            return Some(Vec::new());
        }
        if matches!(self.focus, Part::Control(Hit::Row(_))) {
            let layout = self.layout_now();
            let page = layout.visible_rows() as isize;
            match key {
                Key::Up => self.select_relative(-1),
                Key::Down => self.select_relative(1),
                Key::Home => self.select(0, false),
                Key::End => self.select(self.count().saturating_sub(1), false),
                Key::PageUp => self.select_relative(-page),
                Key::PageDown => self.select_relative(page),
                Key::Left | Key::Right if self.tab == Tab::Contents => self.tree_arrow(key == Key::Right),
                _ => return None,
            }
            return Some(Vec::new());
        }
        if matches!(self.focus, Part::Control(Hit::Article) | Part::Link(_)) {
            let page = self.layout_now().article.h;
            match key {
                Key::Up => self.scroll_to(self.scroll() - 16),
                Key::Down => self.scroll_to(self.scroll() + 16),
                Key::PageUp => self.scroll_to(self.scroll() - page),
                Key::PageDown => self.scroll_to(self.scroll() + page),
                Key::Home => self.scroll_to(0),
                Key::End => self.scroll_to(self.max_scroll()),
                _ => return None,
            }
            return Some(Vec::new());
        }
        None
    }

    fn alt_key(&mut self, key: Key) -> bool {
        match key {
            Key::Left | Key::Right => {
                self.navigate_history(key == Key::Right);
                true
            }
            _ => false,
        }
    }

    fn typed(&mut self, c: char) -> Vec<Effect<Part>> {
        if let Some(popup) = &self.popup {
            let key = c.to_ascii_lowercase();
            let selected = popup.items.iter().position(|(e, a)| {
                a.is_some()
                    && !e.disabled
                    && e.label
                        .split_once('&')
                        .and_then(|(_, label)| label.chars().next())
                        .is_some_and(|ch| ch.to_ascii_lowercase() == key)
            });
            if let Some(i) = selected {
                return self.invoke(Part::MenuItem(i));
            }
        } else if self.has_caret() {
            // Backspace comes as its key (`key`); its character is left out here.
            self.pull();
            if self.edit.type_char(c) {
                self.push();
            }
        }
        Vec::new()
    }
    /// Text from the clipboard: its first line, cut to the field's limit (brief 99; before, every
    /// line was run together).
    fn paste(&mut self, s: &str) {
        if self.has_caret() {
            self.pull();
            if self.edit.paste(s) {
                self.push();
            }
        }
    }
    fn take_copy(&mut self) -> Option<String> {
        self.copied.take()
    }
    fn field_menu(&self, clipboard_has_text: bool) -> Option<crate::paint::Menu> {
        if !self.has_caret() {
            return None;
        }
        let mut edit = self.edit.clone();
        if edit.text != self.query {
            edit.set(&self.query);
        }
        let chars = |b: usize| self.query[..b.min(self.query.len())].chars().count();
        edit.caret = chars(self.caret);
        edit.anchor = chars(self.anchor.unwrap_or(self.caret));
        Some(edit.menu(clipboard_has_text))
    }
    fn field_menu_chosen(&mut self, index: usize) -> Vec<Effect<Part>> {
        self.pull();
        let done = kit::field_menu_chosen(&mut self.edit, index, &mut self.copied);
        self.push();
        done.effects
    }

    fn hover(&mut self, x: i32, y: i32) -> bool {
        let hit = self.hit(x, y);
        let changed = hit != self.hover;
        self.hover = hit;
        if let Some(p) = &mut self.popup {
            p.hover = match hit {
                Some(Part::MenuItem(i)) if p.items[i].1.is_some() => Some(i),
                _ => None,
            };
        }
        changed
    }
    fn pointer_left(&mut self) -> bool {
        self.hover.take().is_some()
    }

    fn press(&mut self, part: Option<Part>, x: i32, y: i32) -> Press<Part> {
        self.keyboard_input = false;
        let Some(part) = part else { return Press::Nothing };
        if !self.enabled(part) {
            return Press::Done;
        }
        match part {
            Part::Dismiss => {
                self.popup = None;
                Press::Done
            }
            Part::Control(Hit::Query) => {
                self.tab_focus(part);
                self.caret = self.query_point(x);
                self.anchor = Some(self.caret);
                self.blink = true;
                Press::Grab(part, 0)
            }
            Part::Control(Hit::Splitter) => Press::Grab(part, self.layout_now().splitter.x - x),
            Part::Control(Hit::ScrollThumb) => Press::Grab(part, self.scroll()),
            Part::Control(Hit::NavThumb) => Press::Grab(part, self.first as i32),
            Part::Control(Hit::NavHThumb) => {
                self.tree_drag_from = self.layout_now().tree_left;
                Press::Grab(part, x)
            }
            Part::Control(Hit::NavHTrack) => {
                let layout = self.layout_now();
                if let Some(thumb) = self.rect(Part::Control(Hit::NavHThumb)) {
                    let page = layout.tree_page();
                    self.scroll_tree_to(layout.tree_left + if x < thumb.x { -page } else { page });
                }
                Press::Done
            }
            Part::Control(Hit::NavTrack) => {
                let layout = self.layout_now();
                if let Some(thumb) = self.rect(Part::Control(Hit::NavThumb)) {
                    let rows = layout.visible_rows();
                    self.first =
                        if y < thumb.y { self.first.saturating_sub(rows) } else { self.first.saturating_add(rows) };
                }
                Press::Done
            }
            Part::Control(Hit::ScrollTrack) => {
                let r = self.rect(Part::Control(Hit::ScrollThumb));
                let page = self.layout_now().article.h;
                if let Some(thumb) = r {
                    self.scroll_to(self.scroll() + if y < thumb.y { -page } else { page });
                }
                Press::Done
            }
            Part::Control(Hit::Article) | Part::Link(_) => {
                self.tab_focus(part);
                if matches!(part, Part::Link(_)) {
                    self.pressed = Some(part);
                }
                self.selection_start = self.article_point(x, y);
                self.selection = None;
                Press::Grab(part, 0)
            }
            Part::Control(Hit::Row(i)) => {
                self.select(i, false);
                Press::Done
            }
            Part::MenuItem(_) => Press::Hold(part),
            _ => {
                self.tab_focus(part);
                Press::Hold(part)
            }
        }
    }
    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        self.invoke(part)
    }

    fn drag(&mut self, part: Part, grab_y: i32, value: i32, x: i32, y: i32) -> bool {
        match part {
            Part::Control(Hit::Splitter) => self.split = (x + value).clamp(132, (self.size.0 - 75).max(132)),
            Part::Control(Hit::ScrollThumb) => {
                let r = self.layout_now().scrollbar;
                let thumb = self.rect(part).unwrap_or_default();
                let track = (r.h - 32 - thumb.h).max(1);
                let delta = (i64::from(y - grab_y) * i64::from(self.max_scroll()) / i64::from(track)) as i32;
                self.scroll_to(value + delta);
            }
            Part::Control(Hit::NavThumb) => {
                let layout = self.layout_now();
                let visible = layout.visible_rows();
                let max = self.count().saturating_sub(visible);
                let thumb = self.rect(part).unwrap_or_default();
                let track = (self.rect(Part::Control(Hit::NavTrack)).unwrap_or_default().h - thumb.h).max(1);
                let delta = i64::from(y - grab_y) * max as i64 / i64::from(track);
                self.first = (i64::from(value) + delta).clamp(0, max as i64) as usize;
            }
            Part::Control(Hit::NavHThumb) => {
                let max = self.layout_now().tree_max_left();
                let thumb = self.rect(part).unwrap_or_default();
                let track = (self.rect(Part::Control(Hit::NavHTrack)).unwrap_or_default().w - thumb.w).max(1);
                let delta = (i64::from(x - value) * i64::from(max) / i64::from(track)) as i32;
                self.scroll_tree_to(self.tree_drag_from + delta);
            }
            Part::Control(Hit::Query) => {
                self.caret = self.query_point(x);
                self.blink = true;
            }
            Part::Control(Hit::Article) | Part::Link(_) => {
                let r = self.layout_now().article;
                if y < r.y {
                    self.scroll_to(self.scroll() - 16);
                } else if y >= r.bottom() {
                    self.scroll_to(self.scroll() + 16);
                }
                let end = self.article_point(x, y);
                self.selection =
                    (end != self.selection_start).then(|| end.min(self.selection_start)..end.max(self.selection_start));
            }
            _ => return false,
        }
        true
    }
    fn dropped(&mut self, part: Part, x: i32, y: i32) -> Vec<Effect<Part>> {
        if matches!(part, Part::Link(_)) && self.selection.is_none() && self.hit(x, y) == Some(part) {
            self.invoke(part)
        } else {
            Vec::new()
        }
    }
    fn double_click(&mut self, x: i32, y: i32) -> Option<Vec<Effect<Part>>> {
        match self.hit(x, y) {
            // In the query field: a word selected (brief 99).
            Some(Part::Control(Hit::Query)) if self.has_caret() => {
                self.pull();
                let at = self.query[..self.query_point(x)].chars().count();
                self.edit.double_click(at);
                self.push();
                Some(Vec::new())
            }
            Some(Part::Control(Hit::Article)) => {
                self.select_word(x, y);
                Some(Vec::new())
            }
            Some(Part::Control(Hit::Row(i))) => {
                if self.tab == Tab::Contents {
                    if let Some(&entry) = self.tree.get(i) {
                        self.toggle_entry(entry);
                    }
                } else {
                    self.select(i, false);
                    return Some(self.invoke(Part::Control(Hit::Display)));
                }
                Some(Vec::new())
            }
            Some(Part::WindowMenu) => Some(vec![Effect::Activate(Part::Close)]),
            None if crate::kit::TITLE_ROWS.contains(&y) => Some(vec![Effect::Activate(Part::Maximize)]),
            _ => None,
        }
    }
    fn field_menu_at(&self) -> Option<(i32, i32)> {
        crate::edit::middle(self.layout_now().query)
    }
    fn button(&mut self, button: crate::kit::Button, down: bool, x: i32, y: i32) -> Vec<Effect<Part>> {
        self.keyboard_input = false;
        // The query field's right button: its edit menu (by the window's own popup menus,
        // `winkit::menu`, as every field's) as it comes up (brief 99).
        if button == crate::kit::Button::Right && self.hit(x, y) == Some(Part::Control(Hit::Query)) {
            if !self.has_caret() {
                self.tab_focus(Part::Control(Hit::Query));
            }
            return if down { Vec::new() } else { vec![Effect::FieldMenu] };
        }
        if button == crate::kit::Button::Right && !down && self.layout_now().article.contains(x, y) {
            let command = |label: &str, action| (MenuEntry::item(label, None), Some(action));
            let disabled = |label: &str| {
                let mut item = MenuEntry::item(label, None);
                item.disabled = true;
                (item, None)
            };
            let point = self.article_point(x, y);
            let selected = self.selection.as_ref().is_some_and(|r| r.start <= point && point <= r.end);
            let mut items = Vec::new();
            if selected {
                if self.era == Era::Win2000 {
                    items.push(disabled("Cu&t"));
                }
                items.push(command("&Copy", Action::Part(Part::Copy)));
                if self.era == Era::Win2000 {
                    items.push(disabled("&Paste"));
                }
                items.push(command("Select &All", Action::SelectAll));
                items.push(command("P&rint", Action::Part(Part::Print)));
            } else {
                self.selection = None;
                let old = self.era == Era::Win98;
                let history = |label, hit, available: bool| {
                    if old && !available { disabled(label) } else { command(label, Action::Part(Part::Control(hit))) }
                };
                items.extend([
                    history("&Back", Hit::Back, self.visit > 0),
                    history("For&ward", Hit::Forward, self.visit + 1 < self.history.len()),
                    (MenuEntry::separator(), None),
                ]);
                if old {
                    items.extend([
                        disabled("&Save Background As..."),
                        disabled("Set as &Wallpaper"),
                        disabled("&Copy Background"),
                        (MenuEntry::separator(), None),
                    ]);
                }
                items.extend([command("Select &All", Action::SelectAll), (MenuEntry::separator(), None)]);
                // These shell commands require an external document or browser profile.
                // A catalog consists of local text and does not have either.
                if old {
                    items.extend([disabled("Create Short&cut"), disabled("Add to &Favorites...")]);
                }
                items.extend([disabled("&View Source"), (MenuEntry::separator(), None)]);
                if old {
                    let mut language = MenuEntry::submenu("&Language", None);
                    language.disabled = true;
                    items.extend([(language, None), (MenuEntry::separator(), None)]);
                }
                items.extend([
                    command(if old { "P&rint" } else { "P&rint..." }, Action::Part(Part::Print)),
                    command("&Refresh", Action::Refresh),
                    (MenuEntry::separator(), None),
                    disabled("&Properties"),
                ]);
            }
            self.tab_focus(Part::Control(Hit::Article));
            self.menu_at(items, Rect::new(x, y, 0, 0));
        }
        Vec::new()
    }
    fn wheel(&mut self, notches: i32) {
        if matches!(self.hover, Some(Part::Control(Hit::Row(_)))) && self.nav {
            let layout = self.layout_now();
            let visible = layout.visible_rows();
            self.first =
                self.first.saturating_add_signed((-3 * notches) as isize).min(self.count().saturating_sub(visible));
        } else {
            self.scroll_to(self.scroll() - 48 * notches);
        }
    }
}

// The Help search index matches both "mines" and "mine" as words. It does not
// match the substring at the start of "Minesweeper".
fn singular(word: &str) -> &str {
    if word.len() > 3 && !word.ends_with("ss") { word.strip_suffix('s').unwrap_or(word) } else { word }
}
