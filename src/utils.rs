use std::task::Poll;

use tokio::io::{AsyncRead, AsyncWrite};

pub struct RepeatSome {
    bytes: &'static [u8],
    len: usize,
}

impl RepeatSome {
    pub fn new(bytes: &'static [u8]) -> Self {
        RepeatSome {
            bytes,
            len: bytes.len(),
        }
    }
}

impl AsyncRead for RepeatSome {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        while buf.remaining() > self.len {
            buf.put_slice(self.bytes)
        }
        Poll::Ready(Ok(()))
    }
}

#[derive(Default)]
pub struct Drain {}

impl AsyncWrite for Drain {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
}
