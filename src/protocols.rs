mod binary_protocol;
mod text_protocol;

use std::io;

pub use binary_protocol::BinaryParser;
pub use text_protocol::TextParser;
use tokio::io::AsyncWriteExt;

use crate::{Canvas, Command, Response};

pub(crate) trait Parser<R>
where
    R: std::marker::Unpin + tokio::io::AsyncBufRead,
{
    async fn parse(&self, reader: &mut R) -> io::Result<Command>;
}

pub(crate) trait IOProtocol {
    fn change_canvas(&mut self, canvas: Canvas) -> io::Result<()>;
}

pub(crate) trait Responder<W>
where
    W: AsyncWriteExt + std::marker::Unpin,
{
    async fn unparse(&self, response: Response, writer: &mut W) -> io::Result<()>;
}
