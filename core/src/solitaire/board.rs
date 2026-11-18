//! The stock, waste, four foundations and seven tableau columns in SOL's client coordinates.

use crate::fb::{Framebuffer, Rect};
use crate::theme::Era;

use super::art::{self, HEIGHT, Slot, WIDTH};
use super::game::{Card, Column, Game, Pile, Source};

const FELT: u32 = 0xff008000;
const FACE_STEP: i32 = HEIGHT * 4 / 25;
const BACK_STEP: i32 = HEIGHT / 25;

#[derive(Clone, Copy, Debug)]
pub struct Board {
    pub bounds: Rect,
}

pub struct View {
    pub era: Era,
    pub back: usize,
    pub ticks: u32,
    pub drag: Option<Drag>,
    pub outline: bool,
    pub stock_edge: bool,
}

// SOL restores the destination under the rounded corners instead of erasing the
// whole pile. When cards leave a column, the newly exposed card keeps six pixels
// of the previous faces beneath its two bottom corners until the client repaints.
const LOWER_CORNERS: [(i32, i32); 6] = [(0, 0), (WIDTH - 1, 0), (0, 1), (1, 1), (WIDTH - 2, 1), (WIDTH - 1, 1)];

#[derive(Clone, Copy)]
struct Exposed {
    top: Card,
    count: usize,
    hidden: usize,
    pixels: [u32; 6],
}

#[derive(Default)]
pub struct Exposures([Option<Exposed>; 7]);

