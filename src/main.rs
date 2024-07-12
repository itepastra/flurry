#![feature(test)]
#![feature(sync_unsafe_cell)]

use std::{
    cell::SyncUnsafeCell,
    hash::{DefaultHasher, Hash, Hasher},
    io::{self, Cursor, Error, ErrorKind},
    iter::once,
    pin::Pin,
    sync::{atomic::AtomicU64, Arc, RwLock},
    task::Poll,
    time::Duration,
};

use futures::Stream;
use hyper::body::Frame;
use image::{GenericImageView, Rgb};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
};

extern crate test;

trait Grid<I, V> {
    fn get(&self, x: I, y: I) -> Option<&V>;
    fn get_unchecked(&self, x: I, y: I) -> &V;
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

impl GenericImageView for FlutGrid<u32> {
    type Pixel = Rgb<u8>;

    fn dimensions(&self) -> (u32, u32) {
        return (self.size_x as u32, self.size_y as u32);
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        let pix = self.get_unchecked(x as u16, y as u16);
        let [a, b, c, _] = pix.to_be_bytes();
        return Rgb([a, b, c]);
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

    fn get_unchecked(&self, x: u16, y: u16) -> &T {
        let idx = y as usize * self.size_x + x as usize;
        return &self.cells[idx];
    }
}

#[derive(Clone)]
struct ImageStreamer {
    v: Arc<RwLock<Cursor<Vec<u8>>>>,
    last: Arc<RwLock<u64>>,
    last_attempt: u64,
}

impl ImageStreamer {
    fn init() -> ImageStreamer {
        let v = Vec::new();
        let hasher = DefaultHasher::default();
        return ImageStreamer {
            v: Arc::new(RwLock::new(Cursor::new(v))),
            last: Arc::new(RwLock::new(hasher.finish())),
            last_attempt: hasher.finish(),
        };
    }

    async fn start(
        &self,
        img_grids: &[FlutGrid<u32>; GRID_LENGTH],
        grid_id: usize,
    ) -> io::Result<()> {
        println!("start called");
        let mut interval = tokio::time::interval(Duration::from_millis(25));
        loop {
            interval.tick().await;
            let mut hasher = DefaultHasher::default();
            img_grids[grid_id].cells.hash(&mut hasher);
            if hasher.finish() == *self.last.read().unwrap() {
                continue;
            } else {
                *self.last.write().unwrap() = hasher.finish();
            }
            println!("ticking...");
            let img = img_grids[grid_id]
                .view(
                    0,
                    0,
                    img_grids[grid_id].width(),
                    img_grids[grid_id].height(),
                )
                .to_image();
            let mut new = self.v.write().unwrap();
            let _ = img.write_to(&mut *new, image::ImageFormat::Png);
        }
    }
}

impl ImageStreamer {
    fn has_next(&self) -> Poll<u64> {
        let now = *self.last.read().unwrap();
        if now == self.last_attempt {
            return Poll::Pending;
        }
        return Poll::Ready(now);
    }
}

impl Stream for ImageStreamer {
    type Item = io::Result<hyper::body::Frame<bytes::Bytes>>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            let new_hash = match Pin::new(&mut self).has_next() {
                Poll::Ready(i) => Some(i),
                Poll::Pending => None,
            };
            if new_hash.is_none() {
                return Poll::Pending;
            }
            println!("got one");
            self.last_attempt = new_hash.unwrap();
            let r = self.v.read().unwrap();
            let v: Vec<u8> = (*r.clone().into_inner()).to_vec();

