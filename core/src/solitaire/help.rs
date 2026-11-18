//! The three Solitaire Help books. The navigation follows the original books, and
//! the instructions describe this game's controls in our own words.
use crate::help::{Catalog, Entry, EntryIcon, Keyword, Paragraph, Style, Topic};
use crate::theme::Era;
use Style::{Body, Bullet, Link, Note, Related, Step, Subhead, Title};

const PLAY: usize = 0;
const SCORE: usize = 1;
const OBJECT: usize = 2;

fn page(label: &str, depth: usize, topic: usize, icon: EntryIcon) -> Entry {
    Entry { label: label.into(), depth, topic: Some(topic), icon }
}

fn index(rows: &[(&str, usize)]) -> Vec<Keyword> {
    rows.iter().map(|&(label, topic)| Keyword { label: label.into(), topic }).collect()
}

fn play(era: Era, title: &str) -> Topic {
    let mut topic = Topic::from_paragraphs(
        title,
        &[
            (if era == Era::Win95 { Title } else { Subhead }, "To play Solitaire"),
            (Step(1), "Choose Deal from the Game menu. Look at the face-up cards at the foot of the seven columns."),
            (
                Step(2),
                "Drag an ace to an empty space at the upper right. Follow it with the two and then the three of the same suit.",
            ),
            (
                Step(3),
                "Click the deck at the upper left to draw. Only the top card of the turned-over pile can be played.",
            ),
            (if era == Era::Win95 { Subhead } else { Note }, "Notes"),
            (
                Bullet,
                "Build down the columns in alternating colors. For example, a red seven fits on a black eight. To move a sequence together, drag its top card.",
            ),
            (
                Bullet,
                "Click the face-down card you uncover at the foot of a column. An empty column can take a king, alone or with the cards below it.",
            ),
            (
                Bullet,
                "Double-click an exposed card to move it to a suit stack when it fits. Finish all four suit stacks, ace through king, to win.",
            ),
        ],
    );
    if era == Era::Win2000 {
        topic.paragraphs.push(Paragraph {
            style: Bullet,
            text: "Right-click the board or press Ctrl+A to send all available cards to their suit stacks.".into(),
        });
    }
    topic
}

fn scoring(era: Era, title: &str) -> Topic {
    Topic::from_paragraphs(
        title,
        &[
            (if era == Era::Win95 { Title } else { Subhead }, "To choose a scoring system"),
            (Step(1), "Choose Options from the Game menu."),
            (
                Step(2),
                "In the Scoring group select Standard, Vegas or None, then click OK. Changing the scoring system starts a new deal.",
            ),
            (if era == Era::Win95 { Subhead } else { Note }, "Notes"),
            (
                Bullet,
                "Standard gives you 5 points for turning up a card in a column or moving a card from the deck to a column. Move a card to a suit stack for 10 points. Taking one back costs 15 points.",
            ),
            (
                Bullet,
                "In Vegas each deal costs $52. You earn $5 for every card you place on a suit stack. Drawing one card gives you a single trip through the deck. Drawing three allows three trips.",
            ),
            (
                Bullet,
                if era == Era::Win2000 {
                    "With Vegas selected, choose Cumulative Score to carry your balance into the next deal while Solitaire is open."
                } else {
                    "With Vegas selected, choose Keep score to carry your balance into the next deal while Solitaire is open."
                },
            ),
            (
                Bullet,
                "Choose None for an unscored game. Under Draw you can choose how many cards to turn over at a time.",
            ),
        ],
    )
}

fn overview(title: &str, style: Style) -> Topic {
    Topic::from_paragraphs(
        title,
        &[
            (style, title),
            (
                Body,
                "Move all 52 cards onto the four suit stacks at the upper right. Start each stack with an ace and finish with the king of that suit.",
            ),
        ],
    )
}

pub fn catalog(era: Era) -> Catalog {
    match era {
        Era::Win95 => Catalog {
            title: "Solitaire Help".into(),
            home: PLAY,
            topics: vec![play(era, "How to play Solitaire"), scoring(era, "Scoring information")],
            contents: vec![
                page("How to play Solitaire", 0, PLAY, EntryIcon::Page),
                page("Scoring information", 0, SCORE, EntryIcon::Page),
            ],
            index: index(&[("how to play", PLAY), ("scoring", SCORE)]),
        },
        Era::Win98 => windows_98(),
        Era::Win2000 => windows_2000(),
    }
}

