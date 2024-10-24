use std::{
    io::{self, Error, ErrorKind},
    sync::Arc,
};

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};

use crate::{
    get_pixel,
    grid::{self, Flut},
    increment_counter,
    protocols::{BinaryParser, IOProtocol, Parser, Responder, TextParser},
    set_pixel_rgba, Canvas, Color, Command, Coordinate, Protocol, Response,
};

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

pub struct FlutClient<R, W>
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
            Protocol::Text => self.parser = ParserTypes::TextParser(TextParser::default()),
            Protocol::Binary => self.parser = ParserTypes::BinaryParser(BinaryParser::default()),
        }
    }

    pub fn new(reader: R, writer: W, grids: Arc<[grid::Flut<u32>]>) -> Self {
        FlutClient {
            reader: BufReader::new(reader),
            writer: BufWriter::new(writer),
            grids,
            parser: ParserTypes::TextParser(TextParser::default()),
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
                        Err(err) if err.kind() == ErrorKind::UnexpectedEof => {
                increment_counter(self.counter);
                            return Ok(())},
                        Err(e) => return Err(e),
                    }
                }
                increment_counter(self.counter);
                self.counter = 0;
            });
        }
    }
}
