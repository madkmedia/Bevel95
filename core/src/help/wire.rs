//! One Help catalog on a pipe. The signature and version are followed by a little-endian
//! u32 payload size. Strings are length-prefixed UTF-8, lists start with their item count,
//! and topic references are zero-based u32 indices. Tags are single bytes.
//! The next byte after the payload belongs to the caller (e.g. a later viewer command).

use std::io::{self, Read, Write};

use super::{Catalog, Entry, EntryIcon, Keyword, Paragraph, Style, Topic};

const SIGNATURE: &[u8; 5] = b"B95H\x01";
const MAX_PAYLOAD: usize = 8 * 1024 * 1024;
const MAX_TEXT: usize = 1024 * 1024;
// Total number of list elements, including paragraphs and related-topic references.
const MAX_ITEMS: usize = 65_536;
const NO_TOPIC: u32 = u32::MAX;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn validate(book: &Catalog) -> io::Result<()> {
    let n = book.topics.len();
    if book.home >= n
        || book.index.iter().any(|k| k.topic >= n)
        || book.contents.iter().any(|e| e.topic.is_some_and(|t| t >= n))
    {
        return Err(invalid("invalid catalog topic reference"));
    }
    let mut next_depth = 0;
    for entry in &book.contents {
        if entry.depth > next_depth {
            return Err(invalid("missing parent in catalog contents"));
        }
        next_depth = entry.depth + 1;
    }
    for paragraph in book.topics.iter().flat_map(|t| &t.paragraphs) {
        match &paragraph.style {
            Style::Link(target) if *target >= n => return Err(invalid("invalid link target")),
            Style::Related(targets) if targets.iter().any(|&t| t >= n) => return Err(invalid("invalid related topic")),
            _ => {}
        }
    }
    Ok(())
}

struct Encoder {
    bytes: Vec<u8>,
    items_left: usize,
}

impl Encoder {
    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > MAX_PAYLOAD - self.bytes.len() {
            return Err(invalid("catalog exceeds maximum packet size"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn integer(&mut self, n: usize) -> io::Result<()> {
        let n = u32::try_from(n).map_err(|_| invalid("catalog integer out of range"))?;
        self.append(&n.to_le_bytes())
    }

    fn count(&mut self, n: usize) -> io::Result<()> {
        self.items_left = self.items_left.checked_sub(n).ok_or_else(|| invalid("too many catalog items"))?;
        self.integer(n)
    }

    fn text(&mut self, text: &str) -> io::Result<()> {
        if text.len() > MAX_TEXT {
            return Err(invalid("catalog string too long"));
        }
        self.integer(text.len())?;
        self.append(text.as_bytes())
    }

    fn paragraph(&mut self, p: &Paragraph) -> io::Result<()> {
        match &p.style {
            Style::Title => self.append(&[0])?,
            Style::Subhead => self.append(&[1])?,
            Style::Body => self.append(&[2])?,
            Style::Continuation => self.append(&[3])?,
            Style::Step(n) => self.append(&[4, *n])?,
            Style::Bullet => self.append(&[5])?,
            Style::NestedBullet => self.append(&[6])?,
            Style::Note => self.append(&[9])?,
            Style::Table { header } => self.append(&[10, u8::from(*header)])?,
            Style::Link(target) => {
                self.append(&[7])?;
                self.integer(*target)?;
            }
            Style::Related(targets) => {
                self.append(&[8])?;
                self.count(targets.len())?;
                for &target in targets {
                    self.integer(target)?;
                }
            }
        }
        self.text(&p.text)
    }
}

/// Validate and write one complete catalog. Does not close the pipe.
pub fn write(out: &mut impl Write, book: &Catalog) -> io::Result<()> {
    validate(book)?;
    let mut packet = Encoder { bytes: Vec::new(), items_left: MAX_ITEMS };
    packet.text(&book.title)?;
    packet.integer(book.home)?;
    packet.count(book.topics.len())?;
    for topic in &book.topics {
        packet.text(&topic.title)?;
        packet.append(&[u8::from(topic.welcome)])?;
        packet.count(topic.paragraphs.len())?;
        for paragraph in &topic.paragraphs {
            packet.paragraph(paragraph)?;
        }
    }
    packet.count(book.contents.len())?;
    for entry in &book.contents {
        packet.text(&entry.label)?;
        packet.integer(entry.depth)?;
        packet.integer(entry.topic.unwrap_or(NO_TOPIC as usize))?;
        packet.append(&[match entry.icon {
            EntryIcon::Book => 0,
            EntryIcon::Page => 1,
            EntryIcon::Overview => 2,
        }])?;
    }
    packet.count(book.index.len())?;
    for keyword in &book.index {
        packet.text(&keyword.label)?;
        packet.integer(keyword.topic)?;
    }
    out.write_all(SIGNATURE)?;
    out.write_all(&(packet.bytes.len() as u32).to_le_bytes())?;
    out.write_all(&packet.bytes)
}

struct Decoder<'a> {
    data: &'a [u8],
    items_left: usize,
}

impl Decoder<'_> {
    fn byte(&mut self) -> io::Result<u8> {
        let mut b = [0];
        self.data.read_exact(&mut b)?;
        Ok(b[0])
    }

    fn integer(&mut self) -> io::Result<usize> {
        let mut b = [0; 4];
        self.data.read_exact(&mut b)?;
        Ok(u32::from_le_bytes(b) as usize)
    }

    fn text(&mut self) -> io::Result<String> {
        let len = self.integer()?;
        if len > MAX_TEXT {
            return Err(invalid("catalog string too long"));
        }
        let (bytes, rest) = self.data.split_at_checked(len).ok_or_else(|| invalid("truncated catalog string"))?;
        self.data = rest;
        Ok(std::str::from_utf8(bytes).map_err(|_| invalid("invalid UTF-8 in catalog"))?.to_owned())
    }

    fn list<T>(&mut self, item: impl Fn(&mut Self) -> io::Result<T>) -> io::Result<Vec<T>> {
        let count = self.integer()?;
        self.items_left = self.items_left.checked_sub(count).ok_or_else(|| invalid("too many catalog items"))?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(item(self)?);
        }
        Ok(items)
    }

