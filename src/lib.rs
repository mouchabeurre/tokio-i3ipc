// re-export i3ipc-types so users only have to import 1 thing
pub use i3ipc_types::*;
mod codec;
pub use codec::*;

use bytes::{Buf, BufMut, ByteOrder, Bytes, BytesMut, IntoBuf, LittleEndian};
use futures::{try_ready, Async, Future, Poll};
use serde::de::DeserializeOwned;
use tokio::prelude::*;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_reactor::{Handle, PollEvented};
use tokio_uds::{ConnectFuture, UnixStream};

use std::{
    io::{self, Cursor, Read, Write},
    marker::PhantomData,
};

#[derive(Debug)]
pub struct AsyncI3(ConnectFuture);

trait AsyncConnect {
    type Stream: AsyncI3IPC;
    fn new() -> io::Result<Self>
    where
        Self: Sized;
}

trait AsyncI3IPC: AsyncRead + AsyncWrite + I3IPC {}
impl AsyncI3IPC for I3Stream {}
impl AsyncI3IPC for UnixStream {}

impl AsyncConnect for AsyncI3 {
    type Stream = UnixStream;
    fn new() -> io::Result<Self> {
        Ok(AsyncI3(UnixStream::connect(socket_path()?)))
    }
}

impl Future for AsyncI3 {
    type Item = UnixStream;
    type Error = io::Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let stream = try_ready!(self.0.poll());
        Ok(Async::Ready(stream))
    }
}

#[derive(Debug)]
pub struct I3Msg<D> {
    stream: UnixStream,
    _marker: PhantomData<D>,
}

impl<D: DeserializeOwned> Future for I3Msg<D> {
    type Item = MsgResponse<D>;
    type Error = io::Error;
    fn poll(&mut self) -> Poll<Self::Item, io::Error> {
        println!("here");
        let mut initial = BytesMut::with_capacity(1024);
        try_ready!(self.stream.read_buf(&mut initial));
        dbg!(&initial);
        println!("here1");
        if &initial[0..6] != MAGIC.as_bytes() {
            panic!("Magic str not received");
        }
        let payload_len = LittleEndian::read_u32(&initial[6..10]) as usize;
        dbg!(payload_len);
        let msg_type = LittleEndian::read_u32(&initial[10..14]);
        dbg!(msg_type);
        // try_ready!(self.stream.read_buf(&mut initial));

        Ok(Async::Ready(MsgResponse {
            msg_type: msg_type.into(),
            body: serde_json::from_slice(&initial[14..])?,
        }))
    }
}

#[derive(Debug)]
pub struct I3Stream(UnixStream);

impl I3Stream {
    pub const MAGIC: &'static str = "i3-ipc";

    pub fn send_msg<P>(&mut self, msg: msg::Msg, payload: P) -> Poll<usize, io::Error>
    where
        P: AsRef<str>,
    {
        let payload = payload.as_ref();
        let len = 14 + payload.len();
        let mut buf = BytesMut::with_capacity(len);
        buf.put_slice(I3Stream::MAGIC.as_bytes());
        buf.put_u32_le(payload.len() as u32);
        buf.put_u32_le(msg.into());
        buf.put_slice(payload.as_bytes());
        let mut n = 0;
        let mut buf = buf.into_buf();
        loop {
            n += try_ready!(self.write_buf(&mut buf));
            if n == len {
                return Ok(Async::Ready(len));
            }
        }
    }

    pub fn receive_msg<D: DeserializeOwned>(&mut self) -> Poll<MsgResponse<D>, io::Error> {
        let mut buf = BytesMut::with_capacity(6);
        let _ = try_ready!(self.read_buf(&mut buf));
        dbg!(&buf);
        let magic_str = buf.take();
        if magic_str != I3Stream::MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Expected 'i3-ipc' but received: {:?}", magic_str),
            ));
        }

        let len = try_ready!(self.read_buf(&mut buf));
        unimplemented!()
    }

    pub fn send_receive<D: DeserializeOwned>(&mut self) -> Poll<MsgResponse<D>, io::Error> {
        unimplemented!()
    }
}

impl Read for I3Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for I3Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl AsyncRead for I3Stream {
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [u8]) -> bool {
        self.0.prepare_uninitialized_buffer(buf)
    }

    fn read_buf<B>(&mut self, buf: &mut B) -> Poll<usize, io::Error>
    where
        B: BufMut,
    {
        self.0.read_buf(buf)
    }
}

impl AsyncWrite for I3Stream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        match self {
            I3Stream(s) => s.shutdown(),
        }
    }

    fn write_buf<B>(&mut self, buf: &mut B) -> Poll<usize, io::Error>
    where
        B: Buf,
    {
        self.0.write_buf(buf)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test() -> Result<(), Box<dyn std::error::Error>> {
//         let fut = I3Connect::new()?.connect().and_then(|stream| {
//             stream.subscribe(&[event::Event::Window]).map(|o| {dbg!(o); () }).map_err(|e| eprintln!("{:?}", e));
//             futures::ok(())
//         });
//         tokio::run(fut);
//         Ok(())
//     }
// }
