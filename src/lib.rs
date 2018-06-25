//! MIO bindings for Unix Domain Sockets

#![cfg(unix)]
#![doc(html_root_url = "https://docs.rs/tokio-utun")]

extern crate futures;
extern crate tokio_io;
extern crate log;
extern crate bytes;
extern crate mio;
extern crate mio_utun;
extern crate tokio_codec;
extern crate tokio_reactor;

use std::io::{self, Read, Write};
use std::os::unix::io::{FromRawFd, RawFd};
use bytes::{Buf, BufMut};
use futures::{Async, Poll};
use mio::Ready;
use tokio_reactor::{PollEvented, Handle};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_codec::{Framed, Encoder, Decoder};

/// The primary class for this crate, a stream of tunneled traffic.
#[derive(Debug)]
pub struct UtunStream {
    io: PollEvented<mio_utun::UtunStream>
}

impl UtunStream {
    pub fn connect(name: &str) -> io::Result<UtunStream> {
        let stream = mio_utun::UtunStream::connect(name)?;
        let io = PollEvented::new(stream);
        Ok(UtunStream { io })
    }

    pub fn connect_with_handle(name: &str, handle: &Handle) -> io::Result<UtunStream> {
        let stream = mio_utun::UtunStream::connect(name)?;
        let io = PollEvented::new_with_handle(stream, handle)?;
        Ok(UtunStream { io })
    }

    pub fn from_fd(fd: RawFd) -> UtunStream {
        let stream = unsafe { mio_utun::UtunStream::from_raw_fd(fd) };
        let io = PollEvented::new(stream);
        UtunStream { io }
    }

    pub fn from_fd_with_handle(fd: RawFd, handle: &Handle) -> io::Result<UtunStream> {
        let stream = unsafe { mio_utun::UtunStream::from_raw_fd(fd) };
        let io = PollEvented::new_with_handle(stream, handle)?;
        Ok(UtunStream { io })
    }

    pub fn name(&self) -> io::Result<String> {
        self.io.get_ref().name()
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
    pub fn framed<C: Decoder + Encoder>(self, codec: C) -> Framed<UtunStream, C> {
        Framed::new(self, codec)
    }

    /// Test whether this socket is ready to be read or not.
    ///
    /// If the socket is *not* readable then the current task is scheduled to
    /// get a notification when the socket does become readable. That is, this
    /// is only suitable for calling in a `Future::poll` method and will
    /// automatically handle ensuring a retry once the socket is readable again.
    pub fn poll_read_ready(&self, ready: Ready) -> Poll<Ready, io::Error> {
        self.io.poll_read_ready(ready)
    }

    /// Test whether this socket is ready to be written to or not.
    ///
    /// If the socket is *not* writable then the current task is scheduled to
    /// get a notification when the socket does become writable. That is, this
    /// is only suitable for calling in a `Future::poll` method and will
    /// automatically handle ensuring a retry once the socket is writable again.
    pub fn poll_write_ready(&self) -> Poll<Ready, io::Error> {
        self.io.poll_write_ready()
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

impl AsyncRead for UtunStream {
    unsafe fn prepare_uninitialized_buffer(&self, _: &mut [u8]) -> bool {
        false
    }

    fn read_buf<B: BufMut>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
        if let Async::NotReady = self.poll_read_ready(Ready::readable())? {
            return Ok(Async::NotReady);
        }

        unsafe {
            match self.io.read(buf.bytes_mut()) {
                Ok(n) => {
                    buf.advance_mut(n);
                    Ok(Async::Ready(n))
                },
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.io.clear_read_ready(Ready::readable())?;
                    Ok(Async::NotReady)
                },
                Err(e) => Err(e)
            }
        }
    }
}

impl AsyncWrite for UtunStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(().into())
    }

    fn write_buf<B: Buf>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
        if let Async::NotReady = self.poll_write_ready()? {
            return Ok(Async::NotReady);
        }

        match self.io.write(buf.bytes()) {
            Ok(n) => {
                buf.advance(n);
                Ok(Async::Ready(n))
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.io.clear_write_ready()?;
                Ok(Async::NotReady)
            },
            Err(e) => Err(e)
        }
    }
}