    fn paragraph(&mut self) -> io::Result<Paragraph> {
        let style = match self.byte()? {
            0 => Style::Title,
            1 => Style::Subhead,
            2 => Style::Body,
            3 => Style::Continuation,
            4 => Style::Step(self.byte()?),
            5 => Style::Bullet,
            6 => Style::NestedBullet,
            7 => Style::Link(self.integer()?),
            8 => Style::Related(self.list(Self::integer)?),
            9 => Style::Note,
            10 => Style::Table {
                header: match self.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(invalid("invalid table header flag")),
                },
            },
            _ => return Err(invalid("unknown catalog paragraph style")),
        };
        Ok(Paragraph { style, text: self.text()? })
    }

    fn topic(&mut self) -> io::Result<Topic> {
        let title = self.text()?;
        let welcome = match self.byte()? {
            0 => false,
            1 => true,
            _ => return Err(invalid("invalid welcome flag")),
        };
        Ok(Topic { title, welcome, paragraphs: self.list(Self::paragraph)? })
    }

    fn entry(&mut self) -> io::Result<Entry> {
        let label = self.text()?;
        let depth = self.integer()?;
        let target = self.integer()?;
        let topic = (target != NO_TOPIC as usize).then_some(target);
        let icon = match self.byte()? {
            0 => EntryIcon::Book,
            1 => EntryIcon::Page,
            2 => EntryIcon::Overview,
            _ => return Err(invalid("unknown catalog contents icon")),
        };
        Ok(Entry { label, depth, topic, icon })
    }
}

