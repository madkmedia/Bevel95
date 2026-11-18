//! The winning deal leaves a trail of card faces across the felt. SOL draws one
//! card at a time, visiting the four foundations in turn for each rank from king to ace.

use crate::fb::{Framebuffer, Rect};
use crate::theme::Era;

use super::art;
use super::game::Card;

struct Flight {
    x: i32,
    y: i32,
    dx: i32,
    vy: i32,
}

pub struct Victory {
    era: Era,
    bounds: Rect,
    origins: [(i32, i32); 4],
    kings: [Card; 4],
    dealt: usize,
    random: u32,
    flight: Option<Flight>,
    trails: Framebuffer,
}

impl Victory {
    /// Both the play area and the four foundation positions are window coordinates.
    pub fn new(era: Era, bounds: Rect, origins: [(i32, i32); 4], kings: [Card; 4], seed: u64) -> Self {
        Self {
            era,
            bounds,
            origins,
            kings,
            dealt: 0,
            random: seed as u32,
            flight: None,
            trails: Framebuffer::new(bounds.w.max(0), bounds.h.max(0), 0),
        }
    }

    // The original chooses both velocities with the Microsoft C runtime generator.
    fn speed(&mut self) -> i32 {
        (super::game::random(&mut self.random) % 110) as i32
    }

    /// Stamp a single position and advance the flight. Return false when the last ace leaves.
    pub fn advance(&mut self) -> bool {
        if self.dealt == 52 {
            return false;
        }
        let pile = self.dealt % 4;
        if self.flight.is_none() {
            let horizontal = self.speed() - 65;
            let dx = if horizontal.abs() < 15 { -2 } else { horizontal / 10 };
            let (x, y) = self.origins[pile];
            self.flight = Some(Flight { x: x - self.bounds.x, y: y - self.bounds.y, dx, vy: self.speed() - 75 });
        }
        let flight = self.flight.as_mut().expect("the card has launched");
        let card = Card { rank: 13 - (self.dealt / 4) as u8, ..self.kings[pile] };
        art::face(&mut self.trails, flight.x, flight.y, self.era, card, false);
        flight.x += flight.dx;
        flight.y += flight.vy / 10;
        flight.vy += 3;
        // SOL's fixed-point bounce retains 8/10 of the vertical velocity and lets
        // the lowest part of the trail extend past the edge where the canvas clips it.
        if flight.y > self.bounds.h - art::HEIGHT && flight.vy > 0 {
            flight.vy = -8 * flight.vy / 10;
        }
        if flight.x <= -art::WIDTH || flight.x >= self.bounds.w {
            self.dealt += 1;
            self.flight = None;
        }
        self.dealt < 52
    }

    /// Transparent pixels expose the board under the trajectory.
    pub fn paint(&self, fb: &mut Framebuffer) {
        for (y, row) in self.trails.pixels.chunks(self.trails.width.max(1) as usize).enumerate() {
            let dest_y = self.bounds.y + y as i32;
            if !(0..fb.height).contains(&dest_y) {
                continue;
            }
            let left = (-self.bounds.x).clamp(0, self.trails.width) as usize;
            let right = (fb.width - self.bounds.x).clamp(0, self.trails.width) as usize;
            for (x, &pixel) in row.iter().enumerate().take(right).skip(left) {
                if pixel >> 24 != 0 {
                    fb.pixels[(dest_y * fb.width + self.bounds.x + x as i32) as usize] = pixel;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solitaire::game::Suit;

    #[test]
    fn cards_leave_a_trail_inside_the_felt_and_stop_after_all_four_aces() {
        let bounds = Rect::new(4, 40, 144, 104);
        let origins = [(24, 41), (44, 41), (64, 41), (74, 41)];
        let kings = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades].map(|suit| Card { rank: 13, suit });
        let mut victory = Victory::new(Era::Win95, bounds, origins, kings, 1);
        assert!(victory.advance());
        // With seed 1 the first card travels two pixels to the left and two down.
        let first = victory.flight.as_ref().unwrap();
        assert_eq!((first.x, first.y), (18, 3));
        let mut steps = 1;
        while victory.advance() {
            steps += 1;
            assert!(steps < 12_000, "a card must eventually leave the board");
        }
        assert_eq!(victory.dealt, 52);
        assert!(!victory.advance());
        let paper = 0xff008000;
        let mut fb = Framebuffer::new(152, 150, paper);
        victory.paint(&mut fb);
        assert!(fb.pixels.iter().any(|&pixel| pixel != paper));
        assert!(fb.pixels.chunks(fb.width as usize).enumerate().all(|(y, row)| {
            row.iter().enumerate().all(|(x, &p)| bounds.contains(x as i32, y as i32) || p == paper)
        }));
    }
}
