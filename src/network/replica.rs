use async_channel::{unbounded, Receiver, Sender};
use petgraph::algo::kosaraju_scc;
use petgraph::graph::DiGraph;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use smol::io::{self, AsyncBufReadExt, AsyncWriteExt};
use smol::stream::StreamExt;
use smol::Async;
use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct CommittedDeps {
    #[serde_as(as = "Vec<(_, _)>")]
    pub committed: HashMap<Instance, bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientRequest {
    // last one is request ID
    Read(String, SocketAddr, u64),
    Write(String, String, SocketAddr, u64),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientReply {
    // Reply(result, ID)
    Reply(Option<String>, u64)
}

pub type Instance = (u8, u64); // ID of replica, instance number

pub type SeqNumber = u64;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum CommandState {
    PreAccepted,
    Accepted,
    Committed,
}

impl fmt::Display for CommandState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            CommandState::PreAccepted => write!(f, "PreAccept"),
            CommandState::Accepted => write!(f, "Accepted "),
            CommandState::Committed => write!(f, "Committed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReplicaState {
    instance_number: u64,
    cmds: HashMap<Instance, (ClientRequest, SeqNumber, HashSet<Instance>, CommandState)>, // (cmd, seq, deps, state)
    dict: HashMap<String, String>,
    preaccept_replies: HashMap<Instance, Vec<(SeqNumber, HashMap<Instance, bool>)>>,
    naccept: HashMap<Instance, u8>,
    dep_graph: DiGraph<Instance, ()>,
    executed: HashSet<Instance>,
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
    PreAccept(ClientRequest, u64, CommittedDeps, Instance, SocketAddr),
    PreAcceptOK(ClientRequest, u64, CommittedDeps, Instance, SocketAddr),
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
                dep_graph: DiGraph::new(),
                dict: HashMap::new(),
                preaccept_replies: HashMap::new(),
                naccept: HashMap::new(),
                executed: HashSet::new(),
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

        let max_num = match rs.cmds.keys().map(|x| -> u64 { x.1 }).max() {
            Some(x) => x,
            None => 0,
        };

        let mut log =
            vec![
                vec!["Empty[----- ...................... -----]".to_string(); max_num as usize];
                n.into()
            ];
        for ((id, num), (req, seq, _, status)) in rs.cmds.clone().into_iter() {
            match req {
                ClientRequest::Read(key, _, _) => {
                    log[(id - 1) as usize][(num - 1) as usize] = format!(
                        "{}{}{}{:<3}{}{}{}",
                        "Read-[".to_string(),
                        key_to_string(key),
                        ", Seq: ".to_string(),
                        seq,
                        " Status: ".to_string(),
                        status,
                        "]".to_string(),
                    );
                }
                ClientRequest::Write(key, _, _, _) => {
                    log[(id - 1) as usize][(num - 1) as usize] = format!(
                        "{}{}{}{:<3}{}{}{}",
                        "Write[".to_string(),
                        key_to_string(key),
                        ", Seq: ".to_string(),
                        seq,
                        " Status: ".to_string(),
                        status,
                        "]".to_string(),
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

        // ans.push_str("Dependency graph:\n");
        // ans.push_str(format!("{:?}\n", rs.dep_graph).as_str());
        // ans.push_str("\n");
        ans.push_str("Dict Status:\n");
        let mut sorted_dict: Vec<(String,String)> = rs.dict.clone().into_iter().collect();
        sorted_dict.sort();
        ans.push_str(format!("{:?}\n", sorted_dict).as_str());
        ans.push_str("\n");
        ans.push_str("Executed command instances:\n");
        let mut sorted_exe: Vec<Instance> = rs.executed.clone().into_iter().collect();
        sorted_exe.sort();
        ans.push_str(format!("{:?}\n", sorted_exe).as_str());

        drop(rs);
        return ans;
    }

    // returns true if requests interfere, otherwise false
    pub fn interfere(a: ClientRequest, b: ClientRequest) -> bool {
        match (a, b) {
            (ClientRequest::Write(k1, _, _, _), ClientRequest::Read(k2, _, _)) => {
                if k1 == k2 {
                    return true;
                }
                return false;
            }
            (ClientRequest::Read(k1, _, _), ClientRequest::Write(k2, _, _, _)) => {
                if k1 == k2 {
                    return true;
                }
                return false;
            }
            (ClientRequest::Write(k1, _, _, _), ClientRequest::Write(k2, _, _, _)) => {
                if k1 == k2 {
                    return true;
                }
                return false;
            }
            _ => false,
        }
    }

    pub fn add_dependency(dep_g: &mut DiGraph<Instance, ()>, src: Instance, dst: Instance) {
        // Check if the nodes exist, if not, add them and get their node indices
        let snode = match dep_g.node_indices().find(|&n| dep_g[n] == src) {
            Some(node) => node,
            None => dep_g.add_node(src),
        };
        let dnode = match dep_g.node_indices().find(|&n| dep_g[n] == dst) {
            Some(node) => node,
            None => dep_g.add_node(dst),
        };

        // Add the edge
        dep_g.update_edge(snode, dnode, ());
    }

    pub async fn reply_to_client(message: ClientReply, addr: SocketAddr) -> () {
        match Async::<TcpStream>::connect(addr).await {
            Ok(mut stream) => {
                // Intro messages.
                println!("Replying to client: {}", stream.get_ref().peer_addr().ok().unwrap());
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

    pub fn execute_command(replica_state: Arc<Mutex<ReplicaState>>) -> () {
        let mut rs = replica_state.lock().unwrap();

        let sccs = kosaraju_scc(&rs.dep_graph);
        for mut sc in sccs {
            // this is in reverse topological order
            // sort each connected comp by seq number
            sc.sort_by(|a, b| {
                let a_node = rs.dep_graph.node_weight(*a).unwrap();
                let b_node = rs.dep_graph.node_weight(*b).unwrap();
                let a_seq = rs.cmds.get(a_node).unwrap().1;
                let b_seq = rs.cmds.get(b_node).unwrap().1;
                a_seq.cmp(&b_seq)
            });
            // execute unexecuted commands in this order
            for node in sc {
                let ins = rs.dep_graph.node_weight(node).unwrap().clone();
                let req = match rs.cmds.get(&ins) {
                    Some((r, _, _, _)) => r,
                    None => return (),
                };
                // execute if not already executed
                if !rs.executed.contains(&ins) {
                    let (res, addr, id) = match req.clone() {
                        ClientRequest::Read(key, addr, id) => (rs.dict.get(&key).cloned(), addr, id),
                        ClientRequest::Write(key, val, addr, id) => (rs.dict.insert(key, val), addr, id),
                    };
                    let mes: ClientReply = ClientReply::Reply(res, id);
                    smol::spawn(Replica::reply_to_client(mes, addr)).detach();
                    // mark executed
                    rs.executed.insert(ins);
                }
            }
        }

        drop(rs);
    }

    pub fn update_state (
        rs: &mut std::sync::MutexGuard<ReplicaState>,
        req: ClientRequest,
        cseq: SeqNumber,
        cdeps: HashSet<Instance>,
        cins: Instance,
        cmd_state: CommandState,
    ) {
        rs.cmds.insert(
            cins,
            (req.clone(), cseq, cdeps.clone(), cmd_state),
        );

        // add dependencies to dep_graph
        match rs.dep_graph.node_indices().find(|&n| rs.dep_graph[n] == cins) {
            Some(_) => (),
            None => {
                rs.dep_graph.add_node(cins);
            }
        };
        for d in cdeps.clone() {
            Replica::add_dependency(&mut rs.dep_graph, cins, d);
        }
    }

    pub fn atomic_request_preaccept(
        replica_id: u8,
        replica_state: Arc<Mutex<ReplicaState>>,
        req: ClientRequest,
        cseq: SeqNumber,
        cdeps: HashMap<Instance, bool>,
        cins: Option<Instance>,
    ) -> (SeqNumber, HashMap<Instance, bool>, Instance) {
        let mut rs = replica_state.lock().unwrap();

        let mut seq = cseq;
        let mut deps = cdeps;

        // check if I have committed any existing dependencies
        for (k, v) in deps.clone().into_iter() {
            if !v {
                match rs.cmds.get(&k).cloned() {
                    Some(cmd) => {
                        if cmd.3 != CommandState::PreAccepted {
                            deps.insert(k, true);
                        }
                    }
                    None => ()
                }
            }
        }

        for (i, (ireq, sn, _, status)) in rs.cmds.clone().into_iter() {
            if Replica::interfere(req.clone(), ireq) {
                seq = max(seq, 1 + sn);
                if status == CommandState::PreAccepted {
                    deps.insert(i, false);
                } else {
                    deps.insert(i, true);
                }
            }
        }

        let deps_keys: HashSet<Instance> = deps.keys().cloned().collect();
        match cins {
            Some(cins) => {
                // replica_id is not command leader
                Replica::update_state(&mut rs, req, seq, deps_keys, cins, CommandState::PreAccepted);

                // if replica_id is not the command leader, then no need to track number of preaccepts

                drop(rs);

                println!("{}{:?}{:?}", seq, deps, cins);

                return (seq, deps, cins);
            }
            _ => {
                // replica_id is command leader
                rs.instance_number += 1;
                let ins = rs.instance_number;
                Replica::update_state(&mut rs, req, seq, deps_keys, (replica_id, ins), CommandState::PreAccepted);
                
                rs.preaccept_replies.insert((replica_id, ins), Vec::new());
                drop(rs);

                println!("{}{:?}{:?}", seq, deps, (replica_id, ins));

                return (seq, deps, (replica_id, ins));
            }
        }
    }

    pub fn path(
        n: u8,
        replica_state: Arc<Mutex<ReplicaState>>,
        req: ClientRequest,
        cseq: SeqNumber,
        cdeps: HashMap<Instance, bool>,
        cins: Instance,
    ) -> Option<(SeqNumber, HashSet<Instance>, bool)> {
        let mut rs = replica_state.lock().unwrap();
        let preaccept_replies = rs.preaccept_replies.get(&cins).cloned();
        match preaccept_replies {
            Some(mut preaccept_replies) => {
                // add preaccept reply to list of preaccept replies
                preaccept_replies.push((cseq, cdeps.clone()));
                rs.preaccept_replies.insert(cins, preaccept_replies.clone());

                if preaccept_replies.len() >= (n / 2).into() {
                    match rs.cmds.get(&cins).cloned() {
                        Some(cmd) => {
                            // if committed or executed, then this is a late PreAcceptOK
                            if cmd.3 == CommandState::PreAccepted {
                                if n == 3 {
                                    // always take fast path and commit
                                    let cdeps_keys: HashSet<Instance> = cdeps.keys().cloned().collect(); 
                                    Replica::update_state(&mut rs, req, cseq, cdeps_keys.clone(), cins, CommandState::Committed);
                                    drop(rs);

                                    println!("{}", Replica::format_log(n, replica_state.clone()));

                                    return Some((cseq, cdeps_keys, true));
                                } else {
                                    // n = 5
                                    // to take fast path when n = 5, every dependence must have some replica in the
                                    // quorum that has committed or executed it

                                    // take max of seq and union of deps
                                    let mut same = true;
                                    let mut union = preaccept_replies[0].clone();
                                    for reply in &preaccept_replies[1..] {
                                        // if seq numbers differ, then no fast path
                                        if reply.0 != union.0 {
                                            same = false;
                                            // at this point, not going to take fast path
                                            // when taking slow path, need to update seq to greatest seq seen
                                            if reply.0 > union.0 {
                                                union.0 = reply.0
                                            }
                                        }

                                        for (k, v) in reply.1.clone().into_iter() {
                                            match union.1.get(&k).cloned() {
                                                // key in union so dependence already in union
                                                Some(v_) => {
                                                    // replica has committed the dependence but the leader has not
                                                    // so update to reflect that someone has committed the dependence
                                                    if (v == true) && (v_ == false) {
                                                        union.1.insert(k, v);
                                                    }
                                                }
                                                // key not in union so some replica had a different dependence
                                                None => {
                                                    same = false;
                                                    union.1.insert(k, v);
                                                }
                                            }
                                        }
                                    }

                                    if same {
                                        // check whether leader has committed any dependencies that the other
                                        // quorum replicas have not
                                        for (k, v) in union.1.clone().into_iter() {
                                            if !v {
                                                match rs.cmds.get(&k).cloned() {
                                                    Some(cmd_) => {
                                                        if cmd_.3 == CommandState::PreAccepted {
                                                            // no one has committed this dependence
                                                            // so must take slow path

                                                            // [FIXME] update command state to accepted
                                                            let union_keys : HashSet<Instance> = union.1.keys().cloned().collect();
                                                            Replica::update_state(&mut rs, req, union.0, union_keys.clone(), cins, CommandState::Accepted);
                                                            drop(rs);
                                                            return Some((union.0, union_keys, false));
                                                        } else {
                                                            union.1.insert(k, true);                                                        }
                                                    }
                                                    None => {
                                                        // no one has committed this dependence
                                                        // so must take slow path
                                                        let union_keys : HashSet<Instance> = union.1.keys().cloned().collect();
                                                        Replica::update_state(&mut rs, req, union.0, union_keys.clone(), cins, CommandState::Accepted);
                                                        drop(rs);
                                                        return Some((
                                                            union.0,
                                                            union.1.keys().cloned().collect(),
                                                            false,
                                                        ));
                                                    }
                                                }
                                            }
                                        }

                                        // all seq and deps are the same, and every dep has some replica that has
                                        // committed it; take fast path
                                        let union_keys: HashSet<Instance> = union.1.keys().cloned().collect(); 
                                        Replica::update_state(&mut rs, req, union.0, union_keys.clone(), cins, CommandState::Committed);
                                        drop(rs);

                                        println!("{}", Replica::format_log(n, replica_state.clone()));
                                        return Some((union.0, union_keys, true));
                                    } else {
                                        drop(rs);
                                        return Some((
                                            union.0,
                                            union.1.keys().cloned().collect(),
                                            false,
                                        ));
                                    }
                                }
                            } else {
                                drop(rs);
                                return None;
                            }
                        }
                        None => {
                            // this part of the code should never be reached
                            drop(rs);
                            println!("PreAcceptOK received before any PreAccept sent");
                            return None;
                        }
                    }
                } else {
                    // insufficient number of PreAcceptOKs
                    drop(rs);
                    return None;
                }
            }
            None => {
                // this part of the code should never be reached
                println!("PreAcceptOK received before any PreAccept sent");
                drop(rs);
                return None;
            }
        }
    }

    pub fn atomic_update_state(
        replica_state: Arc<Mutex<ReplicaState>>,
        req: ClientRequest,
        cseq: u64,
        cdeps: HashSet<(u8, u64)>,
        cins: (u8, u64),
        cmd_state: CommandState
    ) -> () {
        let mut rs = replica_state.lock().unwrap();
        Replica::update_state(&mut rs, req, cseq, cdeps, cins, cmd_state);
        drop(rs);
    }

    pub fn atomic_accept(
        n: u8,
        replica_state: Arc<Mutex<ReplicaState>>, 
        req: ClientRequest,
        cseq: SeqNumber,
        cdeps: HashSet<Instance>,
        cins: Instance
    ) -> bool {
        let mut rs = replica_state.lock().unwrap();
        match rs.cmds.get(&cins).cloned() {
            Some(cmd) => {
                // if committed, then this is a late AcceptOK
                if cmd.3 == CommandState::Committed {
                    drop(rs);
                    return false;
                } else {
                    let naccept = rs.naccept.get(&cins).cloned();
                    match naccept {
                        Some(naccept) => {
                            rs.naccept.insert(cins, naccept + 1);
                            if naccept >= n / 2 {
                                // commit
                                Replica::update_state(&mut rs, req, cseq, cdeps, cins, CommandState::Committed);
                                drop(rs);

                                println!("{}", Replica::format_log(n, replica_state.clone()));
                                return true;
                            } else {
                                drop(rs);
                                return false;
                            }
                        }
                        None => {
                            // this part of the code should never be reached
                            println!("AcceptOK recieved before any Accept sent");
                            drop(rs);
                            return false;
                        }
                    }
                }
            }
            None => {
                // this part of the code should never be reached
                println!("AcceptOK recieved before any PreAccept sent");
                drop(rs);
                return false;
            }
        }
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
                        HashMap::new(),
                        None,
                    );

                    // Send PreAccept to all:
                    for (_, stream) in streams.iter_mut() {
                        let deps: CommittedDeps = CommittedDeps {
                            committed: deps.clone(),
                        };
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
                        cdeps.committed,
                        Some(cins),
                    );

                    let stream = streams.get_mut(&leader).unwrap();
                    let deps: CommittedDeps = CommittedDeps {
                        committed: deps.clone(),
                    };
                    let message =
                        Event::PreAcceptOK(req.clone(), seq, deps.clone(), cins, replica_addr);

                    let _ = stream
                        .write_all(serde_json::to_string(&message).ok().unwrap().as_bytes())
                        .await;
                    let _ = stream.write_all("\n".as_bytes()).await;

                    println!("Replied {:?} to {}", message, stream.get_ref().peer_addr()?);
                }
                Event::PreAcceptOK(req, cseq, cdeps, cins, _) => {
                    let path = Replica::path(n, replica_state.clone(), req.clone(), cseq, cdeps.committed, cins);
                    match path {
                        // either take fast or slow
                        Some((seq, deps, take_fast)) => {
                            if take_fast {
                                Replica::execute_command(replica_state.clone());

                                // notify other replicas about the commit
                                for (_, stream) in streams.iter_mut() {
                                    let message = Event::Commit(
                                        req.clone(),
                                        seq,
                                        deps.clone(),
                                        cins,
                                        replica_addr,
                                    );

                                    let _ = stream
                                        .write_all(
                                            serde_json::to_string(&message)
                                                .ok()
                                                .unwrap()
                                                .as_bytes(),
                                        )
                                        .await;
                                    let _ = stream.write_all("\n".as_bytes()).await;

                                    println!(
                                        "Sent {:?} to {}",
                                        message,
                                        stream.get_ref().peer_addr()?
                                    );
                                }
                            } else {
                                // notify other replicas about the accept
                                for (_, stream) in streams.iter_mut() {
                                    let message =
                                        Event::Accept(req.clone(), seq, deps.clone(), cins, replica_addr);

                                    let _ = stream
                                        .write_all(serde_json::to_string(&message).ok().unwrap().as_bytes())
                                        .await;
                                    let _ = stream.write_all("\n".as_bytes()).await;

                                    println!("Sent {:?} to {}", message, stream.get_ref().peer_addr()?);
                                }
                            }
                        }
                        // either not enough, late, or some error occurred (check log for error)
                        None => ()
                    }
                }
                Event::Commit(req, cseq, cdeps, cins, _) => {
                    Replica::atomic_update_state(replica_state.clone(), req, cseq, cdeps, cins, CommandState::Committed);
                    println!("{}", Replica::format_log(n, replica_state.clone()));
                    Replica::execute_command(replica_state.clone());
                }
                Event::Accept(req, cseq, cdeps, cins, leader) => {
                    Replica::atomic_update_state(replica_state.clone(), req.clone(), cseq, cdeps.clone(), cins, CommandState::Accepted);

                    // reply to leader
                    let stream = streams.get_mut(&leader).unwrap();
                    let message =
                        Event::AcceptOK(req, cseq, cdeps, cins, replica_addr);

                    let _ = stream
                        .write_all(serde_json::to_string(&message).ok().unwrap().as_bytes())
                        .await;
                    let _ = stream.write_all("\n".as_bytes()).await;

                    println!("Replied {:?} to {}", message, stream.get_ref().peer_addr()?);
                }
                Event::AcceptOK(req, cseq, cdeps, cins, _) => {
                    let commit = Replica::atomic_accept(n, replica_state.clone(), req.clone(), cseq, cdeps.clone(), cins);
                    if commit {
                        Replica::execute_command(replica_state.clone());

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
