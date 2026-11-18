//! The Help books supplied by Minesweeper. Contents, index terms and topic order follow
//! the reference machines. The article text is written for this implementation.

use crate::help::{Catalog, Entry, EntryIcon, Keyword, Paragraph, Style, Topic};
use crate::theme::Era;
use Style::{Body, Bullet, Link, Note, Related, Step, Subhead, Title};

#[path = "winhelp_catalog.rs"]
mod winhelp_catalog;

fn page(label: &str, depth: usize, topic: usize, icon: EntryIcon) -> Entry {
    Entry { label: label.into(), depth, topic: Some(topic), icon }
}

fn book(label: &str, depth: usize) -> Entry {
    Entry { label: label.into(), depth, topic: None, icon: EntryIcon::Book }
}

fn keywords(rows: &[(&str, usize)]) -> Vec<Keyword> {
    rows.iter().map(|&(label, topic)| Keyword { label: label.into(), topic }).collect()
}

pub fn catalog(era: Era) -> Catalog {
    if era == Era::Win95 {
        return winhelp_catalog::catalog();
    }
    let nt = era == Era::Win2000;
    let overview = if nt { "Minesweeper overview" } else { "The object of Minesweeper" };
    let play = if nt { "Play Minesweeper" } else { "Playing Minesweeper" };
    let tips = "Strategies and tips";
    let custom = "Customize the playing field";
    let overview_body = if nt {
        "Uncover every square that is free of mines. Leave the mined squares covered. The clock records how quickly you clear the field. Opening a mine ends the game."
    } else {
        "Clear all the squares that are free of mines. Try to finish in the shortest time you can."
    };
    let mut topics = vec![
        Topic::from_paragraphs(overview, &[(if nt { Title } else { Subhead }, overview), (Body, overview_body)]),
        Topic::from_paragraphs(
            if nt { play } else { "To play Minesweeper" },
            &[
                (Subhead, "To play Minesweeper"),
                (Step(1), "Choose New from the Game menu to clear the board."),
                (Step(2), "Click a covered square to begin. The counter on the right starts timing your game."),
                (Note, "Notes"),
            ],
        ),
        Topic::from_paragraphs(
            tips,
            &[
                (if nt { Title } else { Subhead }, tips),
                (
                    Bullet,
                    "Use a question mark as a reminder to come back to a square. With Marks (?) enabled, two right-clicks put a question mark on a covered square. A third removes it.",
                ),
                (
                    Bullet,
                    "Suppose a 2 touches exactly two flags. Press both mouse buttons on the 2 and release to open the other squares around it. Make sure your flags are correct first.",
                ),
                (
                    Bullet,
                    "Count the remaining covered neighbors of a number. If a 3 touches just three covered squares, each of them must hide a mine. A nearby number can then help you find a safe square.",
                ),
            ],
        ),
    ];
    let notes = [
        "The left counter is the mine total minus the number of flags you have placed. The other counter shows elapsed seconds.",
        "Left-click a safe square to uncover it. An empty patch can open several squares at once. Opening a mine ends the game.",
        "A number counts the mines that touch its square, including diagonals. A 1 means exactly one of its neighbors holds a mine.",
        "Right-click a square to flag a suspected mine. A flagged square stays covered while you open the squares beside it.",
    ];
    let order = if nt { [1, 2, 3, 0] } else { [0, 1, 2, 3] };
    for i in order {
        topics[1].paragraphs.push(Paragraph { style: Bullet, text: notes[i].into() });
    }
    let (home, contents, index) = if nt {
        topics[2].paragraphs.push(Paragraph {
            style: Bullet,
            text: "Minimize the game if you need to leave the field for a while. The timer stops while the window is minimized and starts again when you restore it.".into(),
        });
        topics.push(Topic::from_paragraphs(custom, &[
            (Subhead, "To customize the playing field"),
            (Step(1), "Open the Game menu and choose Custom."),
            (Step(2), "Enter the height in rows, the width in columns and the number of mines. Click OK to play on the new field."),
        ]));
        for (t, links) in topics.iter_mut().zip([vec![1, 3, 2], vec![3, 0, 2], vec![3, 0, 1], vec![1, 0, 2]]) {
            t.paragraphs.push(Paragraph { style: Related(links), text: "Related Topics".into() });
        }
        let contents = vec![
            book("Minesweeper", 0),
            page(overview, 1, 0, EntryIcon::Overview),
            page(play, 1, 1, EntryIcon::Page),
            page(custom, 1, 3, EntryIcon::Page),
            page(tips, 1, 2, EntryIcon::Overview),
        ];
        let index = keywords(&[
            ("customizing playing field", 3),
            ("hints and tips", 1),
            ("hints and tips", 2),
            ("how to play", 1),
            ("marking suspected squares", 1),
            ("marking suspected squares", 2),
            ("new games", 1),
            ("numbered squares", 1),
            ("object of the game", 0),
            ("playing the game", 1),
            ("setting up playing field", 3),
            ("strategies", 2),
            ("timer", 2),
            ("winning", 0),
        ]);
        (0, contents, index)
    } else {
        let mut welcome = Topic::from_paragraphs(
            "Welcome to Help",
            &[
                (Title, "Welcome to Help"),
                (Body, "This book will help you get started with Minesweeper."),
                (Bullet, "Browse the contents for instructions."),
                (Bullet, "Read about numbers and flags."),
                (Bullet, "Look up a word in the index."),
                (Bullet, "Search for a rule or a strategy."),
                (Link(4), "About this Help book"),
            ],
        );
        welcome.welcome = true;
        topics.push(welcome);
        topics.push(Topic::from_paragraphs("Legal Information", &[(Title, "About this Help book"),
            (Body, "This guide accompanies Bevel95's recreation of Minesweeper. The window and its controls follow Windows 98 Help. The instructions were written for this game.")]));
        let contents = vec![
            page(overview, 0, 0, EntryIcon::Page),
            page(play, 0, 1, EntryIcon::Page),
            page(tips, 0, 2, EntryIcon::Page),
        ];
        (3, contents, keywords(&[("object of the game", 0), ("playing the game", 1), ("strategies and tips", 2)]))
    };
    Catalog { title: if nt { "Minesweeper" } else { "Minesweeper Help" }.into(), home, topics, contents, index }
}