/// Read exactly one catalog, leaving any later viewer commands in the pipe.
pub fn read(input: &mut impl Read) -> io::Result<Catalog> {
    let mut signature = [0; SIGNATURE.len()];
    input.read_exact(&mut signature)?;
    if &signature != SIGNATURE {
        return Err(invalid("unrecognized Help catalog signature or version"));
    }
    let mut len = [0; 4];
    input.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_PAYLOAD {
        return Err(invalid("catalog exceeds maximum packet size"));
    }
    let mut bytes = vec![0; len];
    input.read_exact(&mut bytes)?;
    let mut packet = Decoder { data: &bytes, items_left: MAX_ITEMS };
    let book = Catalog {
        title: packet.text()?,
        home: packet.integer()?,
        topics: packet.list(Decoder::topic)?,
        contents: packet.list(Decoder::entry)?,
        index: packet.list(|p| Ok(Keyword { label: p.text()?, topic: p.integer()? }))?,
    };
    if !packet.data.is_empty() {
        return Err(invalid("trailing bytes inside catalog packet"));
    }
    validate(&book)?;
    Ok(book)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Catalog {
        let styles = [
            Style::Title,
            Style::Subhead,
            Style::Body,
            Style::Continuation,
            Style::Step(17),
            Style::Bullet,
            Style::NestedBullet,
            Style::Link(1),
            Style::Related(vec![1, 0]),
            Style::Note,
            Style::Table { header: true },
            Style::Table { header: false },
        ];
        Catalog {
            title: "Aide – 日本語".into(),
            home: 1,
            topics: vec![
                Topic {
                    title: "Start".into(),
                    welcome: true,
                    paragraphs: styles
                        .into_iter()
                        .map(|style| Paragraph { style, text: "One\nTwo\0Three".into() })
                        .collect(),
                },
                Topic { title: "End".into(), welcome: false, paragraphs: vec![] },
            ],
            contents: vec![
                Entry { label: "Book".into(), depth: 0, topic: None, icon: EntryIcon::Book },
                Entry { label: "Page".into(), depth: 1, topic: Some(0), icon: EntryIcon::Page },
                Entry { label: "Overview".into(), depth: 0, topic: Some(1), icon: EntryIcon::Overview },
            ],
            index: vec![Keyword { label: "Keyword".into(), topic: 0 }],
        }
    }

    #[test]
    fn catalog_round_trip_leaves_control_stream_unread() {
        let book = sample();
        let mut bytes = Vec::new();
        write(&mut bytes, &book).unwrap();
        bytes.extend_from_slice(b"contents\n");
        let mut pipe = bytes.as_slice();
        assert_eq!(read(&mut pipe).unwrap(), book);
        assert_eq!(pipe, b"contents\n");
    }

    #[test]
    fn rejects_incomplete_packets_wrong_version_and_trailing_payload() {
        let mut bytes = Vec::new();
        write(&mut bytes, &sample()).unwrap();
        for end in 0..bytes.len() {
            assert!(read(&mut &bytes[..end]).is_err());
        }
        bytes[SIGNATURE.len() - 1] = 2;
        assert!(read(&mut bytes.as_slice()).is_err());
        bytes[SIGNATURE.len() - 1] = 1;
        bytes.push(42);
        let len = (bytes.len() - SIGNATURE.len() - 4) as u32;
        bytes[SIGNATURE.len()..SIGNATURE.len() + 4].copy_from_slice(&len.to_le_bytes());
        assert!(read(&mut bytes.as_slice()).is_err());
    }

    #[test]
    fn rejects_invalid_references_and_missing_contents_ancestors() {
        let invalid_books: &[fn(&mut Catalog)] = &[
            |b| b.home = 2,
            |b| b.topics.clear(),
            |b| b.index[0].topic = 2,
            |b| b.contents[0].topic = Some(2),
            |b| b.contents[0].depth = 1,
            |b| b.contents[1].depth = 2,
            |b| b.topics[0].paragraphs[0].style = Style::Related(vec![0, 2]),
            |b| b.topics[0].paragraphs[0].style = Style::Link(9),
        ];
        for change in invalid_books {
            let mut book = sample();
            change(&mut book);
            let mut bytes = Vec::new();
            assert!(write(&mut bytes, &book).is_err());
            assert!(bytes.is_empty());
        }

        // A bad home reference on the receiving side.
        let mut book = sample();
        book.title.clear();
        let mut bytes = Vec::new();
        write(&mut bytes, &book).unwrap();
        let home = SIGNATURE.len() + 4 + 4;
        bytes[home..home + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read(&mut bytes.as_slice()).is_err());
    }

    #[test]
    fn bounds_and_tags_are_checked_before_use() {
        let mut bytes = SIGNATURE.to_vec();
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(read(&mut bytes.as_slice()).unwrap_err().kind(), io::ErrorKind::InvalidData);

        fn decoder(data: &[u8]) -> Decoder<'_> {
            Decoder { data, items_left: MAX_ITEMS }
        }
        assert!(decoder(&u32::MAX.to_le_bytes()).list(Decoder::integer).is_err());
        assert!(decoder(&u32::MAX.to_le_bytes()).text().is_err());
        assert!(decoder(&[1, 0, 0, 0, 255]).text().is_err());
        assert!(decoder(&[0, 0, 0, 0, 2]).topic().is_err());
        assert!(decoder(&[99]).paragraph().is_err());
        assert!(decoder(&[10, 2]).paragraph().is_err());
        assert!(decoder(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99]).entry().is_err());

        let mut book = sample();
        book.title = "x".repeat(MAX_TEXT + 1);
        assert!(write(&mut Vec::new(), &book).is_err());
    }
}
