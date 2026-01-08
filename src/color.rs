use std::fmt::Display;

use rand::{distr::StandardUniform, prelude::Distribution};

#[derive(Debug, PartialEq, Clone)]
pub enum Color {
    RGB24(u8, u8, u8),
    RGBA32(u8, u8, u8, u8),
    W8(u8),
}

impl Color {
    pub fn to_bytes(&self) -> [u8; 4] {
        match self {
            Color::RGB24(r, g, b) => [*r, *g, *b, 0xff],
            Color::RGBA32(r, g, b, a) => [*r, *g, *b, *a],
            Color::W8(w) => [*w, *w, *w, 0xff],
        }
    }
}

impl Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Color::RGB24(r, g, b) => write!(f, "#{r:02X}{g:02X}{b:02X}FF"),
            Color::RGBA32(r, g, b, a) => write!(f, "#{r:02X}{g:02X}{b:02X}{a:02X}"),
            Color::W8(w) => write!(f, "#{w:02X}{w:02X}{w:02X}FF"),
        }
    }
}

impl Distribution<Color> for StandardUniform {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Color {
        let index: u8 = rng.random_range(0..3);
        match index {
            0 => Color::W8(rng.random()),
            1 => Color::RGB24(rng.random(), rng.random(), rng.random()),
            2 => Color::RGBA32(rng.random(), rng.random(), rng.random(), rng.random()),
            _ => unreachable!(),
        }
    }
}