            return Poll::Ready(Some(Ok(Frame::data(v.into()))));
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
const GRID_LENGTH: usize = 1;

static COUNTER: AtomicU64 = AtomicU64::new(0);

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

fn increment_counter() {
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

async fn process_lock<
    R: AsyncReadExt + std::marker::Unpin,
    W: AsyncWriteExt + std::marker::Unpin,
>(
    reader: &mut R,
    writer: &mut W,
    grids: &mut [FlutGrid<u32>; GRID_LENGTH],
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
                match grids.get_mut(aa[0] as usize) {
                    Some(grid) => {
                        grid.set(
                            u16::from_le_bytes([aa[1], aa[2]]),
                            u16::from_le_bytes([aa[3], aa[4]]),
                            u32::from_be_bytes([aa[5], aa[6], aa[7], 0]),
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
    grids: &mut [FlutGrid<u32>; GRID_LENGTH],
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
                            .chain(color.to_be_bytes().into_iter().skip(1))
                            .collect::<Vec<_>>();
                        writer.write_all(towrite).await?;
                    }
                }
                return Ok(());
            }
            SET_PX_RGB_BIN => {
                let canvas = reader.read_u8().await?;
                let x = reader.read_u16_le().await?;
                let y = reader.read_u16_le().await?;
                let r = reader.read_u8().await?;
                let g = reader.read_u8().await?;
                let b = reader.read_u8().await?;
                let rgb = u32::from_be_bytes([r, g, b, 0xff]);
                set_pixel_rgba(grids, canvas, x, y, rgb);
                increment_counter();
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

async fn process_socket<W, R>(
    reader: R,
    writer: W,
    grids: &mut [FlutGrid<u32>; GRID_LENGTH],
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

async fn web(listener: TcpListener, image_streamer: ImageStreamer) -> io::Result<()> {
    loop {
        todo!("idk yet");
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    println!("Start initialisation");
    let grids = [FlutGrid::init(800, 600, 0xff00ff)];
    assert_eq!(grids.len(), GRID_LENGTH);
    let asuc = Arc::new(SyncUnsafeCell::new(grids));
    println!("created grids");

    let flut_listener = TcpListener::bind("0.0.0.0:7791").await?;
    println!("bound flut listener");

    let img_asuc = asuc.clone();
    let img_grids = unsafe { img_asuc.get().as_ref().unwrap() };
    let streamer = ImageStreamer::init();
    let cstrm = streamer.clone();
    let _ = tokio::spawn(async move {
        println!("staring streming");
        let _ = cstrm.start(&img_grids, 0).await;
    });
    println!("streamer started");

    let web_listener = TcpListener::bind("0.0.0.0:7792").await?;
    println!("bound web listener");

    let _ = tokio::spawn(async move { web(web_listener, streamer) });
    println!("web server started");

    loop {
        let (mut socket, _) = flut_listener.accept().await?;
        let asuc = asuc.clone();
        let _ = tokio::spawn(async move {
            let grids = unsafe { asuc.get().as_mut().unwrap() };
            let (reader, writer) = socket.split();
            match process_socket(reader, writer, grids).await {
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
    use tokio_test::assert_ok;

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
        let reader = tokio_test::io::Builder::new()
            .read(&[SET_PX_RGB_BIN, 0, 16, 0, 32, 0, 0, 0, 0])
            .read(&[SET_PX_RGB_BIN, 0, 16, 0, 33, 0, 2, 3, 5])
            .build();
        let writer = tokio_test::io::Builder::new().build();
        assert_ok!(process_socket(reader, writer, &mut grids).await);
        assert_eq!(grids[0].get(16, 32), Some(&0x00000000));
        assert_eq!(grids[0].get(16, 33), Some(&0x02030500));
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

        assert_eq!(grids[0].get(100, 0), Some(&0x02030400));
        assert_eq!(grids[0].get(101, 0), Some(&0x02030500));
        assert_eq!(grids[0].get(102, 0), Some(&0x02030600));
    }

    #[tokio::test]
    async fn test_get_rgb_bin() {
        let mut grids = [FlutGrid::init(800, 600, 0xFF00F0)];
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
        assert_eq!(grids[0].get(15, 21), Some(&0xff00f0));
    }
}
