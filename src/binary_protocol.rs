use std::io::{self, Error, ErrorKind};

use atoi_radix10::parse_from_str;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use crate::{
    increment_counter, Canvas, Color, Command, Coordinate, Parser, Protocol, Responder, Response,
    MEEHHEH,
};

const SIZE_BIN: u8 = 115;
const HELP_BIN: u8 = 104;
const LOCK: u8 = 0;
const GET_PX_BIN: u8 = 32;
const SET_PX_RGB_BIN: u8 = 128;
const SET_PX_RGBA_BIN: u8 = 129;
const SET_PX_W_BIN: u8 = 130;

const SET_PX_RGB_BIN_LENGTH: usize = 8;
pub struct BinaryParser {}

impl BinaryParser {
    pub fn new() -> BinaryParser {
        return BinaryParser {};
    }
}

impl<R: AsyncBufRead + AsyncBufReadExt + std::marker::Unpin> Parser<R> for BinaryParser {
    async fn parse(&self, reader: &mut R) -> io::Result<Command> {
        let fst = reader.read_u8().await;
        match fst {
            Ok(i) => match i {
                HELP_BIN => return Ok(Command::Help),
                SIZE_BIN => {
                    let canvas = reader.read_u8().await?;
                    return Ok(Command::Size(canvas));
                }
                GET_PX_BIN => {
                    let canvas = reader.read_u8().await?;
                    let x = reader.read_u16_le().await?;
                    let y = reader.read_u16_le().await?;
                    return Ok(Command::GetPixel(canvas, x, y));
                }
                SET_PX_W_BIN => {
                    let canvas = reader.read_u8().await?;
                    let x = reader.read_u16_le().await?;
                    let y = reader.read_u16_le().await?;
                    let w = reader.read_u8().await?;
                    return Ok(Command::SetPixel(canvas, x, y, Color::W8(w)));
                }
                SET_PX_RGB_BIN => {
                    let canvas = reader.read_u8().await?;
                    let x = reader.read_u16_le().await?;
                    let y = reader.read_u16_le().await?;
                    let r = reader.read_u8().await?;
                    let g = reader.read_u8().await?;
                    let b = reader.read_u8().await?;
                    return Ok(Command::SetPixel(canvas, x, y, Color::RGB24(r, g, b)));
                }
                SET_PX_RGBA_BIN => {
                    let canvas = reader.read_u8().await?;
                    let x = reader.read_u16_le().await?;
                    let y = reader.read_u16_le().await?;
                    let r = reader.read_u8().await?;
                    let g = reader.read_u8().await?;
                    let b = reader.read_u8().await?;
                    let a = reader.read_u8().await?;
                    return Ok(Command::SetPixel(canvas, x, y, Color::RGBA32(r, g, b, a)));
                }
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
}

impl MEEHHEH for BinaryParser {
    fn change_canvas(&mut self, _canvas: Canvas) -> io::Result<()> {
        return Err(Error::from(ErrorKind::Unsupported));
    }
}

impl<W: AsyncWriteExt + std::marker::Unpin> Responder<W> for BinaryParser {
    async fn unparse(&self, response: Response, writer: &mut W) -> io::Result<()> {
        let help_text = format!(
            "
You found the binary protocol help text
you can get this by sending ({:02X}) to the server
To get the size of a canvas, send ({:02X}) (u8 canvas) to the server
To set a pixel using RGB, use ({:02X}) (u8 canvas) (x as u16_le) (y as u16_le) (u8 r) (u8 g) (u8 b)
",
            HELP_BIN, SIZE_BIN, SET_PX_RGB_BIN
        );
        match response {
            Response::Help => writer.write_all(help_text.as_bytes()).await,
            Response::Size(x, y) => {
                writer.write_u16_le(x).await?;
                writer.write_u16_le(y).await
            }
            Response::GetPixel(_, _, c) => writer.write_all(&c).await,
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
    async fn test_bin_help_parse() {
        let parser = BinaryParser::new();
        let reader = tokio_test::io::Builder::new().read(&[HELP_BIN]).build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::Help)
    }

    #[tokio::test]
    async fn test_bin_size_parse() {
        let parser = BinaryParser::new();
        let reader = tokio_test::io::Builder::new().read(&[SIZE_BIN, 3]).build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::Size(3))
    }

    #[tokio::test]
    async fn test_bin_px_set_w_parse() {
        let parser = BinaryParser::new();
        let reader = tokio_test::io::Builder::new()
            .read(&[SET_PX_W_BIN, 0x01, 0x69, 0x42, 0x42, 0x69, 0x82])
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(1, 0x4269, 0x6942, Color::W8(0x82))
        )
    }

    #[tokio::test]
    async fn test_bin_px_set_rgb_parse() {
        let parser = BinaryParser::new();
        let reader = tokio_test::io::Builder::new()
            .read(&[
                SET_PX_RGB_BIN,
                0x01,
                0x69,
                0x42,
                0x42,
                0x69,
                0x82,
                0x00,
                0xff,
            ])
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(1, 0x4269, 0x6942, Color::RGB24(0x82, 0x00, 0xff))
        )
    }

    #[tokio::test]
    async fn test_bin_px_set_rgba_parse() {
        let parser = BinaryParser::new();
        let reader = tokio_test::io::Builder::new()
            .read(&[
                SET_PX_RGBA_BIN,
                0x01,
                0x69,
                0x42,
                0x42,
                0x69,
                0x82,
                0x00,
                0xff,
                0xa0,
            ])
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(1, 0x4269, 0x6942, Color::RGBA32(0x82, 0x00, 0xff, 0xa0))
        )
    }

    #[tokio::test]
    async fn test_bin_px_get_parse() {
        let parser = BinaryParser::new();
        let reader = tokio_test::io::Builder::new()
            .read(&[GET_PX_BIN, 0x03, 0x69, 0x42, 0x42, 0x69])
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::GetPixel(3, 0x4269, 0x6942))
    }

    #[tokio::test]
    async fn test_bin_parse_multiple() {
        let parser = BinaryParser::new();
        let reader = tokio_test::io::Builder::new()
            .read(&[
                SET_PX_RGB_BIN,
                0x01,
                0x69,
                0x42,
                0x42,
                0x69,
                0x82,
                0x00,
                0xff,
            ])
            .read(&[
                SET_PX_RGBA_BIN,
                0x01,
                0x69,
                0x42,
                0x42,
                0x69,
                0x82,
                0x00,
                0xff,
                0xa0,
            ])
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        let thingy2 = parser.parse(&mut bufreader).await;
        assert_eq!(
            thingy.unwrap(),
            Command::SetPixel(1, 0x4269, 0x6942, Color::RGB24(0x82, 0x00, 0xff))
        );
        assert_eq!(
            thingy2.unwrap(),
            Command::SetPixel(1, 0x4269, 0x6942, Color::RGBA32(0x82, 0x00, 0xff, 0xa0))
        );
    }
}
