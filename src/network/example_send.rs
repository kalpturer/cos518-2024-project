use crate::{network::message::CustomMessage, types::Address};
use smol::{
    io::{self, AsyncWriteExt},
    Async,
};
use std::net::TcpStream;

pub fn run_client(addr: Address) -> io::Result<()> {
    smol::block_on(async {
        // Connect to the server and create async stdin and stdout.
        let stream = Async::<TcpStream>::connect(addr).await?;

        // Intro messages.
        println!("Connected to {}", stream.get_ref().peer_addr()?);

        let mut writer = &stream;

        let line = "Hello";

        let mes: CustomMessage = CustomMessage {
            a_field: line.to_string(),
            b_field: 5,
        };
        let res = writer
            .write_all(serde_json::to_string(&mes).ok().unwrap().as_bytes())
            .await;

        res
    })
}
