use std::io::{self, Error, ErrorKind};

use image::EncodableLayout;
use rand::random;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use crate::{Canvas, Color, Command, Response};

use super::{IOProtocol, Parser, Responder};

const SIZE_BIN: u8 = 115;
const PROTOCOLS_BIN: u8 = 116;
const HELP_BIN: u8 = 104;
const GET_PX_BIN: u8 = 32;
const SET_PX_PALETTE_BIN: u8 = 33;
#[cfg(feature = "palette")]
const SET_PALETTE_COLOR: u8 = 34;

#[derive(Clone)]
pub struct PaletteParser {
    colors: [Color; 256],
}

impl PaletteParser {
    pub fn set_color(&mut self, index: u8, color: Color) {
        self.colors[index as usize] = color;
    }
}

impl Default for PaletteParser {
    fn default() -> Self {
        PaletteParser {
            colors: [0; 256].map(|_| random()),
        }
    }
}

impl<R: AsyncBufRead + AsyncBufReadExt + std::marker::Unpin> Parser<R> for PaletteParser {
    async fn parse(&self, reader: &mut R) -> io::Result<Command> {
        let fst = reader.read_u8().await;
        match fst {
            Ok(command) => match command {
                HELP_BIN => Ok(Command::Help),
                PROTOCOLS_BIN => Ok(Command::Protocols),
                SIZE_BIN => {
                    let canvas = reader.read_u8().await?;
                    Ok(Command::Size(canvas))
                }
                GET_PX_BIN => {
                    let canvas = reader.read_u8().await?;
                    let horizontal = reader.read_u16().await?;
                    let vertical = reader.read_u16().await?;
                    Ok(Command::GetPixel(canvas, horizontal, vertical))
                }
                SET_PX_PALETTE_BIN => {
                    let canvas = reader.read_u8().await?;
                    let horizontal = reader.read_u16().await?;
                    let vertical = reader.read_u16().await?;
                    let color = reader.read_u8().await?;
                    Ok(Command::SetPixel(canvas, horizontal, vertical, unsafe {
                        self.colors.get_unchecked(color as usize).clone()
                    }))
                }
                #[cfg(feature = "palette")]
                SET_PALETTE_COLOR => {
                    let index = reader.read_u8().await?;
                    let r = reader.read_u8().await?;
                    let g = reader.read_u8().await?;
                    let b = reader.read_u8().await?;
                    Ok(Command::ChangeColor(index, Color::RGB24(r, g, b)))
                }
                _ => {
                    tracing::error!("received illegal command: {command}");
                    Err(Error::from(ErrorKind::InvalidInput))
                }
            },
            Err(err) => {
                tracing::error!("{err}");
                Err(err)
            }
        }
    }
}

impl IOProtocol for PaletteParser {
    fn change_canvas(&mut self, _canvas: Canvas) -> io::Result<()> {
        Err(Error::from(ErrorKind::Unsupported))
    }
}

impl<W: AsyncWriteExt + std::marker::Unpin> Responder<W> for PaletteParser {
    async fn unparse(&self, response: Response, writer: &mut W) -> io::Result<()> {
        match response {
            Response::Help => {
                writer
                    .write_all(
                        self.colors
                            .iter()
                            .map(|c| c.to_bytes())
                            .collect::<Vec<_>>()
                            .concat()
                            .as_bytes(),
                    )
                    .await
            }
            Response::Protocols(protos) => {
                for protocol in protos {
                    match protocol {
                        crate::ProtocolStatus::Enabled(proto) => {
                            writer
                                .write_all(format!("Enabled: {}\n", proto).as_bytes())
                                .await?;
                        }
                        crate::ProtocolStatus::Disabled(proto) => {
                            writer
                                .write_all(format!("Disabled: {}\n", proto).as_bytes())
                                .await?;
                        }
                    }
                }
                Ok(())
            }
            Response::Size(x, y) => {
                writer.write_u16(x).await?;
                writer.write_u16(y).await
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
    async fn test_palette_px_set_parse() {
        let parser = PaletteParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(&[SET_PX_PALETTE_BIN, 0x01, 0x69, 0x42, 0x42, 0x69, 0x82])
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await.unwrap();
        assert_eq!(
            thingy,
            Command::SetPixel(1, 0x6942, 0x4269, parser.colors[0x82].clone())
        );
    }

    #[cfg(feature = "palette")]
    #[tokio::test]
    async fn test_palette_set_palette_color_parse() {
        let parser = PaletteParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(&[SET_PALETTE_COLOR, 0x04, 0xfa, 0xbd, 0x2f])
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await.unwrap();
        assert_eq!(
            thingy,
            Command::ChangeColor(0x04, Color::RGB24(0xfa, 0xbd, 0x2f))
        );
    }

    #[tokio::test]
    async fn test_palette_set_color() {
        let mut parser = PaletteParser::default();
        parser.set_color(0x04, Color::RGB24(0xfa, 0xbd, 0x2f));
        parser.set_color(0x13, Color::RGB24(0x23, 0x45, 0x67));
        parser.set_color(0x69, Color::RGB24(0xb0, 0x07, 0x1e));
        assert_eq!(parser.colors[0x04], Color::RGB24(0xfa, 0xbd, 0x2f));
        assert_eq!(parser.colors[0x13], Color::RGB24(0x23, 0x45, 0x67));
        assert_eq!(parser.colors[0x69], Color::RGB24(0xb0, 0x07, 0x1e));
    }
}
