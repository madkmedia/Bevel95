//! Solitaire's `[Solitaire]` options and explicitly selected card back. Windows stored these
//! as two decimal numbers in SOL.INI (95/98) or WIN.INI (2000). Bevel95 keeps the same
//! numbers in solitaire.txt.

use super::game::{Draw, Rules, Scoring};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub rules: Rules,
    pub status_bar: bool,
    pub outline: bool,
    /// CARDS back index, in resource order (54 through 65).
    pub deck: u8,
    /// Until the player accepts Select Card Back, SOL picks a new design at startup.
    pub remember_deck: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { rules: Rules::default(), status_bar: true, outline: false, deck: 0, remember_deck: false }
    }
}

impl Settings {
    pub fn parse(text: &str) -> Self {
        Self::parse_with_random(text, 0)
    }

    pub fn parse_with_random(text: &str, first_rand: u32) -> Self {
        // SOL calculates 53 + rand() % 12 and then clamps to resource 54..65. The first
        // design can therefore appear twice, while the last can only be picked manually.
        let deck = ((first_rand % 12) as u8).saturating_sub(1);
        let mut settings = Self { deck, ..Self::default() };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let Ok(n) = value.trim().parse::<u8>() else { continue };
            match key.trim().to_ascii_lowercase().as_str() {
                "options" if n & 0x80 == 0 && (n >> 4) & 3 != 3 => {
                    settings.status_bar = n & 1 != 0;
                    settings.rules.timed = n & 2 != 0;
                    settings.outline = n & 4 != 0;
                    settings.rules.draw = if n & 8 != 0 { Draw::Three } else { Draw::One };
                    settings.rules.scoring = match (n >> 4) & 3 {
                        1 => Scoring::Vegas,
                        2 => Scoring::None,
                        _ => Scoring::Standard,
                    };
                    settings.rules.cumulative = n & 64 != 0;
                }
                "back" if (1..=12).contains(&n) => {
                    settings.deck = n - 1;
                    settings.remember_deck = true;
                }
                _ => {}
            }
        }
        settings
    }

    pub fn options_value(&self) -> u8 {
        let scoring_bits = match self.rules.scoring {
            Scoring::Standard => 0,
            Scoring::Vegas => 16,
            Scoring::None => 32,
        };
        u8::from(self.status_bar)
            | u8::from(self.rules.timed) << 1
            | u8::from(self.outline) << 2
            | u8::from(self.rules.draw == Draw::Three) << 3
            | scoring_bits
            | u8::from(self.rules.cumulative) << 6
    }

    pub fn to_text(&self) -> String {
        let options = self.options_value();
        let mut output = format!("[Solitaire]\r\nOptions={options}\r\n");
        if self.remember_deck {
            output.push_str(&format!("Back={}\r\n", self.deck + 1));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_profile_values_round_trip() {
        assert_eq!(Settings::default().to_text(), "[Solitaire]\r\nOptions=11\r\n");
        let saved = Settings::parse("[Solitaire]\r\nOptions=85\r\nBack=12\r\n");
        assert_eq!(saved.rules, Rules { draw: Draw::One, scoring: Scoring::Vegas, timed: false, cumulative: true });
        assert!(saved.status_bar && saved.outline && saved.remember_deck);
        assert_eq!(saved.deck, 11);
        assert_eq!(Settings::parse(&saved.to_text()), saved);
    }

    #[test]
    fn a_generated_back_is_only_remembered_after_an_explicit_choice() {
        let generated = Settings::parse_with_random("Back=999\nOptions=bad", 23);
        assert_eq!(generated.deck, 10);
        assert_eq!(generated.rules, Settings::default().rules);
        assert!(!generated.to_text().contains("Back"));
        assert_eq!(Settings::parse_with_random(&generated.to_text(), 13).deck, 0);
    }
}
