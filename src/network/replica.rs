use async_channel::{unbounded, Receiver, Sender};
use smol::io::{self, AsyncBufReadExt, AsyncWriteExt};
use smol::stream::StreamExt;
use smol::Async;
use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use tiny_http::{Response, Server};
use crate::network::message::CustomMessage;

pub enum RequestType {
    Read,
    Write,
}

pub struct Replica {
    id: u8,
    addr: SocketAddr,
    connections: Vec<SocketAddr>,
    log: Vec<(RequestType, String, String)>,
    state: HashMap<String, String>
}

pub enum Event {
    Message(SocketAddr, String),
    Ping(SocketAddr, String),
    Pong(SocketAddr, String),
    ReadRequest(SocketAddr, String),
    WriteRequest(SocketAddr, String),
    StructMessage(CustomMessage, SocketAddr, String),
    Forward(SocketAddr, String),
    Acknowledge(SocketAddr),
}

impl Replica {
    pub fn new(id: u8, addr: SocketAddr, connections: Vec<SocketAddr>) -> Self {
        return Replica {
            id,
            addr,
            connections,
            log: Vec::new(),
            state: HashMap::new()
        };
    }

    pub async fn dispatch(receiver: Receiver<Event>, streams: Vec<Async<TcpStream>>) -> io::Result<()> {
        while let Ok(event) = receiver.recv().await {
            // Process event and construct reply.
            match event {
                Event::Message(addr, msg) => {
                    println!("{} says: {}", addr, msg);

                    for mut stream in &streams {
                        let _ = stream.write_all("Forward ".as_bytes()).await;
                        let _ = stream.write_all(&msg.as_bytes()).await;
                        let _ = stream.write_all("\n".as_bytes()).await;
                        println!("Forwarded message to {}", stream.get_ref().peer_addr()?);
                    }

                },
                Event::Forward(addr, msg) => {
                    println!("{} says: {}", addr, msg);

                    for mut stream in &streams {
                        let _ = stream.write_all("Acknowle\n".as_bytes()).await;
                        println!("Acknowledged message from {}", stream.get_ref().peer_addr()?);
                    }

                },
                Event::StructMessage(j,_,_) => {
                    println!("json: {:?}", j)
                },
                Event::Ping(addr, _) => {
                    println!("Pong back to {}", addr)
                },
                _ => (),
            };
        }
        Ok(())
    }

    /// Reads requests from the other party and forwards them to the dispatcher task.
    async fn read_requests(sender: Sender<Event>, stream: Async<TcpStream>) -> io::Result<()> {
        let addr = stream.get_ref().peer_addr()?;
        let mut lines = io::BufReader::new(stream).lines();

        while let Some(line) = lines.next().await {
            match line {
                Ok(line) => {
                    println!("Message received: {}", line);
                    
                    let event = &line[..8];
                    match event {
                        "Client  " => sender.send(Event::Message(addr, line[8..].to_string())).await.ok(),
                        "Forward " => sender.send(Event::Forward(addr, line[8..].to_string())).await.ok(),
                        "Acknowle" => sender.send(Event::Acknowledge(addr)).await.ok(),
                        _ => Some(()),
                    };
                    
                    // let json : CustomMessage = serde_json::from_str(line.as_str())?;
                    // sender.send(Event::StructMessage(json, addr, "test".to_string())).await.ok();
                },
                Err(e) => {
                    println!("Error: {}", e);
                }
            }
            
        }
        Ok(())
    }

    pub fn start(&mut self) -> io::Result<()>{
        smol::block_on(async {

            // listen incoming connections
            let listener = Async::<TcpListener>::bind(self.addr)?;
            println!(
                "Listening to connections on {}",
                listener.get_ref().local_addr()?
            );

            let mut streams = Vec::new();
            // establish tcp connection with other replicas
            for addr in self.connections.iter() {
                let mut waiting: bool = false;

                // keep trying until connection succeeds
                loop {
                    match Async::<TcpStream>::connect(*addr).await {
                        Ok(stream) =>{

                            // Intro messages.
                            println!("Connected to {}", stream.get_ref().peer_addr()?);
                            println!("My nickname: {}", stream.get_ref().local_addr()?);

                            streams.push(stream);

                            break
                        }
                        Err(_) => {
                            if !waiting {
                                println!("Waiting to connect to {}", addr);
                                waiting = true;
                            }
                            
                        }
                    }
                }
            }

            let (sender, receiver) = unbounded();
            smol::spawn(Replica::dispatch(receiver, streams)).detach();

            loop {
                // Accept the next connection.
                let (stream, _) = listener.accept().await?;
                println!("{} can now send messages to {}", stream.get_ref().peer_addr()?, stream.get_ref().local_addr()?);
            
                let sender = sender.clone();

                // Spawn a background task reading messages from the other party.
                smol::spawn(async move {

                    // Read messages from the other party and ignore I/O errors when the other party quits.
                    Replica::read_requests(sender.clone(), stream).await.ok();

                })
                .detach();
            }
        })
    }
}


