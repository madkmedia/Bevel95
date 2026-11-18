use super::*;
use crate::help::{Entry, Keyword, Topic};

// Use a second book so the viewer interactions exercise the caller-supplied hierarchy.
fn book() -> Catalog {
    let entry = |label: &str, depth, topic, icon| Entry { label: label.into(), depth, topic, icon };
    Catalog {
        title: "Drawing Help".into(),
        home: 0,
        topics: vec![
            Topic::from_paragraphs("Welcome", &[(Style::Body, "Open a drawing.")]),
            Topic::from_paragraphs("Pencils", &[(Style::Body, "An outline can follow another outline.")]),
            Topic::from_paragraphs("Brushes", &[(Style::Body, "Make outlines, then add colour.")]),
        ],
        contents: vec![
            entry("Drawing", 0, None, EntryIcon::Book),
            entry("Tools", 1, None, EntryIcon::Book),
            entry("Pencils", 2, Some(1), EntryIcon::Page),
            entry("Brushes", 2, Some(2), EntryIcon::Page),
        ],
        index: vec![Keyword { label: "lines".into(), topic: 1 }, Keyword { label: "lines".into(), topic: 2 }],
    }
}

fn fonts() -> Fonts<'static> {
    let font = crate::text::ms_sans_serif();
    Fonts { regular: font, bold: font }
}

/// 98's Calculator book has two books at the top: the first opens chosen and open, the second
/// stays closed (win98-truecolor-calc-help-topics); 2000's one book opens.
#[test]
fn only_the_first_book_of_the_contents_opens() {
    let form = HtmlHelp::new(Era::Win98, crate::calculator::help::catalog(Era::Win98), Tab::Contents);
    assert_eq!(form.tree, [0, 1, 2, 3, 4, 5]);
    assert_eq!(form.selected, Some(0));
    let form = HtmlHelp::new(Era::Win2000, crate::calculator::help::catalog(Era::Win2000), Tab::Contents);
    assert_eq!(form.tree, (0..13).collect::<Vec<_>>());
}

/// A row wider than the tree brings Windows' horizontal scroll bar: the thumb is the page's
/// share (98's Calculator book: rows to 241 pixels in 209, a 153-pixel thumb in 177, as the
/// PC showed); arrows move 5 pixels, the track a page, never past the widest row.
#[test]
fn a_wide_tree_scrolls_sideways() {
    let mut form = HtmlHelp::new(Era::Win98, crate::calculator::help::catalog(Era::Win98), Tab::Contents);
    let layout = form.layout_now();
    assert_eq!((layout.tree_width, layout.tree_page(), layout.tree_max_left()), (241, 209, 32));
    let thumb = form.rect(Part::Control(Hit::NavHThumb)).unwrap();
    assert_eq!((thumb.x, thumb.w), (27, 153));
    form.activate(Part::Control(Hit::NavRight));
    assert_eq!(form.layout_now().tree_left, 5);
    let track = form.rect(Part::Control(Hit::NavHTrack)).unwrap();
    form.press(Some(Part::Control(Hit::NavHTrack)), track.right() - 1, track.y);
    assert_eq!(form.layout_now().tree_left, 32);
    assert_eq!(form.rect(Part::Control(Hit::NavHThumb)).unwrap().right(), track.right());
    form.activate(Part::Control(Hit::NavLeft));
    assert_eq!(form.layout_now().tree_left, 27);
    // Rows that fit: no bar (the games' books).
    let form = HtmlHelp::new(Era::Win98, crate::solitaire::help::catalog(Era::Win98), Tab::Contents);
    assert!(form.rect(Part::Control(Hit::NavHTrack)).is_none());
}

#[test]
fn newer_visits_replace_forward_history_and_expand_nested_contents() {
    let mut form = HtmlHelp::new(Era::Win2000, book(), Tab::Contents);
    assert_eq!(form.tree, [0, 1]);
    form.select(1, true);
    assert_eq!(form.tree, [0, 1, 2, 3]);
    form.select(2, false);
    form.show_topic(2);
    form.navigate_history(false);
    assert_eq!(form.topic(), 1);
    assert!(form.show_topic(0));
    form.navigate_history(true);
    assert_eq!(form.topic(), 0);
    assert!(!form.show_topic(usize::MAX));
    form.select(0, false);
    let subtree = form.print_selection(true);
    assert!(subtree.contains("An outline") && subtree.contains("add colour"));
    assert!(!subtree.contains("Open a drawing."));
    form.tab_focus(Part::Control(Hit::Article));
    assert!(!form.print_uses_chooser());
}

