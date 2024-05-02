use async_channel::{unbounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use smol::io::{self, AsyncBufReadExt, AsyncWriteExt};
use smol::stream::StreamExt;
use smol::Async;
use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientRequest {
    Read(String, SocketAddr),
    Write(String, String, SocketAddr),
}

pub type Instance = (u8, u64); // ID of replica, instance number

pub type SeqNumber = u64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CommandState {
    PreAccepted,
    Accepted,
    Committed,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReplicaState {
    instance_number: u64,
    cmds: HashMap<Instance, (ClientRequest, SeqNumber, HashSet<Instance>, CommandState)>, // (cmd, seq, deps, state)
    dict: HashMap<String, String>,
    counting_preaccept: HashMap<Instance, i8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Event {
    Message(SocketAddr, String),
    Ping(SocketAddr, String),
    Pong(SocketAddr, String),
    Forward(SocketAddr, String),
    Acknowledge(SocketAddr),
    SaveState,
    // EPaxos messages: --------------------------------------------------------
    ReceivedRequest(ClientRequest),
    // message(gamma, seq, deps, instance, sender)
    PreAccept(ClientRequest, u64, HashSet<Instance>, Instance, SocketAddr),
    PreAcceptOK(ClientRequest, u64, HashSet<Instance>, Instance, SocketAddr),
    Accept(ClientRequest, u64, HashSet<Instance>, Instance, SocketAddr),
    AcceptOK(ClientRequest, u64, HashSet<Instance>, Instance, SocketAddr),
    Commit(ClientRequest, u64, HashSet<Instance>, Instance, SocketAddr),
}

pub struct Replica {
    replica_state: Arc<Mutex<ReplicaState>>,
    id: u8,
    addr: SocketAddr,
    connections: Vec<SocketAddr>,
    n: u8,
}

impl Replica {
    pub fn new(id: u8, addr: SocketAddr, connections: Vec<SocketAddr>, n: u8) -> Self {
        return Replica {
            id,
            addr,
            connections,
            replica_state: Arc::new(Mutex::new(ReplicaState {
                instance_number: 0,
                cmds: HashMap::new(),
                dict: HashMap::new(),
                counting_preaccept: HashMap::new(),
            })),
            n,
        };
    }

    pub fn format_log(n: u8, replica_state: Arc<Mutex<ReplicaState>>) -> String {
        let rs = replica_state.lock().unwrap();

        fn key_to_string(key: String) -> String {
            let ans: String = if key.len() < 4 {
                let mut formatted = key.clone();
                formatted.push_str("_".repeat(4 - key.len()).as_str());
                formatted
            } else {
                key.chars().take(4).collect()
            };
            return ans;
        }

        let max_num = rs.cmds.keys().map(|x| -> u64 { x.1 }).max().unwrap();

        let mut log = vec![vec!["Empty[----]".to_string(); max_num as usize]; n.into()];
        for ((id, num), (req, _, _, _)) in rs.cmds.clone().into_iter() {
            match req {
                ClientRequest::Read(key, _) => {
                    log[(id - 1) as usize][(num - 1) as usize] = format!(
                        "{}{}{}",
                        "Read-[".to_string(),
                        key_to_string(key),
                        "]".to_string()
                    );
                }
                ClientRequest::Write(key, _val, _) => {
                    log[(id - 1) as usize][(num - 1) as usize] = format!(
                        "{}{}{}",
                        "Write[".to_string(),
                        key_to_string(key),
                        "]".to_string()
                    );
                }
            }
        }

        let mut ans = "".to_string();
        ans.push_str("BEGIN CMD LOG: --------------------------------------------\n");
        let mut i = 1;
        for row in &log {
            ans.push_str(format!("ID: {} -> ", i).as_str());
            i += 1;
            for cmd in row {
                ans.push_str(format!("{:<12} ", cmd).as_str());
            }
            ans.push_str("\n");
        }
        ans.push_str("END CMD LOG: ----------------------------------------------\n");

        drop(rs);
        return ans;
    }

    // returns true if requests interfere, otherwise false
    pub fn interfere(a: ClientRequest, b: ClientRequest) -> bool {
        match (a, b) {
            (ClientRequest::Write(k1, _, _), ClientRequest::Read(k2, _)) => {
                if k1 == k2 {
                    return true;
                }
                return false;
            }
            (ClientRequest::Read(k1, _), ClientRequest::Write(k2, _, _)) => {
                if k1 == k2 {
                    return true;
                }
                return false;
            }
            (ClientRequest::Write(k1, _, _), ClientRequest::Write(k2, _, _)) => {
                if k1 == k2 {
                    return true;
                }
                return false;
            }
            _ => false,
        }
    }

    pub async fn reply_to_client(message: Option<String>, addr: SocketAddr) -> io::Result<()> {
        loop {
            match Async::<TcpStream>::connect(addr).await {
                Ok(mut stream) => {
                    // Intro messages.
                    println!("Replying to client: {}", stream.get_ref().peer_addr()?);
                    let _ = stream
                        .write_all(serde_json::to_string(&message).ok().unwrap().as_bytes())
                        .await;
                    let _ = stream.write_all("\n".as_bytes()).await;
                }
                Err(_) => {
                    println!("Connection to client address {} failed", addr);
                }
            }
        }
    }

    pub fn atomic_request_preaccept(
        replica_id: u8,
        replica_state: Arc<Mutex<ReplicaState>>,
        req: ClientRequest,
        cseq: u64,
        cdeps: HashSet<(u8, u64)>,
        cins: Option<(u8, u64)>,
    ) -> (u64, HashSet<(u8, u64)>, (u8, u64)) {
        let mut rs = replica_state.lock().unwrap();

        let mut seq = cseq;
        let mut deps = cdeps;
        for (i, (ireq, sn, _, _)) in rs.cmds.clone().into_iter() {
            if Replica::interfere(req.clone(), ireq) {
                seq = max(seq, 1 + sn);
                deps.insert(i);
            }
        }

        match cins {
            Some(cins) => {
                // replica_id is not command leader
                rs.cmds.insert(
                    cins,
                    (req.clone(), seq, deps.clone(), CommandState::PreAccepted),
                );

                // if replica_id is not the command leader, then no need to track number of preaccepts

                drop(rs);

                return (seq, deps, cins);
            }
            _ => {
                // replica_id is command leader
                rs.instance_number += 1;
                let ins = rs.instance_number;
                rs.cmds.insert(
                    (replica_id, ins),
                    (req.clone(), seq, deps.clone(), CommandState::PreAccepted),
                );

                rs.counting_preaccept.insert((replica_id, ins), 0);

                drop(rs);

                return (seq, deps, (replica_id, ins));
            }
        }
    }

    pub fn atomic_preaccept_ok(replica_state: Arc<Mutex<ReplicaState>>, cins: (u8, u64)) -> i8 {
        let mut rs = replica_state.lock().unwrap();
        let val = rs.counting_preaccept.get(&cins).cloned();
        match val {
            Some(v) => {
                rs.counting_preaccept.insert(cins, v + 1);
                drop(rs);
                return v + 1;
            }
            None => {
                // this part of the code should never be reached
                drop(rs);
                return -1;
            }
        }
    }

    pub fn atomic_commit(
        n: u8,
        replica_state: Arc<Mutex<ReplicaState>>,
        req: ClientRequest,
        cseq: u64,
        cdeps: HashSet<(u8, u64)>,
        cins: (u8, u64),
    ) -> () {
        let mut rs = replica_state.lock().unwrap();
        rs.cmds.insert(
            cins,
            (req.clone(), cseq, cdeps.clone(), CommandState::Committed),
        );
        drop(rs);
        println!("{}", Replica::format_log(n, replica_state));
    }

    pub async fn dispatch(
        replica_id: u8,
        replica_addr: SocketAddr,
        n: u8,
        replica_state: Arc<Mutex<ReplicaState>>,
        receiver: Receiver<Event>,
        mut streams: HashMap<SocketAddr, Async<TcpStream>>,
    ) -> io::Result<()> {
        while let Ok(event) = receiver.recv().await {
            // Process event and construct reply.
            match event {
                // Testing network setup
                Event::Message(addr, msg) => {
                    println!("{} says: {}", addr, msg);

                    for (_, stream) in streams.iter_mut() {
                        let _ = stream.write_all("Forward ".as_bytes()).await;
                        let _ = stream.write_all(&msg.as_bytes()).await;
                        let _ = stream.write_all("\n".as_bytes()).await;
                        println!("Forwarded message to {}", stream.get_ref().peer_addr()?);
                    }
                }
                Event::Forward(addr, msg) => {
                    println!("{} forwarded: {}", addr, msg);

                    for (_, stream) in streams.iter_mut() {
                        let _ = stream.write_all("Acknowle\n".as_bytes()).await;
                        println!("Acknowledged message from {}", addr);
                    }
                }
                Event::Ping(addr, _) => {
                    println!("Pong back to {}", addr)
                }
                Event::SaveState => {
                    let formatted_state = Replica::format_log(n, replica_state.clone());
                    let mut file = File::create(format!("id_{}.txt", replica_id))?;
                    file.write_all(formatted_state.as_bytes())?;
                }

                // EPaxos client request handling
                Event::ReceivedRequest(req) => {
                    let (seq, deps, ins) = Replica::atomic_request_preaccept(
                        replica_id,
                        replica_state.clone(),
                        req.clone(),
                        1,
                        HashSet::new(),
                        None,
                    );

                    // Send PreAccept to all:
                    for (_, stream) in streams.iter_mut() {
                        let message =
                            Event::PreAccept(req.clone(), seq, deps.clone(), ins, replica_addr);

                        let _ = stream
                            .write_all(serde_json::to_string(&message).ok().unwrap().as_bytes())
                            .await;
                        let _ = stream.write_all("\n".as_bytes()).await;

                        println!("Sent {:?} to {}", message, stream.get_ref().peer_addr()?);
                    }
                }
                Event::PreAccept(req, cseq, cdeps, cins, leader) => {
                    let (seq, deps, _) = Replica::atomic_request_preaccept(
                        replica_id,
                        replica_state.clone(),
                        req.clone(),
                        cseq,
                        cdeps,
                        Some(cins),
                    );

                    let stream = streams.get_mut(&leader).unwrap();
                    let message =
                        Event::PreAcceptOK(req.clone(), seq, deps.clone(), cins, replica_addr);

                    let _ = stream
                        .write_all(serde_json::to_string(&message).ok().unwrap().as_bytes())
                        .await;
                    let _ = stream.write_all("\n".as_bytes()).await;

                    println!("Replied {:?} to {}", message, stream.get_ref().peer_addr()?);
                }
                Event::PreAcceptOK(req, cseq, cdeps, cins, _) => {
                    let n_preaccept_ok = Replica::atomic_preaccept_ok(replica_state.clone(), cins);

                    if n_preaccept_ok == -1 {
                        println!("PreAcceptOK received before any PreAccept sent");
                    } else if n_preaccept_ok >= (n / 2).try_into().unwrap() {
                        Replica::atomic_commit(
                            n,
                            replica_state.clone(),
                            req.clone(),
                            cseq,
                            cdeps.clone(),
                            cins,
                        );

                        // notify other replicas about the commit
                        for (_, stream) in streams.iter_mut() {
                            let message =
                                Event::Commit(req.clone(), cseq, cdeps.clone(), cins, replica_addr);

                            let _ = stream
                                .write_all(serde_json::to_string(&message).ok().unwrap().as_bytes())
                                .await;
                            let _ = stream.write_all("\n".as_bytes()).await;

                            println!("Sent {:?} to {}", message, stream.get_ref().peer_addr()?);
                        }
                    }
                }
                Event::Commit(req, cseq, cdeps, cins, _) => {
                    Replica::atomic_commit(n, replica_state.clone(), req, cseq, cdeps, cins);
                }
                _ => (),
            };
        }
        Ok(())
    }

    /// Reads requests from the other party and forwards them to the dispatcher task.
    async fn read_requests(sender: Sender<Event>, stream: Async<TcpStream>) -> io::Result<()> {
        // read incoming lines until newlines
        let mut lines = io::BufReader::new(stream).lines();

        while let Some(line) = lines.next().await {
            match line {
                Ok(line) => {
                    println!("Message received: {}", line);

                    // parse and forward to dispatch
                    let json: Event = serde_json::from_str(line.as_str())?;
                    sender.send(json).await.ok();
                }
                Err(e) => {
                    println!("Read_request Error: {}", e);
                }
            }
        }
        Ok(())
    }

    pub fn start(&mut self) -> io::Result<()> {
        smol::block_on(async {
            // listen incoming connections
            let listener = Async::<TcpListener>::bind(self.addr)?;
            println!(
                "Listening to connections on {}",
                listener.get_ref().local_addr()?
            );

            let mut streams: HashMap<SocketAddr, Async<TcpStream>> = HashMap::new();
            // establish tcp connection with other replicas
            for addr in self.connections.iter() {
                let mut waiting: bool = false;

                // keep trying until connection succeeds
                loop {
                    match Async::<TcpStream>::connect(*addr).await {
                        Ok(stream) => {
                            // Intro messages.
                            println!("Connected to {}", stream.get_ref().peer_addr()?);
                            println!("My nickname: {}", stream.get_ref().local_addr()?);

                            streams.insert(*addr, stream);

                            break;
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
            smol::spawn(Replica::dispatch(
                self.id,
                self.addr,
                self.n,
                self.replica_state.clone(),
                receiver,
                streams,
            ))
            .detach();

            loop {
                // Accept the next connection.
                let (stream, _) = listener.accept().await?;
                println!(
                    "{} can now send messages to {}",
                    stream.get_ref().peer_addr()?,
                    stream.get_ref().local_addr()?
                );

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
