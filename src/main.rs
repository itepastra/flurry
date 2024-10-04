#![feature(test)]
#![feature(sync_unsafe_cell)]

mod binary_protocol;
mod grid;
mod text_protocol;

use std::{
    cell::SyncUnsafeCell,
    io::{self, Error, ErrorKind},
    iter::once,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use binary_protocol::BinaryParser;
use grid::{FlutGrid, Grid};
use text_protocol::TextParser;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
};

extern crate test;

const HELP_TEXT: &[u8] = b"Flurry is a pixelflut implementation, this means you can use commands to get and set pixels in the canvas
SIZE returns the size of the canvas
PX {x} {y} returns the color of the pixel at {x}, {y}
If you include a color in hex format you set a pixel instead
PX {x} {y} {RGB} sets the color of the pixel at {x}, {y} to the rgb value
PX {x} {y} {RGBA} blends the pixel at {x}, {y} with the rgb value weighted by the a
PX {x} {y} {W} sets the color of the pixel at {x}, {y} to the grayscale value
";
const GRID_LENGTH: usize = 1;

static COUNTER: AtomicU64 = AtomicU64::new(0);

type Canvas = u8;
type Coordinate = u16;

fn set_pixel_rgba(
    grids: &[grid::FlutGrid<u32>],
    canvas: Canvas,
    x: Coordinate,
    y: Coordinate,
    rgb: u32,
) {
    match grids.get(canvas as usize) {
        Some(grid) => grid.set(x, y, rgb),
        None => (),
    }
}

fn get_pixel(
    grids: &[grid::FlutGrid<u32>],
    canvas: Canvas,
    x: Coordinate,
    y: Coordinate,
) -> Option<&u32> {
    match grids.get(canvas as usize) {
        Some(grid) => return grid.get(x, y),
        None => return None,
    }
}

fn increment_counter() {
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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

trait MEEHHEH {
    fn change_canvas(&mut self, canvas: Canvas) -> io::Result<()>;
}

trait Responder<W>
where
    W: AsyncWriteExt + std::marker::Unpin,
{
    async fn unparse(&self, response: Response, writer: &mut W) -> io::Result<()>;
}

async fn listen_handle() {
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
        interval.tick().await;
        let cnt = COUNTER.load(std::sync::atomic::Ordering::Relaxed);
        println!("{} pixels were changed", cnt);
    }
}

macro_rules! build_parser_type_enum {
    ($($name:ident: $t:ty,)*) => {

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
    grids: Arc<[FlutGrid<u32>]>,
    parser: ParserTypes,
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
        return Ok(());
    }

    async fn size_command(&mut self, canvas: Canvas) -> io::Result<()> {
        let (x, y) = self.grids[canvas as usize].get_size();
        match_parser!(parser: self.parser => parser.unparse(Response::Size(x as Coordinate, y as Coordinate), &mut self.writer).await?);

        self.writer.flush().await?;
        return Ok(());
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
        match_parser!(parser: self.parser => parser.unparse(Response::GetPixel(x,y,[color[0], color[1], color[2]]), &mut self.writer).await);

        self.writer.flush().await?;
        return Ok(());
    }

    async fn set_pixel_command(
        &mut self,
        canvas: Canvas,
        x: Coordinate,
        y: Coordinate,
        color: Color,
    ) -> io::Result<()> {
        let c: u32 = match color {
            Color::RGB24(r, g, b) => u32::from_be_bytes([r, g, b, 0xff]),
            Color::RGBA32(r, g, b, a) => u32::from_be_bytes([r, g, b, a]),
            Color::W8(w) => u32::from_be_bytes([w, w, w, 0xff]),
        };
        println!("setting pixel {},{} to {}", x, y, c);
        set_pixel_rgba(self.grids.as_ref(), canvas, x, y, c);
        increment_counter();
        return Ok(());
    }

    async fn change_canvas_command(&mut self, canvas: Canvas) -> io::Result<()> {
        match_parser!(parser: self.parser => parser.change_canvas(canvas))
    }

    async fn change_protocol(&mut self, protocol: Protocol) -> io::Result<()> {
        match protocol {
            Protocol::Text => self.parser = ParserTypes::TextParser(TextParser::new(0)),
            Protocol::Binary => self.parser = ParserTypes::BinaryParser(BinaryParser::new()),
        }
        return Ok(());
    }

    pub fn new(reader: R, writer: W, grids: Arc<[grid::FlutGrid<u32>]>) -> FlutClient<R, W> {
        return FlutClient {
            reader: BufReader::new(reader),
            writer: BufWriter::new(writer),
            grids,
            parser: ParserTypes::TextParser(TextParser::new(0)),
        };
    }

    pub async fn process_socket(&mut self) -> io::Result<()> {
        loop {
            let parsed = match &self.parser {
                ParserTypes::TextParser(parser) => parser.parse(&mut self.reader).await,
                ParserTypes::BinaryParser(parser) => parser.parse(&mut self.reader).await,
            };

            match parsed {
                Ok(Command::Help) => self.help_command().await?,
                Ok(Command::Size(canvas)) => self.size_command(canvas).await?,
                Ok(Command::GetPixel(canvas, x, y)) => self.get_pixel_command(canvas, x, y).await?,
                Ok(Command::SetPixel(canvas, x, y, color)) => {
                    self.set_pixel_command(canvas, x, y, color).await?
                }
                Ok(Command::ChangeCanvas(canvas)) => self.change_canvas_command(canvas).await?,
                Ok(Command::ChangeProtocol(protocol)) => self.change_protocol(protocol).await?,

                Err(err) if err.kind() == ErrorKind::UnexpectedEof => {
                    return Ok(());
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    println!("created grids");
    let grids: Arc<[FlutGrid<u32>; GRID_LENGTH]> =
        [grid::FlutGrid::init(800, 600, 0xff00ffff)].into();

    let flut_listener = TcpListener::bind("0.0.0.0:7791").await?;
    println!("bound flut listener");

    let _ = tokio::spawn(listen_handle());

    loop {
        let (mut socket, _) = flut_listener.accept().await?;
        let grids = grids.clone();
        let _ = tokio::spawn(async move {
            let (reader, writer) = socket.split();
            let mut connection = FlutClient::new(reader, writer, grids);
            let resp = connection.process_socket().await;
            match resp {
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
}
