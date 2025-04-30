use atoi_radix10::parse_from_str;
use std::io::{self, Error, ErrorKind};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt};

use crate::{
    config::{GRID_LENGTH, HELP_TEXT},
    Canvas, Color, Command, Coordinate, Protocol, Response,
};

use super::{IOProtocol, Parser, Responder};

#[derive(Clone, Default)]
pub struct TextParser {
    canvas: Canvas,
}

#[allow(dead_code)]
fn parse_coordinate(string: &str) -> io::Result<Coordinate> {
    match parse_from_str(string) {
        Ok(coord) => Ok(coord),
        Err(_) => Err(Error::from(ErrorKind::InvalidInput)),
    }
}

type HexChar = u8;

fn val(c1: u8, c2: u8) -> io::Result<HexChar> {
    Ok(((match c1 {
        b'A'..=b'F' => c1 - b'A' + 10,
        b'a'..=b'f' => c1 - b'a' + 10,
        b'0'..=b'9' => c1 - b'0',
        _ => return Err(Error::from(ErrorKind::InvalidInput)),
    }) << 4)
        | (match c2 {
            b'A'..=b'F' => c2 - b'A' + 10,
            b'a'..=b'f' => c2 - b'a' + 10,
            b'0'..=b'9' => c2 - b'0',
            _ => return Err(Error::from(ErrorKind::InvalidInput)),
        }))
}

fn parse_color(color: &str) -> io::Result<Color> {
    let color = color.as_bytes();
    match color.len() {
        2 if let Ok(w) = val(color[0], color[1]) => Ok(Color::W8(w)),
        6 if let (Ok(r), Ok(g), Ok(b)) = (
            val(color[0], color[1]),
            val(color[2], color[3]),
            val(color[4], color[5]),
        ) =>
        {
            Ok(Color::RGB24(r, g, b))
        }
        8 if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
            val(color[0], color[1]),
            val(color[2], color[3]),
            val(color[4], color[5]),
            val(color[6], color[7]),
        ) =>
        {
            Ok(Color::RGBA32(r, g, b, a))
        }
        _ => Err(Error::from(ErrorKind::InvalidInput)),
    }
}

impl TextParser {
    pub fn new(canvas: Canvas) -> TextParser {
        TextParser { canvas }
    }

