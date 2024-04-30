use std::net::{TcpStream, SocketAddr};
use smol::{future, io, Async, Unblock};
use smol::io::{AsyncWriteExt, AsyncBufReadExt};
use smol::stream::StreamExt;
use crate::network::replica::ClientRequest;

pub fn send_client_request(addr: SocketAddr) -> io::Result<()> {
    smol::block_on(async {
        // Connect to the server and create async stdin and stdout.
        let stream = Async::<TcpStream>::connect(addr).await?;

        // Intro messages.
        println!("Connected to {}", stream.get_ref().peer_addr()?);

        let mut writer = &stream;

        let mes = ClientRequest::Read("Hello".to_string(), addr);
        let res = writer
            .write_all(serde_json::to_string(&mes).ok().unwrap().as_bytes())
            .await;

        res
    })
}


pub fn debugging_client(addr: SocketAddr) -> io::Result<()> {
    smol::block_on(async {
        
        // Connect to the server and create async stdin and stdout.
        let stream = Async::<TcpStream>::connect(addr).await?;
        let stdin = Unblock::new(std::io::stdin());

        // Intro messages.
        println!("Connected to {}", stream.get_ref().peer_addr()?);
        println!("My nickname: {}", stream.get_ref().local_addr()?);

        let mut writer = &stream;

        println!("Read or write? (r/w): ");

        // read incoming lines until newlines
        let mut lines = io::BufReader::new(stdin).lines();

        let mut mode = "q".to_string();
        let mut key: String = "".to_string();
        
        while let Some(line) = lines.next().await {
            match line {
                Ok(line) => {
                    println!("Message received: {}", line);

                    if mode == "q" {
                        if line == "r" || line == "w"  {
                            mode = line;
                            println!("Key: ");
                        } else {
                            println!("Read or write? (r/w): ");
                        }
                    } else if mode == "r" {
                        mode = "q".to_string();
                        let mes = ClientRequest::Read(line, addr);
                        let _ = writer.write_all(serde_json::to_string(&mes).ok().unwrap().as_bytes()).await;
                        let _ = writer.write_all("\n".as_bytes()).await;
                        println!("Read or write? (r/w): ");
                    } else if mode == "w" {
                        mode = "v".to_string();
                        key = line.clone();
                        println!("Value: ");
                    } else {
                        mode = "q".to_string();
                        let mes = ClientRequest::Write(key.clone(), line, addr);
                        let _ = writer.write_all(serde_json::to_string(&mes).ok().unwrap().as_bytes()).await;
                        let _ = writer.write_all("\n".as_bytes()).await;
                        println!("Read or write? (r/w): ");
                    }
                }
                Err(e) => {
                    println!("Client input error: {}", e);
                }
            }
        }
        Ok(())
    })
}


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