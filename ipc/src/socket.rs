use crate::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::io::Error;
use tokio::net::unix::{ReadHalf, SocketAddr, WriteHalf};
use tokio::net::UnixStream;
use tokio_serde::formats::SymmetricalBincode;
use tokio_serde::SymmetricallyFramed;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

#[derive(Debug)]
pub struct Socket<'a, In, Out> {
    address: SocketAddr,
    reader: SymmetricallyFramed<
        FramedRead<ReadHalf<'a>, LengthDelimitedCodec>,
        In,
        SymmetricalBincode<In>,
    >,
    writer: SymmetricallyFramed<
        FramedWrite<WriteHalf<'a>, LengthDelimitedCodec>,
        Out,
        SymmetricalBincode<Out>,
    >,
}

impl<'a, In, Out> Socket<'a, In, Out>
where
    for<'b> In: Deserialize<'b> + Unpin,
    Out: Serialize + Unpin,
{
    pub fn new(address: SocketAddr, stream: &'a mut UnixStream) -> Self {
        let (stream_read, stream_write) = stream.split();
        let length_delimited_read = FramedRead::new(stream_read, LengthDelimitedCodec::new());
        let reader = tokio_serde::SymmetricallyFramed::new(
            length_delimited_read,
            SymmetricalBincode::default(),
        );
        let length_delimited_write = FramedWrite::new(stream_write, LengthDelimitedCodec::new());
        let writer = tokio_serde::SymmetricallyFramed::new(
            length_delimited_write,
            SymmetricalBincode::default(),
        );

        Self {
            address,
            reader,
            writer,
        }
    }

    pub async fn read(&mut self) -> Option<Result<In, Error>> {
        self.reader.next().await
    }

    pub async fn try_read(&mut self) -> Result<Option<In>, Error> {
        self.reader.try_next().await
    }

    pub async fn send(&mut self, out: Out) -> Result<(), Error> {
        self.writer.send(out).await
    }

    pub fn address(&self) -> &SocketAddr {
        &self.address
    }
}
