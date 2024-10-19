use std::io::{self, Error, ErrorKind};

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use crate::{Canvas, Color, Command, LockableCommand, Response};

use super::{IOProtocol, Parser, Responder};

const SIZE_BIN: u8 = 115;
const SET_CANVAS_BIN: u8 = 116;
const HELP_BIN: u8 = 104;
const GET_PX_BIN: u8 = 32;
const SET_PX_RGB_BIN: u8 = 128;
const SET_PX_RGBA_BIN: u8 = 129;
const SET_PX_W_BIN: u8 = 130;
const LOCK: u8 = 192;

#[derive(Clone, Default)]
pub struct StateParser {
    canvas: Canvas,
}

impl StateParser {
    async fn parse_locked<R>(&self, reader: &mut R) -> io::Result<Vec<LockableCommand>>
    where
        R: AsyncBufRead + AsyncBufReadExt + std::marker::Unpin,
    {
        let amount = reader.read_u16().await?;
        let command = reader.read_u8().await?;

        let command_length = match command {
            SET_PX_RGB_BIN => 7,
            SET_PX_RGBA_BIN => 8,
            SET_PX_W_BIN => 5,
            _ => panic!("command not supported"),
        };

        let lockmask = reader.read_u8().await?;
        let mut buf = vec![0; lockmask.count_ones() as usize];
        let static_amount = reader.read_exact(&mut buf).await?;
        let mut j = 0;

        let static_spreaded: Vec<_> = (0..command_length)
            .map(|i| {
                println!("i is {}, lockmask is {:?}", i, lockmask);
                match lockmask >> (7 - i) & 1 {
                    1 => {
                        let bj = Some(buf[j]);
                        j += 1;
                        bj
                    }
                    0 => None,
                    _ => panic!("lockmask shift was not 0 or 1"),
                }
            })
            .collect();

        debug_assert_eq!(
            static_amount,
            static_spreaded.iter().filter(|x| x.is_some()).count()
        );
        debug_assert_eq!(j, buf.len());

        let pack_fun = |cmd: Vec<u8>| {
            let x = u16::from_be_bytes([cmd[0], cmd[1]]);
            let y = u16::from_be_bytes([cmd[2], cmd[3]]);
            let color = match command {
                SET_PX_RGB_BIN => Color::RGB24(cmd[4], cmd[5], cmd[6]),
                SET_PX_RGBA_BIN => Color::RGBA32(cmd[4], cmd[5], cmd[6], cmd[7]),
                SET_PX_W_BIN => Color::W8(cmd[4]),
                _ => panic!("command does not exist"),
            };
            LockableCommand::SetPixel(self.canvas, x, y, color)
        };

        let mut commands = Vec::with_capacity(amount as usize);
        for _ in 0..(amount as usize) {
            let mut res = Vec::with_capacity(command_length);
            for v in static_spreaded.iter() {
                res.push(match v {
                    Some(val) => *val,
                    None => reader.read_u8().await?,
                });
            }
            println!("{:?}", res);
            commands.push(pack_fun(res));
        }

        Ok(commands)
    }

