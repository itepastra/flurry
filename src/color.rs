use std::fmt::Display;

use rand::{distributions::Standard, prelude::Distribution};

#[derive(Debug, PartialEq)]
pub enum Color {
    RGB24(u8, u8, u8),
    RGBA32(u8, u8, u8, u8),
    W8(u8),
}

impl Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Color::RGB24(r, g, b) => write!(f, "#{:02X}{:02X}{:02X}FF", r, g, b),
            Color::RGBA32(r, g, b, a) => write!(f, "#{:02X}{:02X}{:02X}{:02X}", r, g, b, a),
            Color::W8(w) => write!(f, "#{:02X}{:02X}{:02X}FF", w, w, w),
        }
    }
}

impl Distribution<Color> for Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Color {
        let index: u8 = rng.gen_range(0..3);
        match index {
            0 => Color::W8(rng.gen()),
            1 => Color::RGB24(rng.gen(), rng.gen(), rng.gen()),
            2 => Color::RGBA32(rng.gen(), rng.gen(), rng.gen(), rng.gen()),
            _ => unreachable!(),
        }
    }
}