#[test]
fn search_after_clicking_empty_query_keeps_first_letter_and_highlights_displayed_topic() {
    for era in [Era::Win98, Era::Win2000] {
        let mut form = HtmlHelp::new(era, book(), Tab::Search);
        form.layout(&fonts());
        let query = form.layout_now().query;
        let hit = form.hit(query.x + 10, query.y + 5);
        form.press(hit, query.x + 10, query.y + 5);
        for ch in "outlines".chars() {
            form.typed(ch);
        }
        assert_eq!(form.query, "outlines");
        form.activate(form.enter());
        assert_eq!(form.results, [2, 1]);
        assert!(form.matches.is_empty());
        form.select(1, false);
        assert_eq!(form.topic(), 0);
        form.activate(Part::Control(Hit::Display));
        form.layout(&fonts());
        assert_eq!(form.topic(), 1);
        assert_eq!(form.matches.len(), 2);
        form.navigate_history(false);
        form.layout(&fonts());
        assert!(form.matches.is_empty());
        form.navigate_history(true);
        form.layout(&fonts());
        assert_eq!(form.matches.len(), 2);
        form.show_tab(Tab::Index);
        form.paste("ä");
        form.select_all();
        assert_eq!(form.copy_text(), "ä");
        form.paste("lines");
        form.show_tab(Tab::Search);
        assert_eq!(form.query, "outlines");
        form.show_tab(Tab::Index);
        assert_eq!(form.query, "lines");
        assert!(matches!(form.release(Part::Control(Hit::Display)).as_slice(), [Effect::Activate(Part::TopicsFound)]));
        assert_eq!(form.enter(), Part::TopicsFound);
        let choices = form.keyword_topics();
        assert_eq!(choices, [(1, "Pencils".into()), (2, "Brushes".into())]);
        form.show_topic(choices[1].0);
        assert_eq!(form.topic(), 2);
    }
}

#[test]
fn find_uses_whole_words_and_leaves_the_final_match_selected_at_end() {
    let mut form = HtmlHelp::new(Era::Win2000, book(), Tab::Contents);
    form.show_topic(1);
    form.layout(&fonts());
    let opts = commands::FindOptions { whole_word: true, match_case: false, down: true };
    assert!(!form.find_next("out", opts));
    assert!(form.find_next("OUTLINE", opts));
    assert_eq!(form.copy_text(), "outline");
    let first = form.selection.clone();
    assert!(form.find_next("outline", opts));
    assert_ne!(first, form.selection);
    assert!(!form.find_next("outline", opts));
    assert!(form.find_next("outline", commands::FindOptions { down: false, ..opts }));
    assert_eq!(form.selection, first);
}

#[test]
fn options_accelerators_dispatch_native_commands_and_open_local_web_help() {
    let mut form = HtmlHelp::new(Era::Win2000, book(), Tab::Contents);
    let options = Part::Control(Hit::Options);
    form.activate(options);
    assert!(matches!(form.typed('p').as_slice(), [Effect::Activate(Part::Print)]));
    assert!(form.popup.is_none());
    form.activate(options);
    let command = form.typed('2');
    let [Effect::Activate(command)] = command.as_slice() else { panic!("missing Web Help command") };
    form.activate(*command);
    assert_eq!(form.topic(), form.web);
    assert!(form.popup.is_none());
}

#[test]
fn double_click_selects_a_word_with_the_era_specific_copy_menu() {
    for era in [Era::Win98, Era::Win2000] {
        let mut form = HtmlHelp::new(era, book(), Tab::Contents);
        form.layout(&fonts());
        let bounds = form.article.range_rect(&(0..4)).unwrap();
        let pane = form.layout_now().article;
        let (x, y) = (pane.x + bounds.x + bounds.w / 2, pane.y + bounds.y + bounds.h / 2);
        form.double_click(x, y);
        assert_eq!(form.copy_text(), "Open");
        form.button(crate::kit::Button::Right, false, x, y);
        let menu = form.popup.as_ref().unwrap();
        let labels: Vec<_> =
            menu.items.iter().map(|(entry, _)| crate::text::split_access_key(&entry.label).0).collect();
        let expected = if era == Era::Win98 {
            vec!["Copy", "Select All", "Print"]
        } else {
            vec!["Cut", "Copy", "Paste", "Select All", "Print"]
        };
        assert_eq!(labels, expected);
        assert!(matches!(form.typed('c').as_slice(), [Effect::Activate(Part::Copy)]));
    }
}

#[test]
fn hiding_navigation_preserves_article_width_and_resizing_a_maximized_viewer_keeps_window_width() {
    for era in [Era::Win98, Era::Win2000] {
        let mut form = HtmlHelp::new(era, book(), Tab::Contents);
        let size = form.size();
        let native_width = form.layout_now().article.w;
        let nav = Part::Control(Hit::NavToggle);
        form.activate(nav);
        assert!(form.size().0 < size.0);
        // The hidden splitter also frees a few non-client border pixels in each edition.
        assert!(form.layout_now().article.w.abs_diff(native_width) < 10);
        form.show_tab(Tab::Index);
        assert_eq!(form.size(), size);
        form.set_maximized(true);
        form.activate(nav);
        assert_eq!(form.size(), size);
        assert!(form.layout_now().article.w > native_width);
    }
}
