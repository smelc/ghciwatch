use async_dup::Arc;
use async_dup::Mutex;
use std::fmt::Debug;
use std::io;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncWrite;
use tokio::io::DuplexStream;
use tokio::io::Sink;
use tokio::io::Stderr;
use tokio::io::Stdout;
use tokio_util::compat::Compat;
use tokio_util::compat::FuturesAsyncWriteCompatExt;
use tokio_util::compat::TokioAsyncWriteCompatExt;

/// A dynamically reconfigurable sink for `ghci` process output. Built for use in `GhciOpts`, but
/// usable as a general purpose clonable [`AsyncWrite`]r.
#[derive(Debug)]
pub struct GhciWriter {
    kind: Kind,
    file: Option<std::fs::File>,
    buffer: Option<tokio::io::BufWriter>, // TODO Keep a buffer for writing to file *OR* make a
                                          // tee implementation to split the stream into two
                                          // outputs
}

#[derive(Debug)]
enum Kind {
    Stdout(Stdout),
    Stderr(Stderr),
    DuplexStream(Compat<Arc<Mutex<Compat<DuplexStream>>>>),
    Sink(Sink),
}

impl GhciWriter {
    /// Write to `stdout`.
    pub fn stdout() -> Self {
        Self {
            kind: Kind::Stdout(tokio::io::stdout()),
            file: None,
        }
    }

    /// Write to `stderr`.
    pub fn stderr() -> Self {
        Self {
            kind: Kind::Stderr(tokio::io::stderr()),
            file: None,
        }
    }

    /// Write to an in-memory buffer.
    pub fn duplex_stream(duplex_stream: DuplexStream) -> Self {
        Self {
            kind: Kind::DuplexStream(
                Arc::new(Mutex::new(duplex_stream.compat_write())).compat_write(),
            ),
            file: None,
        }
    }

    /// Write to the void.
    pub fn sink() -> Self {
        Self {
            kind: Kind::Sink(tokio::io::sink()),
            file: None,
        }
    }
}

impl AsyncWrite for GhciWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let inner = Pin::into_inner(self);

        match inner.kind {
            Kind::Stdout(ref mut x) => Pin::new(x).poll_write(cx, buf),
            Kind::Stderr(ref mut x) => Pin::new(x).poll_write(cx, buf),
            Kind::DuplexStream(ref mut x) => Pin::new(x).poll_write(cx, buf),
            Kind::Sink(ref mut x) => Pin::new(x).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match Pin::into_inner(self).kind {
            Kind::Stdout(ref mut x) => Pin::new(x).poll_flush(cx),
            Kind::Stderr(ref mut x) => Pin::new(x).poll_flush(cx),
            Kind::DuplexStream(ref mut x) => Pin::new(x).poll_flush(cx),
            Kind::Sink(ref mut x) => Pin::new(x).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match Pin::into_inner(self).kind {
            Kind::Stdout(ref mut x) => Pin::new(x).poll_shutdown(cx),
            Kind::Stderr(ref mut x) => Pin::new(x).poll_shutdown(cx),
            Kind::DuplexStream(ref mut x) => Pin::new(x).poll_shutdown(cx),
            Kind::Sink(ref mut x) => Pin::new(x).poll_shutdown(cx),
        }
    }
}

impl Clone for GhciWriter {
    fn clone(&self) -> Self {
        match &self.kind {
            Kind::Stdout(_) => Self::stdout(),
            Kind::Stderr(_) => Self::stderr(),
            Kind::DuplexStream(x) => Self {
                kind: Kind::DuplexStream(x.clone()),
                file: None, // FIXME
            },
            Kind::Sink(_) => Self::sink(),
        }
    }
}
