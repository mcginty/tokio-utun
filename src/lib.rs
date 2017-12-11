//! MIO bindings for Unix Domain Sockets

#![cfg(unix)]
#![doc(html_root_url = "https://docs.rs/tokio-utun")]

extern crate bytes;
#[macro_use]
extern crate futures;
extern crate iovec;
extern crate libc;
#[macro_use]
extern crate tokio_core;
extern crate tokio_io;
extern crate mio;
extern crate mio_utun;
#[macro_use]
extern crate log;

mod frame;
pub use self::frame::{UtunCodec, UtunFramed};

use std::io;
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
        Ok(UtunStream { io: io })
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


    /// Sends data on the socket to the address previously bound via connect().
    /// On success, returns the number of bytes written.
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        if let Async::NotReady = self.io.poll_write() {
            return Err(io::ErrorKind::WouldBlock.into())
        }
        match self.io.get_ref().send(buf) {
            Ok(n) => Ok(n),
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    self.io.need_write();
                }
                Err(e)
            }
        }
    }

    /// Receives data from the socket previously bound with connect().
    /// On success, returns the number of bytes read.
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        if let Async::NotReady = self.io.poll_read() {
            return Err(io::ErrorKind::WouldBlock.into())
        }
        match self.io.get_ref().recv(buf) {
            Ok(n) => Ok(n),
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    self.io.need_read();
                }
                Err(e)
            }
        }
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
