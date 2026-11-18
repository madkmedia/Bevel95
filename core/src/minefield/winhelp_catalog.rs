//! The game's two Windows 95 Help topics, in the order shown by the Contents tab.
use crate::help::{Catalog, Entry, EntryIcon, Keyword, Paragraph, Style, Topic};
use Style::{Bullet, Continuation, NestedBullet, Step, Subhead, Title};

const PLAY: &[(Style, &str)] = &[
    (Title, "Starting a game of Minesweeper"),
    (Step(1), "Choose New from the Game menu."),
    (Continuation, "Above the field, the left counter shows the mine count minus the flags you have placed."),
    (Step(2), "Click a covered square. Your first move starts the clock."),
    (
        NestedBullet,
        "Read the number that appears. It counts the mines touching that square along its sides or at its corners. Use the numbers to decide which square to open next. Opening a mine ends the game.",
    ),
    (NestedBullet, "Place a flag with the right mouse button when you know where a mine is hiding."),
    (Subhead, "Note"),
    (
        Bullet,
        "You win when every square without a mine is open. The clock stops on your last move. A shorter time earns a better score.",
    ),
];

const TIPS: &[(Style, &str)] = &[
    (Title, "Minesweeper strategy and tips"),
    (
        Bullet,
        "If you want to revisit a square, right-click it twice. With Marks (?) turned on, the first click puts a flag there and the second leaves a question mark. Right-click again to clear the mark. A question mark does not change the mine counter.",
    ),
    (
        Bullet,
        "When a number touches as many flags as its value, press both mouse buttons on the number. Release to open every unflagged neighbor. Check your flags first. A misplaced flag can cost you the game.",
    ),
    (
        Bullet,
        "A straight edge can reveal several mines at once. Suppose a 3 has only three covered neighbors, all in a row. Each must hold a mine. If a 2 at either end touches two of those mines, its other neighbors are safe. Open those first.",
    ),
];

pub(super) fn catalog() -> Catalog {
    let rows = [("How to play Minesweeper", "how to play", PLAY), ("Strategy and Tips", "strategy", TIPS)];
    let topics = rows
        .iter()
        .map(|&(title, _, body)| Topic {
            title: title.into(),
            welcome: false,
            paragraphs: body
                .iter()
                .map(|(style, text)| Paragraph { style: style.clone(), text: (*text).into() })
                .collect(),
        })
        .collect();
    let contents = rows
        .iter()
        .enumerate()
        .map(|(i, &(label, _, _))| Entry { label: label.into(), depth: 0, topic: Some(i), icon: EntryIcon::Page })
        .collect();
    let index = rows.iter().enumerate().map(|(i, &(_, label, _))| Keyword { label: label.into(), topic: i }).collect();
    Catalog { title: "Minesweeper Help".into(), home: 0, topics, contents, index }
}
