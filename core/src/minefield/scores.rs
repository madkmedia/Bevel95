//! What Minesweeper remembers between runs, as a small text file (minefield.txt, next to
//! Bevel95's settings.txt): the level, the custom field, question marks, where the window was,
//! the player's name and the best time of each level. Reading never fails: what cannot be
//! understood is left at its default.

use crate::minefield::game::{Field, Level};
use crate::theme::Era;

/// A level's best time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Best {
    pub seconds: u32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub level: Level,
    pub custom: Field,
    pub marks: bool,
    pub color: bool,
    /// The Sound command is available in Windows 2000.
    pub sound: bool,
    /// The window's top-left corner on the screen, in screen pixels.
    pub window: Option<(i32, i32)>,
    /// The name last entered for a best time.
    pub name: String,
    /// The best times of Beginner, Intermediate and Expert.
    pub best: [Option<Best>; 3],
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            level: Level::Beginner,
            custom: Field { width: 8, height: 8, mines: 10 },
            marks: true,
            color: true,
            sound: false,
            window: None,
            name: Self::ANONYMOUS.to_string(),
            best: [None, None, None],
        }
    }
}

/// The longest name kept.
pub const NAME_MAX: usize = 31;

fn level_word(level: Level) -> &'static str {
    match level {
        Level::Beginner => "beginner",
        Level::Intermediate => "intermediate",
        Level::Expert => "expert",
        Level::Custom => "custom",
    }
}

/// A level's place among the best times (Custom keeps none).
fn rank(level: Level) -> Option<usize> {
    Level::RANKED.iter().position(|&l| l == level)
}

impl Settings {
    pub const ANONYMOUS: &'static str = "Anonymous";

    /// Reads the file's text.
    pub fn parse(era: Era, text: &str) -> Settings {
        let mut s = Settings { custom: Level::Beginner.field(era).unwrap(), ..Settings::default() };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            let numbers: Vec<i64> = value.split_whitespace().map_while(|n| n.parse().ok()).collect();
            match key {
                "level" => {
                    if let Some(level) = [Level::Beginner, Level::Intermediate, Level::Expert, Level::Custom]
                        .into_iter()
                        .find(|&l| level_word(l) == value)
                    {
                        s.level = level;
                    }
                }
                "custom" => {
                    if let [w, h, m] = numbers[..]
                        && w > 0
                        && h > 0
                        && m > 0
                    {
                        s.custom = Field::custom(
                            era,
                            w as usize,
                            h as usize,
                            m as usize,
                            (Field::MAX_WIDTH, Field::MAX_HEIGHT),
                        );
                    }
                }
                "question_marks" => s.marks = value != "off",
                "color" => s.color = value != "off",
                "sound" => s.sound = value == "on",
                "window" => {
                    if let [x, y] = numbers[..] {
                        s.window = Some((x.clamp(-100_000, 100_000) as i32, y.clamp(-100_000, 100_000) as i32));
                    }
                }
                "name" => s.name = clean_name(value),
                _ => {
                    // best_<level> = <seconds> <name>
                    let level = Level::RANKED.into_iter().find(|&l| key.strip_prefix("best_") == Some(level_word(l)));
                    if let (Some(at), Some(&seconds)) = (level.and_then(rank), numbers.first())
                        && (0..=999).contains(&seconds)
                    {
                        let name = value.split_once(char::is_whitespace).map_or("", |(_, name)| name);
                        s.best[at] = Some(Best { seconds: seconds as u32, name: clean_name(name) });
                    }
                }
            }
        }
        s
    }

    /// The file's text.
    pub fn to_text(&self) -> String {
        let mut out = String::from("# Minesweeper settings and best times.\r\n");
        let mut line = |key: &str, value: String| out.push_str(&format!("{key} = {value}\r\n"));
        line("level", level_word(self.level).to_string());
        line("custom", format!("{} {} {}", self.custom.width, self.custom.height, self.custom.mines));
        line("question_marks", if self.marks { "on" } else { "off" }.to_string());
        line("color", if self.color { "on" } else { "off" }.to_string());
        line("sound", if self.sound { "on" } else { "off" }.to_string());
        if let Some((x, y)) = self.window {
            line("window", format!("{x} {y}"));
        }
        if self.name != Self::ANONYMOUS {
            line("name", self.name.clone());
        }
        for (level, best) in Level::RANKED.into_iter().zip(&self.best) {
            if let Some(best) = best {
                line(&format!("best_{}", level_word(level)), format!("{} {}", best.seconds, best.name));
            }
        }
        out
    }

    /// The best time of a level, if it has one.
    pub fn best(&self, level: Level) -> Option<&Best> {
        rank(level).and_then(|at| self.best[at].as_ref())
    }

    /// The initial entry is "999 seconds, Anonymous". Only a shorter time replaces it.
    pub fn beats(&self, level: Level, seconds: u32) -> bool {
        rank(level).is_some_and(|at| seconds < self.best[at].as_ref().map_or(999, |best| best.seconds))
    }

    /// Writes a best time down, under `name` (which is remembered for the next one).
    pub fn record(&mut self, level: Level, seconds: u32, name: &str) {
        let name = clean_name(name);
        if let Some(at) = rank(level) {
            self.best[at] = Some(Best { seconds, name: name.clone() });
        }
        self.name = name;
    }

    /// Forgets every best time.
    pub fn reset_best(&mut self) {
        self.best = [None, None, None];
    }
}

