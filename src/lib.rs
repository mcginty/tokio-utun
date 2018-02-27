//! MIO bindings for Unix Domain Sockets

#![cfg(unix)]
#![doc(html_root_url = "https://docs.rs/tokio-utun")]

#[macro_use]
extern crate futures;
#[macro_use]
extern crate tokio_core;
extern crate mio_utun;
#[macro_use]
extern crate log;

mod frame;
pub use self::frame::{UtunCodec, UtunFramed};

use std::io::{self, Read, Write};
use std::os::unix::io::{FromRawFd, RawFd};
use futures::Async;
use tokio_core::reactor::{PollEvented, Handle};

/// The primary class for this crate, a stream of tunneled traffic.
#[derive(Debug)]
pub struct UtunStream {
    io: PollEvented<mio_utun::UtunStream>
}

impl UtunStream {
    pub fn connect(name: &str, handle: &Handle) -> io::Result<UtunStream> {
        let stream = mio_utun::UtunStream::connect(name)?;
        let io = PollEvented::new(stream, handle)?;
        Ok(UtunStream { io })
    }

    pub fn from_fd(fd: RawFd, handle: &Handle) -> io::Result<UtunStream> {
        let stream = unsafe { mio_utun::UtunStream::from_raw_fd(fd) };
        let io = PollEvented::new(stream, handle)?;
        Ok(UtunStream { io })
    }

    /// Provides a `Stream` and `Sink` interface for reading and writing to this
    /// `UdpSocket` object, using the provided `UdpCodec` to read and write the
    /// raw data.
    ///
    /// Raw UDP sockets work with datagrams, but higher-level code usually
    /// wants to batch these into meaningful chunks, called "frames". This
    /// method layers framing on top of this socket by using the `UdpCodec`
    /// trait to handle encoding and decoding of messages frames. Note that
    /// the incoming and outgoing frame types may be distinct.
    ///
    /// This function returns a *single* object that is both `Stream` and
    /// `Sink`; grouping this into a single object is often useful for layering
    /// things which require both read and write access to the underlying
    /// object.
    ///
    /// If you want to work more directly with the streams and sink, consider
    /// calling `split` on the `UdpFramed` returned by this method, which will
    /// break them into separate objects, allowing them to interact more
    /// easily.
    pub fn framed<C: UtunCodec>(self, codec: C) -> UtunFramed<C> {
        frame::new(self, codec)
    }

    /// Test whether this socket is ready to be read or not.
    ///
    /// If the socket is *not* readable then the current task is scheduled to
    /// get a notification when the socket does become readable. That is, this
    /// is only suitable for calling in a `Future::poll` method and will
    /// automatically handle ensuring a retry once the socket is readable again.
    pub fn poll_read(&self) -> Async<()> {
        self.io.poll_read()
    }

    /// Test whether this socket is ready to be written to or not.
    ///
    /// If the socket is *not* writable then the current task is scheduled to
    /// get a notification when the socket does become writable. That is, this
    /// is only suitable for calling in a `Future::poll` method and will
    /// automatically handle ensuring a retry once the socket is writable again.
    pub fn poll_write(&self) -> Async<()> {
        self.io.poll_write()
    }
}

impl Read for UtunStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.read(buf)
    }
}

impl Write for UtunStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}
