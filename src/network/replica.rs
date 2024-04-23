use async_channel::{unbounded, Receiver, Sender};
use async_dup::Arc;
use smol::io::{self, AsyncBufReadExt, AsyncWriteExt};
use smol::stream::StreamExt;
use smol::Async;
use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use tiny_http::{Response, Server};
use crate::types::Address;
use crate::network::message::CustomMessage;

pub enum RequestType {
    Read,
    Write,
}

pub struct Replica {
    replica_id: u8,
    replica_addr: Address,
    api_addr: Address,
    connections: Vec<Async<TcpStream>>,
    log: Vec<(RequestType, String, String)>,
    state: HashMap<String, String>
}

pub enum Event {
    Join(SocketAddr, Arc<Async<TcpStream>>),
    Leave(SocketAddr),
    Message(SocketAddr, String),
    Ping(SocketAddr, String),
    Pong(SocketAddr, String),
    ReadRequest(SocketAddr, String),
    WriteRequest(SocketAddr, String),
    StructMessage(CustomMessage, SocketAddr, String),
}

impl Replica {
    pub fn new(id: u8, addr: Address, api: Address) -> Self {
        return Replica {
            replica_id: id,
            replica_addr: addr,
            api_addr: api,
            connections: Vec::new(),
            log: Vec::new(),
            state: HashMap::new()
        };
    }

    pub fn connect_to_replica(&mut self, connection: Async<TcpStream>) {
        self.connections.push(connection);
    }

    pub async fn dispatch_to_client(receiver: Receiver<Event>) -> io::Result<()> {
        while let Ok(event) = receiver.recv().await {
            // Process client event and construct reply.
            let output = match event {
                Event::Message(addr, msg) => format!("{} says: {}\n", addr, msg),
                Event::StructMessage(j,_,_) => format!("json: {:?}\n", j),
                Event::Ping(addr, _) => format!("Pong back to {}\n", addr),
                _ => "None".to_string(),
            };

            print!("{}", output);
        }
        Ok(())
    }

    /// Reads requests from the client and forwards them to the dispatcher task.
    async fn read_requests(sender: Sender<Event>, client: Arc<Async<TcpStream>>) -> io::Result<()> {
        let addr = client.get_ref().peer_addr()?;
        let mut lines = io::BufReader::new(client).lines();

        while let Some(line) = lines.next().await {
            let line = line?;
            let json : CustomMessage = serde_json::from_str(line.as_str())?;
            sender.send(Event::StructMessage(json, addr, "test".to_string())).await.ok();
        }
        Ok(())
    }

    pub fn start(&mut self) -> io::Result<()>{
        smol::block_on(async {

            // listen incoming client connections
            let client_listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 6000))?;
            println!(
                "Listening client connections on {}",
                client_listener.get_ref().local_addr()?
            );

            let (sender, receiver) = unbounded();
            smol::spawn(Replica::dispatch_to_client(receiver)).detach();

            loop {
                // Accept the next connection.
                let (stream, addr) = client_listener.accept().await?;
                let client = Arc::new(stream);
                let sender = sender.clone();

                // Spawn a background task reading messages from the client.
                smol::spawn(async move {
                    // Client starts with a `Join` event.
                    sender.send(Event::Join(addr, client.clone())).await.ok();

                    // Read messages from the client and ignore I/O errors when the client quits.
                    Replica::read_requests(sender.clone(), client).await.ok();

                    // Client ends with a `Leave` event.
                    sender.send(Event::Leave(addr)).await.ok();
                })
                .detach();
            }
        })
    }
}