impl Exposures {
    pub fn record(&mut self, previous: &[Column; 7], game: &Game, era: Era, in_place: bool) {
        for (i, (old, new)) in previous.iter().zip(game.tableau()).enumerate() {
            if old == new {
                continue;
            }
            self.0[i] = None;
            let count = new.cards.len();
            if !in_place || count == 0 || count >= old.cards.len() || !old.cards.starts_with(&new.cards) {
                continue;
            }
            let step = if count <= old.hidden { BACK_STEP } else { FACE_STEP };
            let mut strip = Framebuffer::new(WIDTH, 2, 0);
            let mut y = step - HEIGHT + 2;
            for &card in &old.cards[count..] {
                art::face(&mut strip, 0, y, era, card, false);
                y += FACE_STEP;
                if y >= 2 {
                    break;
                }
            }
            self.0[i] = Some(Exposed {
                top: new.cards[count - 1],
                count,
                hidden: new.hidden,
                pixels: LOWER_CORNERS.map(|(x, y)| strip.get(x, y)),
            });
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Stock,
    Turn(usize),
    Cards(Source),
}

#[derive(Clone, Copy, Debug)]
pub struct Drag {
    pub source: Source,
    origin: Rect,
    grab: (i32, i32),
    pointer: (i32, i32),
}

impl Drag {
    pub fn new(source: Source, origin: Rect, x: i32, y: i32) -> Self {
        Self { source, origin, grab: (x, y), pointer: (x, y) }
    }

    pub fn update(&mut self, x: i32, y: i32) -> bool {
        std::mem::replace(&mut self.pointer, (x, y)) != (x, y)
    }

    fn rect(self) -> Rect {
        Rect::new(
            self.origin.x + self.pointer.0 - self.grab.0,
            self.origin.y + self.pointer.1 - self.grab.1,
            WIDTH,
            HEIGHT,
        )
    }
}

impl Board {
    fn gap(self) -> i32 {
        ((self.bounds.w - 7 * WIDTH) / 8).max(WIDTH / 8 + 3)
    }

    fn top(self, column: usize) -> Rect {
        let gap = self.gap();
        Rect::new(self.bounds.x + gap + column as i32 * (WIDTH + gap), self.bounds.y + 5, WIDTH, HEIGHT)
    }

    pub fn stock(self) -> Rect {
        self.top(0)
    }

    pub fn foundation_origin(self, pile: usize) -> Rect {
        self.top(3 + pile)
    }

    pub(super) fn foundation(self, pile: usize, card_index: usize) -> Rect {
        offset(self.top(3 + pile), 2 * (card_index / 4) as i32, (card_index / 4) as i32)
    }

    fn waste(self, game: &Game, index: usize) -> Rect {
        // Each ten buried cards add another thin edge beneath the current packet.
        let buried = game.waste().len() - game.visible_waste().len();
        let packet_index = index.saturating_sub(buried) as i32;
        let edge = (index.min(buried) / 10) as i32;
        offset(self.top(1), 2 * edge + packet_index * (WIDTH / 5), edge + packet_index)
    }

    fn tableau(self, game: &Game, column: usize, index: usize) -> Rect {
        let hidden = game.tableau()[column].hidden.min(index) as i32;
        offset(self.top(column), 0, HEIGHT + 6 + hidden * BACK_STEP + (index as i32 - hidden) * FACE_STEP)
    }

    pub fn source(self, game: &Game, source: Source) -> Rect {
        match source {
            Source::Waste => self.waste(game, game.waste().len().saturating_sub(1)),
            Source::Foundation(pile) => self.foundation(pile, game.foundations()[pile].len().saturating_sub(1)),
            Source::Tableau { column, index } => self.tableau(game, column, index),
        }
    }

    pub fn hit(self, game: &Game, x: i32, y: i32) -> Option<Hit> {
        if !self.bounds.contains(x, y) {
            return None;
        }
        self.hit_cards(game, x, y)
    }

    pub(super) fn hit_cards(self, game: &Game, x: i32, y: i32) -> Option<Hit> {
        let stock = self.stock();
        if Rect::new(stock.x, stock.y, WIDTH + 10, HEIGHT + 5).contains(x, y) {
            return Some(Hit::Stock);
        }
        if !game.waste().is_empty() && self.source(game, Source::Waste).contains(x, y) {
            return Some(Hit::Cards(Source::Waste));
        }
        for (pile, cards) in game.foundations().iter().enumerate() {
            let source = Source::Foundation(pile);
            if !cards.is_empty() && self.source(game, source).contains(x, y) {
                return Some(Hit::Cards(source));
            }
        }
        for (column, cards) in game.tableau().iter().enumerate() {
            for index in (0..cards.cards.len()).rev() {
                if self.tableau(game, column, index).contains(x, y) {
                    return if index >= cards.hidden {
                        Some(Hit::Cards(Source::Tableau { column, index }))
                    } else if index + 1 == cards.cards.len() {
                        Some(Hit::Turn(column))
                    } else {
                        None
                    };
                }
            }
        }
        None
    }

    pub fn destination(self, game: &Game, drag: Drag) -> Option<Pile> {
        let moving = drag.rect();
        let mut best = None;
        let mut best_area = 0;
        let mut consider = |pile, target: Rect| {
            let w = moving.right().min(target.right()) - moving.x.max(target.x);
            let h = moving.bottom().min(target.bottom()) - moving.y.max(target.y);
            if w > 0 && h > 0 && w * h > best_area && game.can_move(drag.source, pile) {
                best = Some(pile);
                best_area = w * h;
            }
        };
        for (i, cards) in game.foundations().iter().enumerate() {
            consider(Pile::Foundation(i), self.foundation(i, cards.len().saturating_sub(1)));
        }
        for (i, column) in game.tableau().iter().enumerate() {
            consider(Pile::Tableau(i), self.tableau(game, i, column.cards.len().saturating_sub(1)));
        }
        best
    }

    pub fn paint(self, fb: &mut Framebuffer, game: &Game, view: &View, exposures: &Exposures) {
        let View { era, back, ticks, drag, outline, stock_edge } = *view;
        let bounds = self.bounds;
        if bounds.w <= 0 || bounds.h <= 0 {
            return;
        }
        // GDI clips SOL's drawing to the client. In particular a long column or a card
        // dragged off the edge must never cover the menu, status bar or resize border.
        let mut felt = Framebuffer::new(bounds.w, bounds.h, FELT);
        let local = |r: Rect| (r.x - bounds.x, r.y - bounds.y);
        let (sx, sy) = local(self.stock());
        if game.stock().is_empty() {
            art::slot(&mut felt, sx, sy, era, if game.recycles_allowed() { Slot::Recycle } else { Slot::Stop });
        } else {
            let edges = ((game.stock().len() - 1) / 10) as i32;
            for edge in 0..=edges {
                art::back(&mut felt, sx + edge * 2, sy + edge, era, back);
            }
            // A shrinking stock only erases the exposed right and bottom strips.
            // The previous layer's top border remains inside the transparent corner.
            if stock_edge {
                felt.set(sx + edges * 2 + WIDTH - 1, sy + edges + 1, 0xff000000);
            }
            if ticks > 0 {
                art::animate_back(&mut felt, sx + edges * 2, sy + edges, back, ticks);
            }
        }
        let lifted = drag.filter(|_| !outline).map(|d| d.source);
        let waste = game.waste();
        let waste_len = waste.len().saturating_sub(usize::from(lifted == Some(Source::Waste)));
        for (index, &card) in waste[..waste_len].iter().enumerate() {
            let (x, y) = local(self.waste(game, index));
            art::face(&mut felt, x, y, era, card, false);
        }
        for (pile, cards) in game.foundations().iter().enumerate() {
            let count = cards.len().saturating_sub(usize::from(lifted == Some(Source::Foundation(pile))));
            if count == 0 {
                let (x, y) = local(self.foundation(pile, 0));
                art::slot(&mut felt, x, y, era, Slot::Empty);
            }
            for (index, &card) in cards[..count].iter().enumerate() {
                let (x, y) = local(self.foundation(pile, index));
                art::face(&mut felt, x, y, era, card, false);
            }
        }
        for (column, cards) in game.tableau().iter().enumerate() {
            let count = match lifted {
                Some(Source::Tableau { column: from, index }) if column == from => index,
                _ => cards.cards.len(),
            };
            for (index, &card) in cards.cards[..count].iter().enumerate() {
                let (x, y) = local(self.tableau(game, column, index));
                if index < cards.hidden {
                    art::back(&mut felt, x, y, era, back);
                } else {
                    art::face(&mut felt, x, y, era, card, false);
                }
            }
            if let Some(exposed) = exposures.0[column]
                && count == cards.cards.len()
                && exposed.count == count
                && exposed.hidden == cards.hidden
                && cards.cards.last() == Some(&exposed.top)
            {
                let (x, y) = local(self.tableau(game, column, count - 1));
                for ((dx, dy), pixel) in LOWER_CORNERS.into_iter().zip(exposed.pixels) {
                    if pixel != 0 {
                        felt.set(x + dx, y + HEIGHT - 2 + dy, pixel);
                    }
                }
            }
        }
        if let Some(drag) = drag {
            let (x, y) = local(drag.rect());
            let cards = game.movable(drag.source);
            if outline {
                if let Some(pile) = self.destination(game, drag) {
                    let target = match pile {
                        Pile::Tableau(i) => self.tableau(game, i, game.tableau()[i].cards.len().saturating_sub(1)),
                        Pile::Foundation(i) => self.foundation(i, game.foundations()[i].len().saturating_sub(1)),
                    };
                    let (tx, ty) = local(target);
                    invert(&mut felt, Rect::new(tx, ty, WIDTH, HEIGHT));
                }
                let h = HEIGHT + (cards.len().saturating_sub(1) as i32) * FACE_STEP;
                // The original traces all four inclusive line endpoints at x+width/y+height.
                invert(&mut felt, Rect::new(x, y, WIDTH + 1, 1));
                invert(&mut felt, Rect::new(x, y + h, WIDTH + 1, 1));
                invert(&mut felt, Rect::new(x, y + 1, 1, h - 1));
                invert(&mut felt, Rect::new(x + WIDTH, y + 1, 1, h - 1));
            } else {
                // SOL grabs a rectangular bitmap from the source, including the pixels
                // under every rounded corner. A moving card carries those pixels across
                // a differently colored pile until it is redrawn there on release.
                let height = HEIGHT + (cards.len().saturating_sub(1) as i32) * FACE_STEP;
                let (sx, sy) = local(drag.origin);
                let mut packet = Framebuffer::new(WIDTH, height, FELT);
                let left = (-sx).clamp(0, WIDTH);
                let right = (felt.width - sx).clamp(0, WIDTH);
                if right > left {
                    for dy in (-sy).clamp(0, height)..(felt.height - sy).clamp(0, height) {
                        let from = ((sy + dy) * felt.width + sx + left) as usize;
                        let to = (dy * WIDTH + left) as usize;
                        let n = (right - left) as usize;
                        packet.pixels[to..to + n].copy_from_slice(&felt.pixels[from..from + n]);
                    }
                }
                for (index, &card) in cards.iter().enumerate() {
                    art::face(&mut packet, 0, index as i32 * FACE_STEP, era, card, false);
                }
                felt.blit(&packet, x, y);
            }
        }
        fb.blit(&felt, bounds.x, bounds.y);
    }
}

fn offset(r: Rect, dx: i32, dy: i32) -> Rect {
    Rect::new(r.x + dx, r.y + dy, r.w, r.h)
}

fn invert(fb: &mut Framebuffer, r: Rect) {
    for y in r.y.max(0)..r.bottom().min(fb.height) {
        for x in r.x.max(0)..r.right().min(fb.width) {
            let i = (y * fb.width + x) as usize;
            fb.pixels[i] ^= 0x00ffffff;
        }
    }
}
