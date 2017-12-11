use std::io;

use futures::{Async, Poll, Stream, Sink, StartSend, AsyncSink};

use UtunStream;

/// Encoding of frames via buffers.
///
/// This trait is used when constructing an instance of `UdpFramed` and provides
/// the `In` and `Out` types which are decoded and encoded from the socket,
/// respectively.
///
/// Because UDP is a connectionless protocol, the `decode` method receives the
/// address where data came from and the `encode` method is also responsible for
/// determining the remote host to which the datagram should be sent
///
/// The trait itself is implemented on a type that can track state for decoding
/// or encoding, which is particularly useful for streaming parsers. In many
/// cases, though, this type will simply be a unit struct (e.g. `struct
/// HttpCodec`).
pub trait UtunCodec {
    /// The type of decoded frames.
    type In;

    /// The type of frames to be encoded.
    type Out;

    /// Attempts to decode a frame from the provided buffer of bytes.
    ///
    /// This method is called by `UdpFramed` on a single datagram which has been
    /// read from a socket. The `buf` argument contains the data that was
    /// received from the remote address, and `src` is the address the data came
    /// from. Note that typically this method should require the entire contents
    /// of `buf` to be valid or otherwise return an error with trailing data.
    ///
    /// Finally, if the bytes in the buffer are malformed then an error is
    /// returned indicating why. This informs `Framed` that the stream is now
    /// corrupt and should be terminated.
    fn decode(&mut self, buf: &[u8]) -> io::Result<Self::In>;

    /// Encodes a frame into the buffer provided.
    ///
    /// This method will encode `msg` into the byte buffer provided by `buf`.
    /// The `buf` provided is an internal buffer of the `Framed` instance and
    /// will be written out when possible.
    ///
    /// The encode method also determines the destination to which the buffer
    /// should be directed, which will be returned as a `SocketAddr`.
    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>);
}

/// A unified `Stream` and `Sink` interface to an underlying `UdpSocket`, using
/// the `UdpCodec` trait to encode and decode frames.
///
/// You can acquire a `UdpFramed` instance by using the `UdpSocket::framed`
/// adapter.
#[must_use = "sinks do nothing unless polled"]
pub struct UtunFramed<C> {
    socket: UtunStream,
    codec: C,
    rd: Vec<u8>,
    wr: Vec<u8>,
    flushed: bool,
}

impl<C: UtunCodec> Stream for UtunFramed<C> {
    type Item = C::In;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<C::In>, io::Error> {
        let n = try_nb!(self.socket.recv(&mut self.rd));
        trace!("received {} bytes, decoding", n);
        let frame = try!(self.codec.decode(&self.rd[..n]));
        trace!("frame decoded from buffer");
        Ok(Async::Ready(Some(frame)))
    }
}

impl<C: UtunCodec> Sink for UtunFramed<C> {
    type SinkItem = C::Out;
    type SinkError = io::Error;

    fn start_send(&mut self, item: C::Out) -> StartSend<C::Out, io::Error> {
        trace!("sending frame");

        if !self.flushed {
            match try!(self.poll_complete()) {
                Async::Ready(()) => {},
                Async::NotReady => return Ok(AsyncSink::NotReady(item)),
            }
        }

        self.codec.encode(item, &mut self.wr);
        self.flushed = false;
        trace!("frame encoded; length={}", self.wr.len());

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        if self.flushed {
            return Ok(Async::Ready(()))
        }

        trace!("flushing frame; length={}", self.wr.len());
        let n = try_nb!(self.socket.send(&self.wr));
        trace!("written {}", n);

        let wrote_all = n == self.wr.len();
        self.wr.clear();
        self.flushed = true;

        if wrote_all {
            Ok(Async::Ready(()))
        } else {
            Err(io::Error::new(io::ErrorKind::Other,
                               "failed to write entire datagram to socket"))
        }
    }

    fn close(&mut self) -> Poll<(), io::Error> {
        try_ready!(self.poll_complete());
        Ok(().into())
    }
}

pub fn new<C: UtunCodec>(socket: UtunStream, codec: C) -> UtunFramed<C> {
    UtunFramed {
        socket: socket,
        codec: codec,
        rd: vec![0; 64 * 1024],
        wr: Vec::with_capacity(8 * 1024),
        flushed: true,
    }
}

impl<C> UtunFramed<C> {
    /// Returns a reference to the underlying I/O stream wrapped by `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise being
    /// worked with.
    pub fn get_ref(&self) -> &UtunStream {
        &self.socket
    }

    /// Returns a mutable reference to the underlying I/O stream wrapped by
    /// `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise being
    /// worked with.
    pub fn get_mut(&mut self) -> &mut UtunStream {
        &mut self.socket
    }

    /// Consumes the `Framed`, returning its underlying I/O stream.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise being
    /// worked with.
    pub fn into_inner(self) -> UtunStream {
        self.socket
    }
}
