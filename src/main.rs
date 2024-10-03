#![feature(test)]
#![feature(sync_unsafe_cell)]

mod grid;

use std::{
    cell::SyncUnsafeCell,
    io::{self, Error, ErrorKind},
    iter::once,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use grid::{FlutGrid, Grid};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
};

extern crate test;

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
const GRID_LENGTH: usize = 1;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn set_pixel_rgba(grids: &[grid::FlutGrid<u32>], canvas: u8, x: u16, y: u16, rgb: u32) {
    match grids.get(canvas as usize) {
        Some(grid) => grid.set(x, y, rgb),
        None => (),
    }
}

fn get_pixel(grids: &[grid::FlutGrid<u32>], canvas: u8, x: u16, y: u16) -> Option<&u32> {
    match grids.get(canvas as usize) {
        Some(grid) => return grid.get(x, y),
        None => return None,
    }
}

fn increment_counter() {
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

async fn process_lock<
    R: AsyncReadExt + std::marker::Unpin,
    W: AsyncWriteExt + std::marker::Unpin,
>(
    reader: &mut R,
    _writer: &mut W,
    grids: &[grid::FlutGrid<u32>; GRID_LENGTH],
) -> io::Result<()> {
    let amount = reader.read_u16_le().await?;
    let command = reader.read_u8().await?;
    let lockmask = reader.read_u16().await?;
    let mut buf = vec![0; lockmask.count_ones() as usize];
    let statics = reader.read_exact(&mut buf).await?;

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
                    k => panic!("WTF, how is {} not 0 or 1", k),
                })
                .collect();
            let mut mod_buf: Vec<u8> = vec![0; per];
            for _ in 0..amount {
                a = 0;
                let _ = reader.read_exact(&mut mod_buf).await?;
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
                match grids.get(aa[0] as usize) {
                    Some(grid) => {
                        grid.set(
                            u16::from_le_bytes([aa[1], aa[2]]),
                            u16::from_le_bytes([aa[3], aa[4]]),
                            u32::from_be_bytes([aa[5], aa[6], aa[7], 0xff]),
                        );
                        increment_counter()
                    }
                    None => (),
                }
            }
        }
        SET_PX_RGBA_BIN => todo!("Set rgba lock"),
        SET_PX_W_BIN => todo!("set w lock"),
        _ => {
            eprintln!("not a cmd");
            return Err(Error::from(ErrorKind::InvalidInput));
        }
    }

    return Ok(());
}

async fn process_msg<
    R: AsyncReadExt + std::marker::Unpin,
    W: AsyncWriteExt + std::marker::Unpin,
