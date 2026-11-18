//! What Calculator remembers between runs, as the real ones kept it in WIN.INI's `[SciCalc]`
//! section: `layout` (1 Standard, 0 Scientific) and, on Windows 2000, `UseSep` (Digit
//! grouping). Bevel95 keeps the same lines in calculator.txt beside settings.txt.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub scientific: bool,
    pub grouping: bool,
}

impl Settings {
    pub fn parse(text: &str) -> Settings {
        let mut settings = Settings::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let on = value.trim() == "1";
            match key.trim().to_ascii_lowercase().as_str() {
                "layout" => settings.scientific = value.trim() == "0",
                "usesep" => settings.grouping = on,
                _ => {}
            }
        }
        settings
    }

    pub fn to_text(&self) -> String {
        format!("[SciCalc]\r\nlayout={}\r\nUseSep={}\r\n", u8::from(!self.scientific), u8::from(self.grouping))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_back_what_it_writes() {
        assert_eq!(Settings::parse(""), Settings::default());
        let s = Settings { scientific: false, grouping: true };
        assert_eq!(s.to_text(), "[SciCalc]\r\nlayout=1\r\nUseSep=1\r\n");
        assert_eq!(Settings::parse(&s.to_text()), s);
        assert!(Settings::parse("[SciCalc]\nlayout=0\n").scientific);
    }
}
