use std::io::{self, Error, ErrorKind};

use image::EncodableLayout;
use rand::random;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use crate::{Canvas, Color, Command, Response};

use super::{IOProtocol, Parser, Responder};

const SIZE_BIN: u8 = 115;
const HELP_BIN: u8 = 104;
const GET_PX_BIN: u8 = 32;
const SET_PX_PALETTE_BIN: u8 = 33;

#[derive(Clone)]
pub struct PaletteParser {
    colors: [Color; 256],
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
}
