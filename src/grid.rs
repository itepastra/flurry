use std::cell::SyncUnsafeCell;

pub trait Grid<I, V> {
    fn get(&self, x: I, y: I) -> Option<&V>;
    fn get_unchecked(&self, x: I, y: I) -> &V;
    fn set(&self, x: I, y: I, value: V);
}

pub struct FlutGrid<T> {
    size_x: usize,
    size_y: usize,
    cells: SyncUnsafeCell<Vec<T>>,
}

impl<T: Clone> FlutGrid<T> {
    pub fn init(size_x: usize, size_y: usize, value: T) -> FlutGrid<T> {
        let mut vec = Vec::with_capacity(size_x * size_y);
        for _ in 0..(size_x * size_y) {
            vec.push(value.clone());
        }
        return FlutGrid {
            size_x,
            size_y,
            cells: vec.into(),
        };
    }
}

impl<T> FlutGrid<T> {
    fn index(&self, x: u16, y: u16) -> Option<usize> {
        let x = x as usize;
        let y = y as usize;
        if x >= self.size_x || y >= self.size_y {
            return None;
        }
        return Some((y * self.size_x) + x);
    }
}

impl<T> Grid<u16, T> for FlutGrid<T> {
    fn get(&self, x: u16, y: u16) -> Option<&T> {
        match self.index(x, y) {
            None => None,
            Some(idx) => Some(unsafe { &(*self.cells.get())[idx] }),
        }
    }

    fn set(&self, x: u16, y: u16, value: T) {
        match self.index(x, y) {
            None => (),
            Some(idx) => unsafe { (*self.cells.get())[idx] = value },
        }
    }

    fn get_unchecked(&self, x: u16, y: u16) -> &T {
        let idx = y as usize * self.size_x + x as usize;
        return unsafe { &(*self.cells.get())[idx] };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::FlutGrid;
    use test::Bencher;

    #[tokio::test]
    async fn test_grid_init_values() {
        let grid = FlutGrid::init(3, 3, 0);

        assert_eq!(grid.cells.into_inner(), vec![0, 0, 0, 0, 0, 0, 0, 0, 0])
    }

    #[tokio::test]
    async fn test_grid_init_size() {
        let grid = FlutGrid::init(800, 600, 0);

        assert_eq!(grid.size_x, 800);
        assert_eq!(grid.size_y, 600);
    }

    #[tokio::test]
    async fn test_grid_set() {
        let mut grid = FlutGrid::init(3, 3, 0);
        grid.set(1, 1, 255);
        grid.set(2, 1, 256);
        assert_eq!(grid.cells.into_inner(), vec![0, 0, 0, 0, 255, 256, 0, 0, 0])
    }

    #[tokio::test]
    async fn test_grid_set_out_of_range() {
        let mut grid = FlutGrid::init(3, 3, 0);
        grid.set(1, 1, 255);
        grid.set(3, 1, 256);
        assert_eq!(grid.cells.into_inner(), vec![0, 0, 0, 0, 255, 0, 0, 0, 0])
    }

    #[tokio::test]
    async fn test_grid_get() {
        let mut grid = FlutGrid::init(3, 3, 0);
        grid.set(1, 2, 222);
        assert_eq!(grid.get(1, 2), Some(&222));
    }

    #[tokio::test]
    async fn test_grid_get_out_of_range() {
        let mut grid = FlutGrid::init(3, 3, 0);
        grid.set(3, 1, 256);
        assert_eq!(grid.get(3, 1), None);
        assert_eq!(grid.get(1, 2), Some(&0));
    }

    #[bench]
    fn bench_init(b: &mut Bencher) {
        b.iter(|| FlutGrid::init(800, 600, 0 as u32))
    }

    #[bench]
    fn bench_set(b: &mut Bencher) {
        let mut grid = FlutGrid::init(800, 600, 0 as u32);
        b.iter(|| {
            let x = test::black_box(293);
            let y = test::black_box(222);
            let color = test::black_box(293923);
            grid.set(x, y, color);
        })
    }

    #[bench]
    fn bench_get(b: &mut Bencher) {
        let grid = FlutGrid::init(800, 600, 0 as u32);
        b.iter(|| {
            let x = test::black_box(293);
            let y = test::black_box(222);
            grid.get(x, y)
        })
    }
}
