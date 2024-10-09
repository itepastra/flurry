use std::io::{self, Error, ErrorKind};

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use crate::{Canvas, Color, Command, IOProtocol, Parser, Responder, Response};

const SIZE_BIN: u8 = 115;
const HELP_BIN: u8 = 104;
const GET_PX_BIN: u8 = 32;
const SET_PX_RGB_BIN: u8 = 128;
const SET_PX_RGBA_BIN: u8 = 129;
const SET_PX_W_BIN: u8 = 130;

#[derive(Clone)]
pub struct BinaryParser {}

impl BinaryParser {
    pub fn new() -> BinaryParser {
        BinaryParser {}
    }
}

impl<R: AsyncBufRead + AsyncBufReadExt + std::marker::Unpin> Parser<R> for BinaryParser {
    async fn parse(&self, reader: &mut R) -> io::Result<Command> {
        let fst = reader.read_u8().await;
        match fst {
            Ok(command) => match command {
                HELP_BIN => Ok(Command::Help),
                SIZE_BIN => {
                    let canvas = reader.read_u8().await?;
                    Ok(Command::Size(canvas))
                }
                GET_PX_BIN => {
                    let canvas = reader.read_u8().await?;
                    let horizontal = reader.read_u16_le().await?;
                    let vertical = reader.read_u16_le().await?;
                    Ok(Command::GetPixel(canvas, horizontal, vertical))
                }
                SET_PX_W_BIN => {
                    let canvas = reader.read_u8().await?;
                    let horizontal = reader.read_u16_le().await?;
                    let vertical = reader.read_u16_le().await?;
                    let white = reader.read_u8().await?;
                    Ok(Command::SetPixel(
                        canvas,
                        horizontal,
                        vertical,
                        Color::W8(white),
                    ))
                }
                SET_PX_RGB_BIN => {
                    let canvas = reader.read_u8().await?;
                    let horizontal = reader.read_u16_le().await?;
                    let vertical = reader.read_u16_le().await?;
                    let red = reader.read_u8().await?;
                    let green = reader.read_u8().await?;
                    let blue = reader.read_u8().await?;
                    Ok(Command::SetPixel(
                        canvas,
                        horizontal,
                        vertical,
                        Color::RGB24(red, green, blue),
                    ))
                }
                SET_PX_RGBA_BIN => {
                    let canvas = reader.read_u8().await?;
                    let horizontal = reader.read_u16_le().await?;
                    let vertical = reader.read_u16_le().await?;
                    let red = reader.read_u8().await?;
                    let green = reader.read_u8().await?;
                    let blue = reader.read_u8().await?;
                    let alpha = reader.read_u8().await?;
                    Ok(Command::SetPixel(
                        canvas,
                        horizontal,
                        vertical,
                        Color::RGBA32(red, green, blue, alpha),
                    ))
                }
                _ => {
                    eprintln!("received illegal command: {command}");
                    Err(Error::from(ErrorKind::InvalidInput))
                }
            },
            Err(err) => {
                eprintln!("{err}");
                Err(err)
            }
        }
    }
}

impl IOProtocol for BinaryParser {
    fn change_canvas(&mut self, _canvas: Canvas) -> io::Result<()> {
        Err(Error::from(ErrorKind::Unsupported))
    }
}

impl<W: AsyncWriteExt + std::marker::Unpin> Responder<W> for BinaryParser {
    async fn unparse(&self, response: Response, writer: &mut W) -> io::Result<()> {
        match response {
            Response::Help => {
                let help_text = format!(
"
You found the binary protocol help text
you can get this by sending ({HELP_BIN:02X}) to the server
To get the size of a canvas, send ({SIZE_BIN:02X}) (u8 canvas) to the server
To set a pixel using RGB, use ({SET_PX_RGB_BIN:02X}) (u8 canvas) (x as u16_le) (y as u16_le) (u8 r) (u8 g) (u8 b)
",
);
                writer.write_all(help_text.as_bytes()).await
            }
            Response::Size(x, y) => {
                writer.write_u16_le(x).await?;
                writer.write_u16_le(y).await
            }
            Response::GetPixel(_, _, c) => {
                writer.write_u8(c[0]).await?;
                writer.write_u8(c[1]).await?;
                writer.write_u8(c[2]).await
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
    async fn test_bin_help_parse() {
        let parser = BinaryParser::new();
        let reader = tokio_test::io::Builder::new().read(&[HELP_BIN]).build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::Help);
    }

    #[tokio::test]
    async fn test_bin_size_parse() {
        let parser = BinaryParser::new();
        let reader = tokio_test::io::Builder::new().read(&[SIZE_BIN, 3]).build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::Size(3));
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
        );
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
        );
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
        );
    }

    #[tokio::test]
    async fn test_bin_px_get_parse() {
        let parser = BinaryParser::new();
        let reader = tokio_test::io::Builder::new()
            .read(&[GET_PX_BIN, 0x03, 0x69, 0x42, 0x42, 0x69])
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::GetPixel(3, 0x4269, 0x6942));
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
