use std::net::{TcpStream, SocketAddr};
use smol::{future, io, Async, Unblock};
use smol::io::AsyncWriteExt;

pub fn run_client(addr : SocketAddr) -> io::Result<()> {
    smol::block_on(async {
        
        // Connect to the server and create async stdin and stdout.
        let stream = Async::<TcpStream>::connect(addr).await?;
        let stdin = Unblock::new(std::io::stdin());
        let mut stdout = Unblock::new(std::io::stdout());

        // Intro messages.
        println!("Connected to {}", stream.get_ref().peer_addr()?);
        println!("My nickname: {}", stream.get_ref().local_addr()?);
        println!("Type a message and hit enter!\n");

        let reader = &stream;
        let mut writer = &stream;

        // Wait until the standard input is closed or the connection is closed.
        future::race(
            async {
                let _ = writer.write_all("Client  ".as_bytes()).await;
                let res = io::copy(stdin, &mut writer).await;
                println!("Quit!");
                res
            },
            async {
                let res = io::copy(reader, &mut stdout).await;
                println!("Server disconnected!");
                res
            },
        )
        .await?;

        Ok(())
    })
}