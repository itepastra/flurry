#![feature(test)]
#![feature(sync_unsafe_cell)]
#![feature(if_let_guard)]

use std::sync::atomic::AtomicU64;

pub use color::Color;
use grid::Grid;

pub mod config;
pub mod flutclient;
pub mod grid;
pub mod protocols;
pub mod utils;

mod color;

pub type Canvas = u8;
pub type Coordinate = u16;

pub static COUNTER: AtomicU64 = AtomicU64::new(0);

fn set_pixel_rgba(
    grids: &[grid::Flut<u32>],
    canvas: Canvas,
    x: Coordinate,
    y: Coordinate,
    rgb: u32,
) {
    if let Some(grid) = grids.get(canvas as usize) {
        grid.set(x, y, rgb);
    }
}

fn get_pixel(
    grids: &[grid::Flut<u32>],
    canvas: Canvas,
    x: Coordinate,
    y: Coordinate,
) -> Option<&u32> {
    match grids.get(canvas as usize) {
        Some(grid) => grid.get(x, y),
        None => None,
    }
}

#[inline]
fn increment_counter(amount: u64) {
    COUNTER.fetch_add(amount, std::sync::atomic::Ordering::Relaxed);
}

#[derive(Debug, PartialEq)]
pub enum Protocol {
    Text,
    Binary,
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Help,
    Size(Canvas),
    GetPixel(Canvas, Coordinate, Coordinate),
    SetPixel(Canvas, Coordinate, Coordinate, Color),
    ChangeCanvas(Canvas),
    ChangeProtocol(Protocol),
}

#[derive(Debug, PartialEq)]
pub enum Response {
    Help,
    Size(Coordinate, Coordinate),
    GetPixel(Coordinate, Coordinate, [u8; 3]),
}
