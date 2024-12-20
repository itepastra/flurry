use std::{
    cell::SyncUnsafeCell,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{RwLock, RwLockReadGuard},
};

use image::{GenericImageView, Rgb};

use crate::Coordinate;

pub trait Grid<I, V> {
    fn get(&self, x: I, y: I) -> Option<&V>;
    #[allow(dead_code)]
    fn get_unchecked(&self, x: I, y: I) -> &V;
    fn set(&self, x: I, y: I, value: V);
}

pub struct Flut<T> {
    size_x: usize,
    size_y: usize,
    cells: SyncUnsafeCell<Vec<T>>,
    last_hash: SyncUnsafeCell<u64>,
    jpgbuf: RwLock<Vec<u8>>,
}

impl<T: Clone> Flut<T> {
    pub fn init(size_x: usize, size_y: usize, value: T) -> Flut<T> {
        let mut vec = Vec::with_capacity(size_x * size_y);
        for _ in 0..(size_x * size_y) {
            vec.push(value.clone());
        }
        Flut {
            size_x,
            size_y,
            cells: vec.into(),
            last_hash: 0.into(),
            jpgbuf: RwLock::new(Vec::new()),
        }
    }

    pub fn get_size(&self) -> (usize, usize) {
        (self.size_x, self.size_y)
    }
}

impl<T> Flut<T> {
    fn index(&self, x: Coordinate, y: Coordinate) -> Option<usize> {
        let x = x as usize;
        let y = y as usize;
        if x >= self.size_x || y >= self.size_y {
            return None;
        }
        Some((y * self.size_x) + x)
    }
    pub fn read_jpg_buffer(&self) -> RwLockReadGuard<'_, Vec<u8>> {
        self.jpgbuf.read().expect("RWlock didn't exit nicely")
    }
}

impl<T> Grid<Coordinate, T> for Flut<T> {
    fn get(&self, x: Coordinate, y: Coordinate) -> Option<&T> {
        self.index(x, y)
            .map(|idx| unsafe { &(*self.cells.get())[idx] })
    }

    fn set(&self, x: Coordinate, y: Coordinate, value: T) {
        match self.index(x, y) {
            None => (),
            Some(idx) => unsafe { (*self.cells.get())[idx] = value },
        }
    }

    fn get_unchecked(&self, x: Coordinate, y: Coordinate) -> &T {
        let idx = y as usize * self.size_x + x as usize;
        unsafe { &(*self.cells.get())[idx] }
    }
}

impl GenericImageView for Flut<u32> {
    type Pixel = Rgb<u8>;

    fn dimensions(&self) -> (u32, u32) {
        let (x, y) = self.get_size();
        (x as u32, y as u32)
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        let pixel = self.get_unchecked(x as Coordinate, y as Coordinate);
        let [r, g, b, _a] = pixel.to_be_bytes();
        Rgb::from([r, g, b])
    }
}

impl Flut<u32> {
    pub fn check_changed(&self) -> bool {
        let previous = unsafe { *self.last_hash.get() };
        let mut hasher = DefaultHasher::new();
        unsafe { (*self.cells.get()).hash(&mut hasher) };
        if hasher.finish() == previous {
            return false;
        }
        unsafe { *self.last_hash.get() = hasher.finish() }
        true
    }

    pub fn update_jpg_buffer(&self) {
        if !self.check_changed() {
            return;
        }
        let mut jpgbuf = self.jpgbuf.write().expect("Could not get write RWlock");
        jpgbuf.clear();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut *jpgbuf, 50);
        let subimage = self.view(0, 0, self.width(), self.height()).to_image();
        match subimage.write_with_encoder(encoder) {
            Ok(_) => {}
            Err(err) => tracing::error!("Error writing jpeg buffer: {:?}", err),
        }
    }
}

#[cfg(test)]
#[allow(clippy::needless_return)]
mod tests {
    use super::Flut;
    use super::Grid;

    #[tokio::test]
    async fn test_grid_init_values() {
        let grid = Flut::init(3, 3, 0);

        assert_eq!(grid.cells.into_inner(), vec![0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn test_grid_init_size() {
        let grid = Flut::init(800, 600, 0);

        assert_eq!(grid.size_x, 800);
        assert_eq!(grid.size_y, 600);
    }

    #[tokio::test]
    async fn test_grid_set() {
        let grid = Flut::init(3, 3, 0);
        grid.set(1, 1, 255);
        grid.set(2, 1, 256);
        assert_eq!(grid.cells.into_inner(), vec![0, 0, 0, 0, 255, 256, 0, 0, 0]);
    }

    #[tokio::test]
    async fn test_grid_set_out_of_range() {
        let grid = Flut::init(3, 3, 0);
        grid.set(1, 1, 255);
        grid.set(3, 1, 256);
        assert_eq!(grid.cells.into_inner(), vec![0, 0, 0, 0, 255, 0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn test_grid_get() {
        let grid = Flut::init(3, 3, 0);
        grid.set(1, 2, 222);
        assert_eq!(grid.get(1, 2), Some(&222));
    }

    #[tokio::test]
    async fn test_grid_get_out_of_range() {
        let grid = Flut::init(3, 3, 0);
        grid.set(3, 1, 256);
        assert_eq!(grid.get(3, 1), None);
        assert_eq!(grid.get(1, 2), Some(&0));
    }
}