    fn parse_pixel(&self, line: &str) -> io::Result<Command> {
        let mut split = line.trim().split(' ');

        let _command = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        let x_coordinate = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        let y_coordinate = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        if let (Ok(horizontal), Ok(vertical)) = (x_coordinate.parse(), y_coordinate.parse()) {
            match split.next() {
                None => Ok(Command::GetPixel(self.canvas, horizontal, vertical)),
                Some(color) => match parse_color(color) {
                    Ok(color) => Ok(Command::SetPixel(self.canvas, horizontal, vertical, color)),
                    Err(err) => Err(err),
                },
            }
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }
    fn parse_canvas(line: &str) -> io::Result<Command> {
        let mut split = line.trim().split(' ');

        let _command = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        let canvas = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        if let Ok(canvas) = canvas.parse() {
            Ok(Command::ChangeCanvas(canvas))
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }
    fn parse_protocol(line: &str) -> io::Result<Command> {
        let mut split = line.trim().split(' ');

        let _command = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        let protocol = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        match protocol {
            "binary" => Ok(Command::ChangeProtocol(Protocol::Binary)),
            "text" => Ok(Command::ChangeProtocol(Protocol::Text)),
            _ => Err(Error::from(ErrorKind::InvalidInput)),
        }
    }
}

impl<R: AsyncBufRead + AsyncBufReadExt + std::marker::Unpin> Parser<R> for TextParser {
    async fn parse(&self, reader: &mut R) -> io::Result<Command> {
        let mut line = String::new();
        if reader.read_line(&mut line).await.is_ok() {
            if line.starts_with("HELP") {
                return Ok(Command::Help);
            } else if line.starts_with("PROTOCOLS") {
                return Ok(Command::Protocols);
            } else if line.starts_with("SIZE") {
                return Ok(Command::Size(self.canvas));
            } else if line.starts_with("PX ") {
                return self.parse_pixel(&line);
            } else if line.starts_with("CANVAS ") {
                return TextParser::parse_canvas(&line);
            } else if line.starts_with("PROTOCOL ") {
                return TextParser::parse_protocol(&line);
            }
        }
        Err(Error::from(ErrorKind::InvalidInput))
    }
}

impl IOProtocol for TextParser {
    fn change_canvas(&mut self, canvas: Canvas) -> io::Result<()> {
        if (canvas as usize) < GRID_LENGTH {
            self.canvas = canvas;
            Ok(())
        } else {
            Err(Error::from(ErrorKind::InvalidInput))
        }
    }
}

impl<W: AsyncWriteExt + std::marker::Unpin> Responder<W> for TextParser {
    async fn unparse(&self, response: Response, writer: &mut W) -> io::Result<()> {
        match response {
            Response::Help => writer.write_all(HELP_TEXT).await,
            Response::Protocols(protos) => {
                for protocol in protos {
                    match protocol {
                        crate::ProtocolStatus::Enabled(proto) => {
                            writer
                                .write_all(format!("Enabled: {proto}\n").as_bytes())
                                .await?;
                        }
                        crate::ProtocolStatus::Disabled(proto) => {
                            writer
                                .write_all(format!("Disabled: {proto}\n").as_bytes())
                                .await?;
                        }
                    }
                }
                Ok(())
            }
            Response::Size(x, y) => writer.write_all(format!("SIZE {x} {y}\n").as_bytes()).await,
            Response::GetPixel(x, y, color) => {
                writer
                    .write_all(
                        format!(
                            "PX {x} {y} {:02X}{:02X}{:02X}\n",
                            color[0], color[1], color[2]
                        )
                        .as_bytes(),
                    )
                    .await
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::needless_return)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn test_help_parse() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new().read(b"HELP\n").build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::Help);
    }

    #[tokio::test]
    async fn test_size_parse() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new().read(b"SIZE\n").build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::Size(0));
    }

    #[tokio::test]
    async fn test_canvas_parse() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new().read(b"CANVAS 12\n").build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::ChangeCanvas(12));
    }

    #[tokio::test]
    async fn test_px_set_w_parse() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991 81\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(0, 28283, 29991, Color::W8(0x81))
        );
    }

    #[tokio::test]
    async fn test_px_set_w_parse_caps() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991 AB\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(0, 28283, 29991, Color::W8(0xAB))
        );
    }

    #[tokio::test]
    async fn test_px_set_rgb_parse() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991 8800ff\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(0, 28283, 29991, Color::RGB24(0x88, 0x00, 0xff))
        );
    }

    #[tokio::test]
    async fn test_px_set_rgb_parse_caps() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991 8800FA\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(0, 28283, 29991, Color::RGB24(0x88, 0x00, 0xfa))
        );
    }

    #[tokio::test]
    async fn test_px_set_rgba_parse() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991 8800ff28\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(0, 28283, 29991, Color::RGBA32(0x88, 0x00, 0xff, 0x28))
        );
    }

    #[tokio::test]
    async fn test_px_set_rgba_parse_caps() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991 AB0c3F88\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(0, 28283, 29991, Color::RGBA32(0xab, 0x0c, 0x3f, 0x88))
        );
    }

    #[tokio::test]
    async fn test_px_get_parse() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::GetPixel(0, 28283, 29991));
    }

    #[tokio::test]
    async fn parse_multiple() {
        let parser = TextParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(b"CANVAS 12\n")
            .read(b"SIZE\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        let thingy2 = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::ChangeCanvas(12));
        assert_eq!(thingy2.unwrap(), Command::Size(0));
    }
}
