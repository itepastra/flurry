#[cfg(feature = "auth")]
use std::cell::SyncUnsafeCell;

#[cfg(feature = "auth")]
use image::{GenericImageView, Rgba};

#[cfg(feature = "auth")]
use crate::Coordinate;

#[cfg(feature = "auth")]
pub(crate) type User = u32;

#[cfg(feature = "auth")]
pub(crate) struct BlameMap {
    size_x: usize,
    size_y: usize,
    cells: SyncUnsafeCell<Vec<User>>,
}

#[cfg(feature = "auth")]
impl BlameMap {
    fn index(&self, x: Coordinate, y: Coordinate) -> Option<usize> {
        let x = x as usize;
        let y = y as usize;
        if x >= self.size_x || y >= self.size_y {
            return None;
        }
        Some((y * self.size_x) + x)
    }

    pub(crate) fn new(size_x: usize, size_y: usize) -> Self {
        let mut cells = Vec::with_capacity(size_x * size_y);
        for _y in 0..size_y {
            for _x in 0..size_x {
                cells.push(0);
            }
        }
        BlameMap {
            size_x,
            size_y,
            cells: cells.into(),
        }
    }

    pub(crate) fn set_blame(&self, x: Coordinate, y: Coordinate, user: User) {
        match self.index(x, y) {
            None => (),
            Some(idx) => unsafe { (*self.cells.get())[idx] = user },
        }
    }
}

#[cfg(feature = "auth")]
impl GenericImageView for BlameMap {
    type Pixel = Rgba<u8>;

    fn dimensions(&self) -> (u32, u32) {
        (self.size_x as u32, self.size_y as u32)
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        let idx = (y as usize) * self.size_x + (x as usize);
        let pixel = unsafe { (*self.cells.get())[idx] };
        let [r, g, b, a] = pixel.to_be_bytes();
        Rgba::from([r, g, b, a])
    }
}