/// A name as it is kept: one line, no spaces at its ends, at most `NAME_MAX` characters.
pub fn clean_name(name: &str) -> String {
    let one_line: String = name.chars().filter(|c| !c.is_control()).collect();
    one_line.trim().chars().take(NAME_MAX).collect::<String>().trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_is_read_back() {
        let mut s = Settings {
            level: Level::Custom,
            marks: false,
            color: false,
            sound: true,
            window: Some((-30, 250)),
            ..Settings::default()
        };
        s.custom = Field { width: 25, height: 10, mines: 33 };
        s.record(Level::Beginner, 12, "  Ada Lovelace ");
        s.record(Level::Expert, 301, "");
        assert_eq!(s.name, "", "the last name entered, even an empty one");
        s.name = "Ada".to_string();
        let text = s.to_text();
        assert_eq!(Settings::parse(Era::Win2000, &text), s);
        assert!(text.contains("best_beginner = 12 Ada Lovelace\r\n"), "{text}");
        assert!(text.contains("best_expert = 301 \r\n"), "{text}");
        assert!(!text.contains("best_intermediate"));
    }

    #[test]
    fn a_missing_or_broken_file_gives_the_defaults() {
        let read = |text| Settings::parse(Era::Win95, text);
        assert_eq!(read(""), Settings::default());
        assert_eq!(Settings::parse(Era::Win2000, "").custom.width, 9);
        let odd = "level = wizard\ncustom = 1 2\nwindow = here\nbest_beginner = fast Bob\nbest_custom = 5 Eve\nnonsense\n= =\n";
        assert_eq!(read(odd), Settings::default());
        // a custom field outside the limits is brought inside them
        assert_eq!(read("custom = 500 3 9999").custom, Field { width: 30, height: 8, mines: 203 });
        assert!(!read("question_marks = off").marks);
        assert_eq!(read("best_expert = 1000 Too slow").best[2], None);
    }

    #[test]
    fn only_a_faster_time_beats_the_best() {
        let mut s = Settings::default();
        assert!(!s.beats(Level::Intermediate, 999), "ties with the default entry");
        assert!(s.beats(Level::Intermediate, 998));
        assert!(!s.beats(Level::Custom, 1), "custom fields keep no best time");
        s.record(Level::Intermediate, 80, "Grace");
        assert!(s.beats(Level::Intermediate, 79) && !s.beats(Level::Intermediate, 80));
        assert_eq!(s.best(Level::Intermediate), Some(&Best { seconds: 80, name: "Grace".to_string() }));
        assert_eq!(s.best(Level::Beginner), None);
        s.reset_best();
        assert_eq!(s.best, [None, None, None]);
        assert_eq!(clean_name("  a very long name that goes on and on and on  "), "a very long name that goes on a");
        assert_eq!(clean_name("two\r\nlines"), "twolines");
    }
}