    async fn parse_unlocked<R>(&self, reader: &mut R) -> io::Result<Command>
    where
        R: AsyncBufRead + AsyncBufReadExt + Unpin,
    {
        let fst = reader.read_u8().await;
        match fst {
            Ok(command) => match command {
                HELP_BIN => Ok(Command::Help),
                SET_CANVAS_BIN => {
                    let canvas = reader.read_u8().await?;
                    Ok(Command::ChangeCanvas(canvas))
                }
                SIZE_BIN => Ok(Command::Size(self.canvas)),
                GET_PX_BIN => {
                    let horizontal = reader.read_u16().await?;
                    let vertical = reader.read_u16().await?;
                    Ok(Command::GetPixel(self.canvas, horizontal, vertical))
                }
                SET_PX_W_BIN => {
                    let horizontal = reader.read_u16().await?;
                    let vertical = reader.read_u16().await?;
                    let white = reader.read_u8().await?;
                    Ok(Command::SetPixel(
                        self.canvas,
                        horizontal,
                        vertical,
                        Color::W8(white),
                    ))
                }
                SET_PX_RGB_BIN => {
                    let horizontal = reader.read_u16().await?;
                    let vertical = reader.read_u16().await?;
                    let red = reader.read_u8().await?;
                    let green = reader.read_u8().await?;
                    let blue = reader.read_u8().await?;
                    Ok(Command::SetPixel(
                        self.canvas,
                        horizontal,
                        vertical,
                        Color::RGB24(red, green, blue),
                    ))
                }
                SET_PX_RGBA_BIN => {
                    let horizontal = reader.read_u16().await?;
                    let vertical = reader.read_u16().await?;
                    let red = reader.read_u8().await?;
                    let green = reader.read_u8().await?;
                    let blue = reader.read_u8().await?;
                    let alpha = reader.read_u8().await?;
                    Ok(Command::SetPixel(
                        self.canvas,
                        horizontal,
                        vertical,
                        Color::RGBA32(red, green, blue, alpha),
                    ))
                }
                LOCK => self.parse_locked(reader).await.map(Command::Multiple),
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

impl<R> Parser<R> for StateParser
where
    R: AsyncBufRead + AsyncBufReadExt + std::marker::Unpin,
{
    async fn parse(&self, reader: &mut R) -> io::Result<Command> {
        self.parse_unlocked(reader).await
    }
}

impl IOProtocol for StateParser {
    fn change_canvas(&mut self, _canvas: Canvas) -> io::Result<()> {
        Err(Error::from(ErrorKind::Unsupported))
    }
}

impl<W> Responder<W> for StateParser
where
    W: AsyncWriteExt + std::marker::Unpin,
{
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
    use crate::LockableCommand;

    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn test_bin_help_parse() {
        let parser = StateParser::default();
        let reader = tokio_test::io::Builder::new().read(&[HELP_BIN]).build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::Help);
    }

    #[tokio::test]
    async fn test_bin_size_parse() {
        let parser = StateParser::default();
        let reader = tokio_test::io::Builder::new().read(&[SIZE_BIN]).build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::Size(0));
    }

    #[tokio::test]
    async fn test_lock_parse() {
        let parser = StateParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(&[
                LOCK,
                0,
                4,
                SET_PX_RGB_BIN,
                0b10100110,
                0x31,
                0x32,
                0x88,
                0x92,
            ])
            .read(&[0x00, 0xaa, 0x10])
            .read(&[0x11, 0xbb, 0x20])
            .read(&[0x22, 0xcc, 0x30])
            .read(&[0x33, 0xdd, 0x40])
            .build();
        let mut bufreader = BufReader::new(reader);
        assert_eq!(
            parser.parse(&mut bufreader).await.unwrap(),
            Command::Multiple(vec![
                LockableCommand::SetPixel(0, 0x3100, 0x32aa, Color::RGB24(0x10, 0x88, 0x92)),
                LockableCommand::SetPixel(0, 0x3111, 0x32bb, Color::RGB24(0x20, 0x88, 0x92)),
                LockableCommand::SetPixel(0, 0x3122, 0x32cc, Color::RGB24(0x30, 0x88, 0x92)),
                LockableCommand::SetPixel(0, 0x3133, 0x32dd, Color::RGB24(0x40, 0x88, 0x92))
            ])
        );
    }

    #[tokio::test]
    async fn test_canvas_parse() {
        let parser = StateParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(&[SET_CANVAS_BIN, 3])
            .build();
        let mut bufreader = BufReader::new(reader);
        assert_eq!(
            parser.parse(&mut bufreader).await.unwrap(),
            Command::ChangeCanvas(3)
        );
    }

    #[tokio::test]
    async fn test_bin_px_set_w_parse() {
        let parser = StateParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(&[SET_PX_W_BIN, 0x69, 0x42, 0x42, 0x69, 0x82])
            .build();
        let mut bufreader = BufReader::new(reader);
        assert_eq!(
            parser.parse(&mut bufreader).await.unwrap(),
            Command::SetPixel(0, 0x6942, 0x4269, Color::W8(0x82))
        );
    }

    #[tokio::test]
    async fn test_bin_px_set_rgb_parse() {
        let parser = StateParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(&[SET_PX_RGB_BIN, 0x42, 0x69, 0x69, 0x42, 0x82, 0x00, 0xff])
            .build();
        let mut bufreader = BufReader::new(reader);
        assert_eq!(
            parser.parse(&mut bufreader).await.unwrap(),
            Command::SetPixel(0, 0x4269, 0x6942, Color::RGB24(0x82, 0x00, 0xff))
        );
    }

    #[tokio::test]
    async fn test_bin_px_set_rgba_parse() {
        let parser = StateParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(&[
                SET_PX_RGBA_BIN,
                0x42,
                0x69,
                0x69,
                0x42,
                0x82,
                0x00,
                0xff,
                0xa0,
            ])
            .build();
        let mut bufreader = BufReader::new(reader);
        assert_eq!(
            parser.parse(&mut bufreader).await.unwrap(),
            Command::SetPixel(0, 0x4269, 0x6942, Color::RGBA32(0x82, 0x00, 0xff, 0xa0))
        );
    }

    #[tokio::test]
    async fn test_bin_px_get_parse() {
        let parser = StateParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(&[GET_PX_BIN, 0x69, 0x42, 0x42, 0x69])
            .build();
        let mut bufreader = BufReader::new(reader);
        let thingy = parser.parse(&mut bufreader).await;
        assert_eq!(thingy.unwrap(), Command::GetPixel(0, 0x6942, 0x4269));
    }

    #[tokio::test]
    async fn test_bin_parse_multiple() {
        let parser = StateParser::default();
        let reader = tokio_test::io::Builder::new()
            .read(&[SET_PX_RGB_BIN, 0x69, 0x42, 0x42, 0x69, 0x82, 0x00, 0xff])
            .read(&[
                SET_PX_RGBA_BIN,
                0x69,
                0x42,
                0x42,
                0x70,
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
            Command::SetPixel(0, 0x6942, 0x4269, Color::RGB24(0x82, 0x00, 0xff))
        );
        assert_eq!(
            thingy2.unwrap(),
            Command::SetPixel(0, 0x6942, 0x4270, Color::RGBA32(0x82, 0x00, 0xff, 0xa0))
        );
    }
}
