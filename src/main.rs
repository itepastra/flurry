#![feature(test)]
#![feature(sync_unsafe_cell)]

use std::{
    cell::SyncUnsafeCell,
    io::{self, Error, ErrorKind},
    iter::{self, once},
    sync::Arc,
    usize, vec,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufStream},
    net::TcpListener,
};

extern crate test;

trait Grid<I, V> {
    fn get(&self, x: I, y: I) -> Option<&V>;
    fn set(&mut self, x: I, y: I, value: V);
}

struct FlutGrid<T> {
    size_x: usize,
    size_y: usize,
    cells: Vec<T>,
}

impl<T: Clone> FlutGrid<T> {
    fn init(size_x: usize, size_y: usize, value: T) -> FlutGrid<T> {
        let mut vec = Vec::with_capacity(size_x * size_y);
        for _ in 0..(size_x * size_y) {
            vec.push(value.clone());
        }
        return FlutGrid {
            size_x,
            size_y,
            cells: vec,
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
            Some(idx) => Some(&self.cells[idx]),
        }
    }

    fn set(&mut self, x: u16, y: u16, value: T) {
        match self.index(x, y) {
            None => (),
            Some(idx) => self.cells[idx] = value,
        }
    }
}

const HELP_TEXT: u8 = 72;
const SIZE_TEXT: u8 = 83;
const PX_TEXT: u8 = 80;
const SIZE_BIN: u8 = 115;
const HELP_BIN: u8 = 104;
const LOCK: u8 = 0;
const GET_PX_BIN: u8 = 32;
const SET_PX_RGB_BIN: u8 = 128;
const SET_PX_RGBA_BIN: u8 = 129;
const SET_PX_W_BIN: u8 = 130;

const SET_PX_RGB_BIN_LENGTH: usize = 8;

fn set_pixel_rgba(grids: &mut [FlutGrid<u32>], canvas: u8, x: u16, y: u16, rgb: u32) {
    match grids.get_mut(canvas as usize) {
        Some(grid) => grid.set(x, y, rgb),
        None => (),
    }
}

fn get_pixel(grids: &mut [FlutGrid<u32>], canvas: u8, x: u16, y: u16) -> Option<&u32> {
    match grids.get_mut(canvas as usize) {
        Some(grid) => return grid.get(x, y),
        None => return None,
    }
}

