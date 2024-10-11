#![feature(test)]
#![feature(sync_unsafe_cell)]
#![feature(if_let_guard)]

mod binary_protocol;
mod grid;
mod text_protocol;

use std::{
    alloc::System,
    fmt::Debug,
    fs::{create_dir_all, File},
    io::{self, Error, ErrorKind},
    path::Path,
    sync::{atomic::AtomicU64, Arc},
    time::{Duration, SystemTime},
};

use binary_protocol::BinaryParser;
use chrono::Local;
use grid::{Flut, Grid};
use image::{codecs::jpeg::JpegEncoder, save_buffer, DynamicImage, GenericImageView, SubImage};
use text_protocol::TextParser;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
    time::{interval, Instant},
};

extern crate test;
const GRID_LENGTH: usize = 1;
const HOST: &str = "0.0.0.0:7791";

const HELP_TEXT: &[u8] = b"Flurry is a pixelflut implementation, this means you can use commands to get and set pixels in the canvas
SIZE returns the size of the canvas
PX {x} {y} returns the color of the pixel at {x}, {y}
If you include a color in hex format you set a pixel instead
PX {x} {y} {RGB} sets the color of the pixel at {x}, {y} to the rgb value
PX {x} {y} {RGBA} blends the pixel at {x}, {y} with the rgb value weighted by the a
PX {x} {y} {W} sets the color of the pixel at {x}, {y} to the grayscale value
";

static COUNTER: AtomicU64 = AtomicU64::new(0);

type Canvas = u8;
type Coordinate = u16;

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
enum Color {
    RGB24(u8, u8, u8),
    RGBA32(u8, u8, u8, u8),
    W8(u8),
}

#[derive(Debug, PartialEq)]
enum Protocol {
    Text,
    Binary,
}

#[derive(Debug, PartialEq)]
enum Command {
    Help,
    Size(Canvas),
    GetPixel(Canvas, Coordinate, Coordinate),
    SetPixel(Canvas, Coordinate, Coordinate, Color),
    ChangeCanvas(Canvas),
    ChangeProtocol(Protocol),
}

#[derive(Debug, PartialEq)]
enum Response {
    Help,
    Size(Coordinate, Coordinate),
    GetPixel(Coordinate, Coordinate, [u8; 3]),
}

trait Parser<R>
where
    R: std::marker::Unpin + tokio::io::AsyncBufRead,
{
    async fn parse(&self, reader: &mut R) -> io::Result<Command>;
}

trait IOProtocol {
    fn change_canvas(&mut self, canvas: Canvas) -> io::Result<()>;
}

trait Responder<W>
where
    W: AsyncWriteExt + std::marker::Unpin,
{
    async fn unparse(&self, response: Response, writer: &mut W) -> io::Result<()>;
}

async fn listen_handle() -> io::Result<()> {
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
        interval.tick().await;
        let cnt = COUNTER.load(std::sync::atomic::Ordering::Relaxed);
        println!("{cnt} pixels were changed");
    }
}

macro_rules! build_parser_type_enum {
    ($($name:ident: $t:ty,)*) => {

        #[derive(Clone)]
        enum ParserTypes {
            $($name($t),)*
        }

        macro_rules! match_parser {
            ($pident:ident: $parser:expr => $f:expr) => (
                match &mut $parser {
                    $(
                        ParserTypes::$name($pident) => $f,
                    )*
                }
            )
        }
    };
}

build_parser_type_enum! {
    TextParser: TextParser,
    BinaryParser: BinaryParser,
}

struct FlutClient<R, W>
where
    R: AsyncReadExt + std::marker::Unpin,
    W: AsyncWriteExt + std::marker::Unpin,
{
    reader: BufReader<R>,
    writer: BufWriter<W>,
    grids: Arc<[Flut<u32>]>,
    parser: ParserTypes,
    counter: u64,
}