>(
    reader: &mut R,
    writer: &mut W,
    grids: &[grid::FlutGrid<u32>; GRID_LENGTH],
) -> io::Result<()> {
    let fst = reader.read_u8().await;
    match fst {
        Ok(i) => match i {
            HELP_TEXT => todo!("HELP command check and message"),
            SIZE_TEXT => todo!("SIZE command check and message"),
            PX_TEXT => todo!("PX command handling"),
            HELP_BIN => todo!("HELP command message"),
            SIZE_BIN => todo!("SIZE command check and message"),
            LOCK => process_lock(reader, writer, grids).await,
            GET_PX_BIN => {
                let canvas = reader.read_u8().await?;
                let x = reader.read_u16_le().await?;
                let y = reader.read_u16_le().await?;
                match get_pixel(grids, canvas, x, y) {
                    None => (),
                    Some(color) => {
                        let towrite = &once(GET_PX_BIN)
                            .chain(once(canvas))
                            .chain(x.to_le_bytes())
                            .chain(y.to_le_bytes())
                            .chain(color.to_be_bytes().into_iter().take(3))
                            .collect::<Vec<_>>();
                        writer.write_all(towrite).await?;
                    }
                }
                return Ok(());
            }
            SET_PX_RGB_BIN => set_px_rgb_bin(reader, grids).await,
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

async fn rgb_bin_read<R: AsyncReadExt + std::marker::Unpin>(
    reader: &mut R,
) -> io::Result<(u8, u16, u16, u8, u8, u8)> {
    let canvas = reader.read_u8().await?;
    let x = reader.read_u16_le().await?;
    let y = reader.read_u16_le().await?;
    let r = reader.read_u8().await?;
    let g = reader.read_u8().await?;
    let b = reader.read_u8().await?;
    return Ok((canvas, x, y, r, g, b));
}

async fn set_px_rgb_bin<R: AsyncReadExt + std::marker::Unpin>(
    reader: &mut R,
    grids: &[grid::FlutGrid<u32>; GRID_LENGTH],
) -> io::Result<()> {
    let (canvas, x, y, r, g, b) = rgb_bin_read(reader).await?;
    let rgb = u32::from_be_bytes([r, g, b, 0xff]);
    set_pixel_rgba(grids, canvas, x, y, rgb);
    increment_counter();
    return Ok(());
}

async fn process_socket<W, R>(
    reader: R,
    writer: W,
    grids: &[grid::FlutGrid<u32>; GRID_LENGTH],
) -> io::Result<()>
where
    W: AsyncWriteExt + std::marker::Unpin,
    R: AsyncReadExt + std::marker::Unpin,
{
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);
    loop {
        match process_msg(&mut reader, &mut writer, grids).await {
            Ok(()) => {}
            Err(err) if err.kind() == ErrorKind::UnexpectedEof => {
                let _ = writer.flush().await;
                return Ok(());
            }
            Err(e) => {
                eprintln!("error with kind {}", e.kind());
                return Err(e);
            }
        }
    }
}

async fn listen_handle() {
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
        interval.tick().await;
        let cnt = COUNTER.load(std::sync::atomic::Ordering::Relaxed);
        println!("{} pixels were changed", cnt);
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    println!("created grids");
    let grids: Arc<[FlutGrid<u32>; GRID_LENGTH]> =
        [grid::FlutGrid::init(800, 600, 0xff00ffff)].into();

    let flut_listener = TcpListener::bind("0.0.0.0:7791").await?;
    println!("bound flut listener");

    let web_listener = TcpListener::bind("0.0.0.0:7792").await?;

    println!("bound web listener");
    let _ = tokio::spawn(listen_handle());

    loop {
        let (mut socket, _) = flut_listener.accept().await?;
        let grids = grids.clone();
        let _ = tokio::spawn(async move {
            let (reader, writer) = socket.split();
            match process_socket(reader, writer, &grids).await {
                Ok(()) => return Ok(()),
                Err(err) => return Err(err),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::FlutGrid;
    use tokio_test::assert_ok;

    #[tokio::test]
    async fn test_set_rgb_bin() {
        let mut grids = [FlutGrid::init(800, 600, 0xFF00FF)];
        let reader = tokio_test::io::Builder::new()
            .read(&[SET_PX_RGB_BIN, 0, 16, 0, 32, 0, 0, 0, 0])
            .read(&[SET_PX_RGB_BIN, 0, 16, 0, 33, 0, 2, 3, 5])
            .build();
        let writer = tokio_test::io::Builder::new().build();
        assert_ok!(process_socket(reader, writer, &mut grids).await);
        assert_eq!(grids[0].get(16, 32), Some(&0x000000ff));
        assert_eq!(grids[0].get(16, 33), Some(&0x020305ff));
    }

    #[tokio::test]
    async fn test_set_rgb_lock() {
        let mut grids = [FlutGrid::init(800, 600, 0xFF00FF)];
        let reader = tokio_test::io::Builder::new()
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
        let writer = tokio_test::io::Builder::new().build();
        assert_ok!(process_socket(reader, writer, &mut grids).await);

        assert_eq!(grids[0].get(100, 0), Some(&0x020304ff));
        assert_eq!(grids[0].get(101, 0), Some(&0x020305ff));
        assert_eq!(grids[0].get(102, 0), Some(&0x020306ff));
    }

    #[tokio::test]
    async fn test_get_rgb_bin() {
        let mut grids = [FlutGrid::init(800, 600, 0xFF00F0FF)];
        let reader = tokio_test::io::Builder::new()
            .read(&[GET_PX_BIN, 0, 15, 0, 21, 0])
            .read(&[GET_PX_BIN, 0, 16, 0, 21, 0])
            .read(&[GET_PX_BIN, 0, 17, 0, 21, 0])
            .build();
        let writer = tokio_test::io::Builder::new()
            .write(&[GET_PX_BIN, 0, 15, 0, 21, 0, 0xff, 0x00, 0xf0])
            .write(&[GET_PX_BIN, 0, 16, 0, 21, 0, 0xff, 0x00, 0xf0])
            .write(&[GET_PX_BIN, 0, 17, 0, 21, 0, 0xff, 0x00, 0xf0])
            .build();
        assert_ok!(process_socket(reader, writer, &mut grids).await);
        assert_eq!(grids[0].get(15, 21), Some(&0xff00f0ff));
    }
}