fn windows_98() -> Catalog {
    const WELCOME: usize = 3;
    const ABOUT: usize = 4;
    let mut welcome = Topic::from_paragraphs(
        "Welcome to Help",
        &[
            (Title, "Welcome to Help"),
            (Body, "Use this book to learn how to play Solitaire."),
            (Bullet, "Browse the contents to get started."),
            (Bullet, "Learn to build columns and suit stacks."),
            (Bullet, "Look up drawing and scoring in the index."),
            (Bullet, "Search for a word from the instructions."),
            (Link(ABOUT), "About this Help book"),
        ],
    );
    welcome.welcome = true;
    Catalog {
        title: "Solitaire Help".into(),
        home: WELCOME,
        topics: vec![
            play(Era::Win98, "To play Solitaire"),
            scoring(Era::Win98, "To choose a scoring system"),
            overview("The object of Solitaire", Subhead),
            welcome,
            Topic::from_paragraphs(
                "Legal Information",
                &[
                    (Title, "About this Help book"),
                    (
                        Body,
                        "These instructions were written for Bevel95's Solitaire. The topic list and Help window follow the Windows 98 edition.",
                    ),
                ],
            ),
        ],
        contents: vec![
            page("The object of Solitaire", 0, OBJECT, EntryIcon::Page),
            page("Playing Solitaire", 0, PLAY, EntryIcon::Page),
            page("Choosing a scoring system", 0, SCORE, EntryIcon::Page),
        ],
        index: index(&[
            ("drawing options", SCORE),
            ("object of the game", OBJECT),
            ("playing the game", PLAY),
            ("scoring", SCORE),
        ]),
    }
}

fn windows_2000() -> Catalog {
    const OPTIONS: usize = 3;
    let mut topics = vec![
        play(Era::Win2000, "Play Solitaire"),
        scoring(Era::Win2000, "Choose a scoring system"),
        overview("Solitaire overview", Title),
        Topic::from_paragraphs(
            "Change game options",
            &[
                (Subhead, "To change game options"),
                (
                    Step(1),
                    "Choose Options from the Game menu. Pick Draw One or Draw Three to set how many cards turn over with a click of the deck.",
                ),
                (
                    Step(2),
                    "Select Timed game to run the clock. Select Status bar to see the time and score along the bottom of the window.",
                ),
                (
                    Body,
                    "Select Outline dragging to see the outline of the cards while you move them. Click OK to accept your choices. A change to the draw, scoring system or timer starts a new deal.",
                ),
                (Note, "Note"),
                (
                    Bullet,
                    "To change the picture on the cards, choose Deck from the Game menu. Select a card back and click OK.",
                ),
            ],
        ),
    ];
    for (topic, links) in topics.iter_mut().zip([
        vec![OPTIONS, OBJECT, SCORE],
        vec![PLAY, OBJECT, OPTIONS],
        vec![OPTIONS, PLAY, SCORE],
        vec![PLAY, OBJECT, SCORE],
    ]) {
        topic.paragraphs.push(Paragraph { style: Related(links), text: "Related Topics".into() });
    }
    Catalog {
        title: "Solitaire".into(),
        home: OBJECT,
        topics,
        contents: vec![
            Entry { label: "Solitaire".into(), depth: 0, topic: None, icon: EntryIcon::Book },
            page("Solitaire overview", 1, OBJECT, EntryIcon::Overview),
            page("Play Solitaire", 1, PLAY, EntryIcon::Page),
            page("Change game options", 1, OPTIONS, EntryIcon::Page),
            page("Choose a scoring system", 1, SCORE, EntryIcon::Page),
        ],
        index: index(&[
            ("changing game options", OPTIONS),
            ("dealing the cards", PLAY),
            ("deck design, changing", OPTIONS),
            ("how to play", PLAY),
            ("moving the cards", PLAY),
            ("object of the game", OBJECT),
            ("options", OPTIONS),
            ("outline dragging", OPTIONS),
            ("playing the game", PLAY),
            ("score, viewing", OPTIONS),
            ("scoring systems", SCORE),
            ("starting the game", PLAY),
            ("timed games", OPTIONS),
            ("winning", OBJECT),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_editions_index_opens_its_scoring_topic_in_the_shared_viewer() {
        for (era, home, keyword, scoring_title) in [
            (Era::Win95, "How to play Solitaire", "scoring", "Scoring information"),
            (Era::Win98, "Welcome to Help", "scoring", "To choose a scoring system"),
            (Era::Win2000, "Solitaire overview", "scoring systems", "Choose a scoring system"),
        ] {
            let mut packet = Vec::new();
            crate::help::wire::write(&mut packet, &catalog(era)).unwrap();
            let book = crate::help::wire::read(&mut packet.as_slice()).unwrap();
            assert_eq!(book.topics[book.home].title, home);
            let entry = book.index.iter().find(|entry| entry.label == keyword).unwrap();
            assert_eq!(book.topics[entry.topic].title, scoring_title);
        }
    }
}