impl<R, W> FlutClient<R, W>
where
    R: AsyncReadExt + std::marker::Unpin,
    W: AsyncWriteExt + std::marker::Unpin,
{
    async fn help_command(&mut self) -> io::Result<()> {
        println!("HELP wanted");
        match_parser!(parser: self.parser => parser.unparse(Response::Help, &mut self.writer).await?);

        self.writer.flush().await?;
        Ok(())
    }

    async fn size_command(&mut self, canvas: Canvas) -> io::Result<()> {
        let (x, y) = self.grids[canvas as usize].get_size();
        match_parser!(parser: self.parser => parser.unparse(
            Response::Size(Coordinate::try_from(x).unwrap(), Coordinate::try_from(y).unwrap()), &mut self.writer).await?);

        self.writer.flush().await?;
        Ok(())
    }

    async fn get_pixel_command(
        &mut self,
        canvas: Canvas,
        x: Coordinate,
        y: Coordinate,
    ) -> io::Result<()> {
        let color = match get_pixel(&self.grids, canvas, x, y) {
            None => return Err(Error::from(ErrorKind::InvalidInput)),
            Some(color) => color.to_be_bytes(),
        };
        match_parser!(parser: self.parser => parser.unparse(
            Response::GetPixel(x,y,[color[0], color[1], color[2]]), &mut self.writer).await?
        );
        Ok(())
    }

    fn set_pixel_command(&mut self, canvas: Canvas, x: Coordinate, y: Coordinate, color: &Color) {
        let c: u32 = match color {
            Color::RGB24(red, green, blue) => u32::from_be_bytes([*red, *green, *blue, 0xff]),
            Color::RGBA32(red, green, blue, alpha) => {
                u32::from_be_bytes([*red, *green, *blue, *alpha])
            }
            Color::W8(white) => u32::from_be_bytes([*white, *white, *white, 0xff]),
        };
        set_pixel_rgba(self.grids.as_ref(), canvas, x, y, c);
        self.counter += 1;
    }

    fn change_canvas_command(&mut self, canvas: Canvas) -> io::Result<()> {
        match_parser!(parser: self.parser => parser.change_canvas(canvas))
    }

    fn change_protocol(&mut self, protocol: &Protocol) {
        match protocol {
            Protocol::Text => self.parser = ParserTypes::TextParser(TextParser::new(0)),
            Protocol::Binary => self.parser = ParserTypes::BinaryParser(BinaryParser::new()),
        }
    }

    pub fn new(reader: R, writer: W, grids: Arc<[grid::Flut<u32>]>) -> FlutClient<R, W> {
        FlutClient {
            reader: BufReader::new(reader),
            writer: BufWriter::new(writer),
            grids,
            parser: ParserTypes::TextParser(TextParser::new(0)),
            counter: 0,
        }
    }

    pub async fn process_socket(&mut self) -> io::Result<()> {
        loop {
            match_parser!(parser: &self.parser.clone() => 'outer: loop {
                for _ in 0..1000 {
                    let parsed = parser.parse(&mut self.reader).await;
                    match parsed {
                        Ok(Command::Help) => self.help_command().await?,
                        Ok(Command::Size(canvas)) => self.size_command(canvas).await?,
                        Ok(Command::GetPixel(canvas, x, y)) => self.get_pixel_command(canvas, x, y).await?,
                        Ok(Command::SetPixel(canvas, x, y, color)) => self.set_pixel_command(canvas, x, y, &color),
                        Ok(Command::ChangeCanvas(canvas)) => {
                            self.change_canvas_command(canvas)?;
                            break 'outer;
                        }
                        Ok(Command::ChangeProtocol(protocol)) => {
                            self.change_protocol(&protocol);
                            break 'outer;
                        }
                        Err(err) if err.kind() == ErrorKind::UnexpectedEof => return Ok(()),
                        Err(e) => return Err(e),
                    }
                }
                increment_counter(self.counter);
                self.counter = 0;
            });
        }
    }
}

async fn save_image_frames(grids: Arc<[grid::Flut<u32>]>) -> io::Result<()> {
    let base_dir = Path::new("./recordings");
    let mut timer = interval(Duration::from_secs(5));
    create_dir_all(base_dir)?;
    loop {
        timer.tick().await;
        for grid in grids.as_ref() {
            let p = base_dir.join(format!("{}", Local::now().format("%Y-%m-%d %H:%M:%S")));
            println!("timer ticked, grid writing to {:?}", p);
            let mut file_writer = File::create(p)?;

            let encoder = JpegEncoder::new_with_quality(&mut file_writer, 50);
            grid.view(0, 0, grid.width(), grid.height()).to_image();

            let sub_image = SubImage::new(grid, 0, 0, grid.width(), grid.height());
            let image = sub_image.to_image();
            match image.write_with_encoder(encoder) {
                Ok(_) => {}
                Err(err) => eprintln!("{}", err),
            }
        }
    }
}

async fn handle_flut(flut_listener: TcpListener, grids: Arc<[grid::Flut<u32>]>) -> io::Result<()> {
    let mut handles = Vec::new();
    loop {
        let (mut socket, _) = flut_listener.accept().await?;
        let grids = grids.clone();
        handles.push(tokio::spawn(async move {
            let (reader, writer) = socket.split();
            let mut connection = FlutClient::new(reader, writer, grids);
            let resp = connection.process_socket().await;
            match resp {
                Ok(()) => Ok(()),
                Err(err) => Err(err),
            }
        }));
    }
}

#[tokio::main]
#[allow(clippy::needless_return)]
async fn main() {
    println!("created grids");
    let grids: Arc<[Flut<u32>; GRID_LENGTH]> = [grid::Flut::init(800, 600, 0xff_00_ff_ff)].into();

    let Ok(flut_listener) = TcpListener::bind(HOST).await else {
        eprintln!("Was unable to bind to {HOST}, please check if a different process is bound");
        return;
    };
    println!("bound flut listener");

    let handles = vec![
        // log the amount of changed pixels each second
        (tokio::spawn(listen_handle())),
        // save frames every 5 seconds
        (tokio::spawn(save_image_frames(grids.clone()))),
        // accept and handle flut connections
        (tokio::spawn(handle_flut(flut_listener, grids.clone()))),
    ];

    for handle in handles {
        println!("joined handle had result {:?}", handle.await);
    }
}

#[cfg(test)]
mod tests {}
