//! A Help book supplied by the program asking for help. Topic references are positions
//! in this book, independent of the program's names or commands.

pub(crate) mod table;
pub mod winhelp;
pub mod wire;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub title: String,
    pub home: usize,
    pub topics: Vec<Topic>,
    pub contents: Vec<Entry>,
    pub index: Vec<Keyword>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topic {
    pub title: String,
    pub welcome: bool,
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paragraph {
    pub style: Style,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Style {
    Title,
    Subhead,
    Note,
    Body,
    Continuation,
    Step(u8),
    Bullet,
    NestedBullet,
    Link(usize),
    Related(Vec<usize>),
    /// Rows are separated by newlines and cells by tabs. The first row can be a heading.
    Table {
        header: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryIcon {
    Book,
    Page,
    Overview,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub label: String,
    pub depth: usize,
    pub topic: Option<usize>,
    pub icon: EntryIcon,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyword {
    pub label: String,
    pub topic: usize,
}

impl Topic {
    /// Build a page from the styled paragraphs supplied by a program.
    pub fn from_paragraphs(title: &str, items: &[(Style, &str)]) -> Topic {
        let paragraphs =
            items.iter().map(|(style, text)| Paragraph { style: style.clone(), text: (*text).into() }).collect();
        Topic { title: title.into(), welcome: false, paragraphs }
    }

    pub fn plain_text(&self) -> String {
        self.paragraphs
            .iter()
            .map(|p| match p.style {
                Style::Step(n) => format!("{n}. {}", p.text),
                Style::Bullet | Style::NestedBullet => format!("• {}", p.text),
                _ => p.text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}
