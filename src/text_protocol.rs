use std::io::{self, Error, ErrorKind};

use atoi_radix10::parse_from_str;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt};

use crate::{
    Canvas, Color, Command, Coordinate, Parser, Protocol, Responder, Response, GRID_LENGTH,
    HELP_TEXT, MEEHHEH,
};

pub struct TextParser {
    canvas: Canvas,
}

fn parse_coordinate(string: &str) -> io::Result<Coordinate> {
    match parse_from_str(string) {
        Ok(coord) => return Ok(coord),
        Err(_) => return Err(Error::from(ErrorKind::InvalidInput)),
    }
}

fn parse_color(color: &str) -> io::Result<Color> {
    if let Ok(bytes) = hex::decode(color) {
        match bytes.len() {
            1 => return Ok(Color::W8(bytes[0])),
            3 => return Ok(Color::RGB24(bytes[0], bytes[1], bytes[2])),
            4 => return Ok(Color::RGBA32(bytes[0], bytes[1], bytes[2], bytes[3])),
            _ => return Err(Error::from(ErrorKind::InvalidInput)),
        }
    }
    return Err(Error::from(ErrorKind::InvalidInput));
}

impl TextParser {
    pub fn new(canvas: Canvas) -> TextParser {
        return TextParser { canvas };
    }

    fn parse_pixel(&self, line: &str) -> io::Result<Command> {
        let mut split = line.trim().split(' ');

        let _command = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        let x_coordinate = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        let y_coordinate = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        if let (Ok(horizontal), Ok(vertical)) = (x_coordinate.parse(), y_coordinate.parse()) {
            match split.next() {
                None => return Ok(Command::GetPixel(self.canvas, horizontal, vertical)),
                Some(color) => match parse_color(color) {
                    Ok(color) => {
                        return Ok(Command::SetPixel(self.canvas, horizontal, vertical, color))
                    }
                    Err(err) => return Err(err),
                },
            }
        } else {
            return Err(Error::from(ErrorKind::InvalidInput));
        }
    }
    fn parse_canvas(&self, line: &str) -> io::Result<Command> {
        let mut split = line.trim().split(' ');

        let _command = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        let canvas = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        println!("{:?}", canvas);
        if let Ok(canvas) = canvas.parse() {
            return Ok(Command::ChangeCanvas(canvas));
        } else {
            return Err(Error::from(ErrorKind::InvalidInput));
        }
    }
    fn parse_protocol(&self, line: &str) -> io::Result<Command> {
        let mut split = line.trim().split(' ');

        let _command = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        let protocol = split.next().ok_or(Error::from(ErrorKind::InvalidInput))?;
        match protocol {
            "binary" => return Ok(Command::ChangeProtocol(Protocol::Binary)),
            "text" => return Ok(Command::ChangeProtocol(Protocol::Text)),
            _ => return Err(Error::from(ErrorKind::InvalidInput)),
        }
    }
}

impl<R: AsyncBufRead + AsyncBufReadExt + std::marker::Unpin> Parser<R> for TextParser {
    async fn parse(&self, reader: &mut R) -> io::Result<Command> {
        let mut line = "".to_string();
        if let Ok(_) = reader.read_line(&mut line).await {
            println!("{:?}", line);
            if line.starts_with("HELP") {
                return Ok(Command::Help);
            } else if line.starts_with("SIZE") {
                return Ok(Command::Size(self.canvas));
            } else if line.starts_with("PX ") {
                return self.parse_pixel(&line);
            } else if line.starts_with("CANVAS ") {
                return self.parse_canvas(&line);
            } else if line.starts_with("PROTOCOL ") {
                return self.parse_protocol(&line);
            }
        }
        return Err(Error::from(ErrorKind::InvalidInput));
    }
}

impl MEEHHEH for TextParser {
    fn change_canvas(&mut self, canvas: Canvas) -> io::Result<()> {
        if (canvas as usize) < GRID_LENGTH {
            self.canvas = canvas;
            return Ok(());
        } else {
            return Err(Error::from(ErrorKind::InvalidInput));
        }
    }
}

impl<W: AsyncWriteExt + std::marker::Unpin> Responder<W> for TextParser {
    async fn unparse(&self, response: Response, writer: &mut W) -> io::Result<()> {
        match response {
            Response::Help => writer.write_all(HELP_TEXT).await,
            Response::Size(x, y) => {
                writer
                    .write_all(format!("SIZE {} {}\n", x, y).as_bytes())
                    .await
            }
            Response::GetPixel(x, y, color) => {
                writer
                    .write_all(format!("PX {} {} {}\n", x, y, hex::encode_upper(color)).as_bytes())
                    .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::FlutGrid;
    use tokio::io::BufReader;
    use tokio_test::assert_ok;

    #[tokio::test]
    async fn test_help_parse() {
        let parser = TextParser::new(0);
        let reader = tokio_test::io::Builder::new().read(b"HELP\n").build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::Help)
    }

    #[tokio::test]
    async fn test_size_parse() {
        let parser = TextParser::new(0);
        let reader = tokio_test::io::Builder::new().read(b"SIZE\n").build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::Size(0))
    }

    #[tokio::test]
    async fn test_canvas_parse() {
        let parser = TextParser::new(0);
        let reader = tokio_test::io::Builder::new().read(b"CANVAS 12\n").build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::ChangeCanvas(12))
    }

    #[tokio::test]
    async fn test_px_set_w_parse() {
        let parser = TextParser::new(0);
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991 81\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(0, 28283, 29991, Color::W8(0x81))
        )
    }

    #[tokio::test]
    async fn test_px_set_rgb_parse() {
        let parser = TextParser::new(0);
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991 8800ff\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(0, 28283, 29991, Color::RGB24(0x88, 0x00, 0xff))
        )
    }

    #[tokio::test]
    async fn test_px_set_rgba_parse() {
        let parser = TextParser::new(0);
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991 8800ff28\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(0, 28283, 29991, Color::RGBA32(0x88, 0x00, 0xff, 0x28))
        )
    }

    #[tokio::test]
    async fn test_px_get_parse() {
        let parser = TextParser::new(0);
        let reader = tokio_test::io::Builder::new()
            .read(b"PX 28283 29991\n")
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::GetPixel(0, 28283, 29991))
    }

    #[tokio::test]
    async fn parse_multiple() {
        let parser = TextParser::new(0);
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
