use std::{
    io::{self, Error, ErrorKind},
    sync::Arc,
};

#[cfg(feature = "auth")]
use crate::{blame::User, config::AUTH_SERVER_URL};
use crate::{
    get_pixel,
    grid::{self, Flut},
    increment_counter,
    protocols::{BinaryParser, IOProtocol, Parser, Responder, TextParser},
    set_pixel_rgba, Canvas, Color, Command, Coordinate, Protocol, Response,
};
#[cfg(feature = "auth")]
use bytes::Buf;
#[cfg(feature = "auth")]
use reqwest::{Client, ClientBuilder};
#[cfg(feature = "auth")]
use tokio::io::AsyncBufReadExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};

macro_rules! build_parser_type_enum {
    ($($name:ident: $t:ty: $feat:expr,)*) => {

        #[derive(Clone)]
        pub enum ParserTypes {
            $(
                #[cfg(feature = $feat)]
                $name($t),
            )*
        }

        impl std::default::Default for ParserTypes {
            // add code here
            fn default() -> Self {
                $(
                    #[cfg(feature = $feat)]
                    #[allow(unreachable_code)]
                    return ParserTypes::$name(<$t>::default());
                )*
            }
        }

        impl ParserTypes {
            pub fn announce() {
                $(
                    #[cfg(feature = $feat)]
                    tracing::info!("Enabled {}", $feat);
                    #[cfg(not(feature = $feat))]
                    tracing::info!("Disabled {}", $feat);
                )*
            }
        }

        macro_rules! match_parser {
            ($pident:ident: $parser:expr => $f:expr) => (
                match &mut $parser {
                    $(
                        #[cfg(feature = $feat)]
                        ParserTypes::$name($pident) => $f,
                    )*
                }
            )
        }
    };
}

build_parser_type_enum! {
    TextParser: TextParser: "text",
    BinaryParser: BinaryParser: "binary",
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
    #[cfg(feature = "auth")]
    auth_client: Client,
    #[cfg(feature = "auth")]
    user: User,
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
        set_pixel_rgba(
            self.grids.as_ref(),
            canvas,
            x,
            y,
            c,
            #[cfg(feature = "auth")]
            self.user,
        );
        self.counter += 1;
    }

    fn change_canvas_command(&mut self, canvas: Canvas) -> io::Result<()> {
        match_parser!(parser: self.parser => parser.change_canvas(canvas))
    }

    fn change_protocol(&mut self, protocol: &Protocol) {
        match protocol {
            #[cfg(feature = "text")]
            Protocol::Text => self.parser = ParserTypes::TextParser(TextParser::default()),
            #[cfg(not(feature = "text"))]
            Protocol::Text => {
                self.writer.write(b"feature \"text\" is not enabled.");
                self.writer.flush();
            }
            #[cfg(feature = "binary")]
            Protocol::Binary => self.parser = ParserTypes::BinaryParser(BinaryParser::default()),
            #[cfg(not(feature = "binary"))]
            Protocol::Binary => {
                self.writer.write(b"feature \"binary\" is not enabled.");
                self.writer.flush();
            }
        }
    }

    pub fn new(reader: R, writer: W, grids: Arc<[grid::Flut<u32>]>) -> Self {
        FlutClient {
            reader: BufReader::new(reader),
            writer: BufWriter::new(writer),
            grids,
            parser: ParserTypes::default(),
            counter: 0,
            #[cfg(feature = "auth")]
            auth_client: ClientBuilder::new().https_only(true).build().unwrap(),
            #[cfg(feature = "auth")]
            user: 0,
        }
    }

    pub async fn process_socket(&mut self) -> io::Result<()> {
        // Handle the auth flow
        #[cfg(feature = "auth")]
        {
            let mut buf = Vec::new();
            let chars = self.reader.read_until(b' ', &mut buf).await?;
            if chars != 5 {
                return Err(Error::from(ErrorKind::PermissionDenied));
            }
            if buf != b"AUTH " {
                return Err(Error::from(ErrorKind::PermissionDenied));
            }

            buf.clear();
            let token_length = self.reader.read_until(b'\n', &mut buf).await?;

            if token_length > 100 {
                return Err(Error::from(ErrorKind::PermissionDenied));
            }

            let request = self
                .auth_client
                .post(AUTH_SERVER_URL)
                .body(buf)
                .build()
                .unwrap();
            let response = self.auth_client.execute(request).await.unwrap();
            if response.status() != 200 {
                return Err(Error::from(ErrorKind::PermissionDenied));
            }

            let user = response.bytes().await.unwrap().get_u32();

            tracing::info!("User with id {user} authenticated");
            self.user = user;
        }
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
