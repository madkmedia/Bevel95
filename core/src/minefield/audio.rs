//! Windows 2000 WINMINE's clock (WAVE 432), win (433) and explosion (434).
//! The assets are the raw 8-bit unsigned mono samples cut from the owner's reference copy.
//! Wrap them in their original 22,050 Hz RIFF format for playback from static memory.

#[derive(Clone, Copy)]
pub enum Cue {
    Tick,
    Win,
    Lose,
}

impl Cue {
    pub fn wave(self) -> &'static [u8] {
        const TICK: &[u8; 1316] = crate::art::exactly("minesweeper/sound-tick", crate::art::Look::Win2000);
        const WIN: &[u8; 37916] = crate::art::exactly("minesweeper/sound-win", crate::art::Look::Win2000);
        const LOSE: &[u8; 25667] = crate::art::exactly("minesweeper/sound-lose", crate::art::Look::Win2000);
        static TICK_WAVE: [u8; 44 + TICK.len()] = wave(TICK);
        static WIN_WAVE: [u8; 44 + WIN.len()] = wave(WIN);
        static LOSE_WAVE: [u8; 44 + LOSE.len()] = wave(LOSE);
        match self {
            Self::Tick => &TICK_WAVE,
            Self::Win => &WIN_WAVE,
            Self::Lose => &LOSE_WAVE,
        }
    }
}

const fn wave<const N: usize>(samples: &[u8]) -> [u8; N] {
    let rate = 22_050u32.to_le_bytes();
    let fields: &[&[u8]] = &[
        b"RIFF",
        &(N as u32 - 8).to_le_bytes(),
        b"WAVEfmt ",
        &16u32.to_le_bytes(),
        &1u16.to_le_bytes(), // PCM
        &1u16.to_le_bytes(), // mono
        &rate,
        &rate, // sample rate and bytes per second at eight bits per sample
        &1u16.to_le_bytes(),
        &8u16.to_le_bytes(), // block alignment and sample width
        b"data",
        &(samples.len() as u32).to_le_bytes(),
        samples,
    ];
    let mut bytes = [0; N];
    let mut remaining: &mut [u8] = &mut bytes;
    let mut i = 0;
    while i < fields.len() {
        let (field, tail) = remaining.split_at_mut(fields[i].len());
        field.copy_from_slice(fields[i]);
        remaining = tail;
        i += 1;
    }
    assert!(remaining.is_empty());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_bytes_match_the_original_resources() {
        // FNV-1a over the *complete* WAVE resources from the Windows 2000 installation disc.
        for (cue, expected) in
            [(Cue::Tick, 0xae9f5c978b0db898), (Cue::Win, 0x75835cd15c696145), (Cue::Lose, 0xa200bc8933b2d8f3)]
        {
            let hash = cue
                .wave()
                .iter()
                .fold(0xcbf29ce484222325u64, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3));
            assert_eq!(hash, expected);
        }
    }
}