async fn process_msg<T: AsyncReadExt + AsyncWriteExt + std::marker::Unpin>(
    stream: &mut T,
    grids: &mut [FlutGrid<u32>; GRID_LENGTH],
) -> io::Result<()> {
    match stream.read_u8().await {
        Ok(i) => match i {
            HELP_TEXT => todo!("HELP command check and message"),
            SIZE_TEXT => todo!("SIZE command check and message"),
            PX_TEXT => todo!("PX command handling"),
            HELP_BIN => todo!("HELP command message"),
            SIZE_BIN => todo!("SIZE command check and message"),
            LOCK => {
                let amount = stream.read_u16_le().await?;
                let command = stream.read_u8().await?;
                let lockmask = stream.read_u16().await?;
                let mut buf = vec![0; lockmask.count_ones() as usize];
                let statics = stream.read_exact(&mut buf).await?;

                match command {
                    GET_PX_BIN => todo!("GET pixel lock"),
                    SET_PX_RGB_BIN => {
                        let per = SET_PX_RGB_BIN_LENGTH - statics;
                        let mut j = 0;
                        let mut a;
                        let static_buf: Vec<Option<u8>> = (0..SET_PX_RGB_BIN_LENGTH)
                            .map(|i| match lockmask >> (15 - i) & 1 {
                                1 => {
                                    let b = Some(buf[j]);

                                    j += 1;
                                    return b;
                                }
                                0 => None,
                                _ => todo!("WTF"),
                            })
                            .collect();
                        let mut mod_buf: Vec<u8> = vec![0; per];
                        for z in 0..amount {
                            a = 0;
                            let _ = stream.read_exact(&mut mod_buf).await?;
                            let aa = static_buf
                                .iter()
                                .map(|x| *match x {
                                    Some(val) => val,
                                    None => {
                                        let b = mod_buf[a];
                                        a += 1;
                                        return b;
                                    }
                                })
                                .map(|z| z)
                                .collect::<Vec<_>>();
                            match grids.get_mut(aa[0] as usize) {
                                Some(grid) => grid.set(
                                    u16::from_le_bytes([aa[1], aa[2]]),
                                    u16::from_le_bytes([aa[3], aa[4]]),
                                    u32::from_be_bytes([aa[5], aa[6], aa[7], 0]),
                                ),
                                None => (),
                            }
                        }
                    }
                    SET_PX_RGBA_BIN => todo!("Set rgba lock"),
                    SET_PX_W_BIN => todo!("set w lock"),
                    _ => return Err(Error::from(ErrorKind::InvalidInput)),
                }

                return Ok(());
            }
            GET_PX_BIN => {
                let canvas = stream.read_u8().await?;
                let x = stream.read_u16_le().await?;
                let y = stream.read_u16_le().await?;
                match get_pixel(grids, canvas, x, y) {
                    None => (),
                    Some(color) => {
                        stream
                            .write_all(
                                &once(canvas)
                                    .chain(x.to_le_bytes())
                                    .chain(y.to_le_bytes())
                                    .chain(color.to_be_bytes())
                                    .collect::<Vec<_>>(),
                            )
                            .await?;
                        ()
                    }
                }
                return Ok(());
            }
            SET_PX_RGB_BIN => {
                let canvas = stream.read_u8().await?;
                let x = stream.read_u16_le().await?;
                let y = stream.read_u16_le().await?;
                let r = stream.read_u8().await?;
                let g = stream.read_u8().await?;
                let b = stream.read_u8().await?;
                let rgb = (r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8;
                set_pixel_rgba(grids, canvas, x, y, rgb);
                return Ok(());
            }
            SET_PX_RGBA_BIN => todo!("SET rgba"),
            SET_PX_W_BIN => todo!("SET w"),
            _ => {
                eprintln!("received illegal command: {}", i);
                return Err(Error::from(ErrorKind::InvalidInput));
            }
        },
        Err(err) => {
            eprintln!("{}", err);
            return Err(err);
        }
    }
}

async fn process_socket<T: AsyncRead + AsyncWrite + std::marker::Unpin>(
    socket: T,
    grids: &mut [FlutGrid<u32>; GRID_LENGTH],
) -> io::Result<()> {
    let mut stream = BufStream::new(socket);
    loop {
        match process_msg(&mut stream, grids).await {
            Ok(()) => (),
            Err(e) => return Err(e),
        }
    }
}

const GRID_LENGTH: usize = 1;

#[tokio::main]
async fn main() -> io::Result<()> {
    let grids = [FlutGrid::init(800, 600, 0xff00ff)];
    assert_eq!(grids.len(), GRID_LENGTH);

    let flut_listener = TcpListener::bind("0.0.0.0:7791").await?;
    let asuc = Arc::new(SyncUnsafeCell::new(grids));

    loop {
        let (socket, _) = flut_listener.accept().await?;
        let asuc = asuc.clone();
        let _ = tokio::spawn(async move {
            let grids = unsafe { asuc.get().as_mut().unwrap() };
            match process_socket(socket, grids).await {
                Ok(()) => return Ok(()),
                Err(err) => return Err(err),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    #[tokio::test]
    async fn test_grid_init_values() {
        let grid = FlutGrid::init(3, 3, 0);

        assert_eq!(grid.cells, vec![0, 0, 0, 0, 0, 0, 0, 0, 0])
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
        assert_eq!(grid.cells, vec![0, 0, 0, 0, 255, 256, 0, 0, 0])
    }

    #[tokio::test]
    async fn test_grid_set_out_of_range() {
        let mut grid = FlutGrid::init(3, 3, 0);
        grid.set(1, 1, 255);
        grid.set(3, 1, 256);
        assert_eq!(grid.cells, vec![0, 0, 0, 0, 255, 0, 0, 0, 0])
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

    #[tokio::test]
    async fn test_set_rgb_bin() {
        let mut grids = [FlutGrid::init(800, 600, 0xFF00FF)];
        let writer = tokio_test::io::Builder::new()
            .read(&[SET_PX_RGB_BIN, 0, 16, 0, 32, 0, 0, 0, 0])
            .read(&[SET_PX_RGB_BIN, 0, 16, 0, 33, 0, 2, 3, 5])
            .build();
        let res = process_socket(writer, &mut grids).await;
        match res {
            Ok(()) => (),
            Err(err) => eprintln!("{}", err),
        };
        assert_eq!(grids[0].get(16, 32), Some(&0x00000000));
        assert_eq!(grids[0].get(16, 33), Some(&0x02030500));
    }

    #[tokio::test]
    async fn test_set_rgb_lock() {
        let mut grids = [FlutGrid::init(800, 600, 0xFF00FF)];
        let writer = tokio_test::io::Builder::new()
            .read(&[
                LOCK,
                3,
                0,
                SET_PX_RGB_BIN,
                0b1011_1110,
                0b0000_0000,
                0,
                0,
                0,
                0,
                2,
                3,
            ])
            .read(&[100, 4])
            .read(&[101, 5])
            .read(&[102, 6])
            .build();
        let res = process_socket(writer, &mut grids).await;
        match res {
            Ok(()) => (),
            Err(err) => eprintln!("{}", err),
        };

        assert_eq!(grids[0].get(100, 0), Some(&0x02030400));
        assert_eq!(grids[0].get(101, 0), Some(&0x02030500));
        assert_eq!(grids[0].get(102, 0), Some(&0x02030600));
    }
}
