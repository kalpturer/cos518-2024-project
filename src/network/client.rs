use crate::network::replica::ClientReply;
use crate::network::replica::{ClientRequest, Event::ReceivedRequest, Event::SaveState};
use rand::distributions::Alphanumeric;
use rand::Rng;
use smol::io::{AsyncBufReadExt, AsyncWriteExt};
use smol::stream::StreamExt;
use smol::{future, io, Async, Unblock};
use std::collections::HashMap;
use std::io::{stdout, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub fn send_client_request(addr: SocketAddr, id: u64) -> io::Result<()> {
    smol::block_on(async {
        // Connect to the server and create async stdin and stdout.
        let stream = Async::<TcpStream>::connect(addr).await?;

        // Intro messages.
        println!("Connected to {}", stream.get_ref().peer_addr()?);

        let mut writer = &stream;

        let mes = ClientRequest::Read("Hello".to_string(), addr, id);
        let res = writer
            .write_all(serde_json::to_string(&mes).ok().unwrap().as_bytes())
            .await;

        res
    })
}

pub fn save_replica_state(addr: SocketAddr) -> io::Result<()> {
    smol::block_on(async {
        let stream = Async::<TcpStream>::connect(addr).await?;
        let mut writer = &stream;
        let mes = SaveState;
        let _ = writer
            .write_all(serde_json::to_string(&mes).ok().unwrap().as_bytes())
            .await;
        let _ = writer.write_all("\n".as_bytes()).await;
        Ok(())
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
        let mut id: u64 = 0; 

        print!("Read or write? (r/w): ");
        let _ = stdout().flush();

        // read incoming lines until newlines
        let mut lines = io::BufReader::new(stdin).lines();

        let mut mode = "q".to_string();
        let mut key: String = "".to_string();

        while let Some(line) = lines.next().await {
            match line {
                Ok(line) => {
                    if mode == "q" {
                        if line == "r" || line == "w" {
                            mode = line;
                            print!("Key: ");
                            let _ = stdout().flush();
                        } else {
                            print!("Read or write? (r/w): ");
                            let _ = stdout().flush();
                        }
                    } else if mode == "r" {
                        mode = "q".to_string();
                        let mes = ReceivedRequest(ClientRequest::Read(line, addr, id));
                        let _ = writer
                            .write_all(serde_json::to_string(&mes).ok().unwrap().as_bytes())
                            .await;
                        let _ = writer.write_all("\n".as_bytes()).await;
                        print!("Read or write? (r/w): ");
                        let _ = stdout().flush();
                    } else if mode == "w" {
                        mode = "v".to_string();
                        key = line.clone();
                        print!("Value: ");
                        let _ = stdout().flush();
                    } else {
                        mode = "q".to_string();
                        let mes = ReceivedRequest(ClientRequest::Write(key.clone(), line, addr, id));
                        let _ = writer
                            .write_all(serde_json::to_string(&mes).ok().unwrap().as_bytes())
                            .await;
                        let _ = writer.write_all("\n".as_bytes()).await;
                        print!("Read or write? (r/w): ");
                        let _ = stdout().flush();
                    }
                }
                Err(e) => {
                    println!("Client input error: {}", e);
                }
            }
            id += 1;
        }
        Ok(())
    })
}

pub fn generator_client(
    addr: SocketAddr,
    listen_addr: SocketAddr,
    conflict: f64,
    timesleep: u64,
) -> io::Result<()> {

    async fn print_incoming(listener: Async<TcpListener>, ts: Arc<Mutex<HashMap<u64, Instant>>>) -> io::Result<()> {
        loop {
            // Accept the next connection.
            let (stream, _) = listener.accept().await?;
            let ref mut line = String::new();
            io::BufReader::new(stream).read_line(line).await?;

            println!("Reply received:");
            // parse
            let json: ClientReply = serde_json::from_str(line.as_str())?;

            let ts_access = ts.lock().unwrap();
            match json.clone() {
                ClientReply::Reply(x, id) => {
                    println!("{:?}, duration: {}ms", x, ts_access.get(&id).unwrap().elapsed().as_millis())
                }
            }
            drop(ts_access);
            println!("{:?}", json);
        }
    }

    smol::block_on(async {
        // Listen
        let listener = Async::<TcpListener>::bind(listen_addr)?;
        println!(
            "Listening to connections on {}",
            listener.get_ref().local_addr()?
        );
        let time_store: Arc<Mutex<HashMap<u64, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

        smol::spawn(print_incoming(listener, time_store.clone())).detach();

        // Connect to the server
        let stream = Async::<TcpStream>::connect(addr).await?;

        // Intro messages.
        println!("Connected to {}", stream.get_ref().peer_addr()?);

        let mut writer = &stream;

        let fixed = "hello".to_string();
        let write_percentage = 0.5;
        let mut id: u64 = 0;

        loop {
            let mut rng = rand::thread_rng();
            let mut key = fixed.clone();

            let conflict_coin = {
                let cf: f64 = rng.gen();
                cf < conflict
            };

            let write_coin = {
                let wf: f64 = rng.gen();
                wf < write_percentage
            };

            if !conflict_coin {
                // no conflict so sample new key
                key = rng
                    .sample_iter(&Alphanumeric)
                    .take(8)
                    .map(char::from)
                    .collect();
            }

            let mut ts_access = time_store.lock().unwrap();

            if write_coin {
                let mes = ReceivedRequest(ClientRequest::Write(key.clone(), key, listen_addr, id));
                let _ = writer
                    .write_all(serde_json::to_string(&mes).ok().unwrap().as_bytes())
                    .await;
                let _ = writer.write_all("\n".as_bytes()).await;
            } else {
                let mes = ReceivedRequest(ClientRequest::Read(key, listen_addr, id));
                let _ = writer
                    .write_all(serde_json::to_string(&mes).ok().unwrap().as_bytes())
                    .await;
                let _ = writer.write_all("\n".as_bytes()).await;
            }

            ts_access.insert(id, Instant::now());
            drop(ts_access);

            thread::sleep(Duration::from_millis(timesleep));
            id += 1;
        }
    })
}

pub fn run_client(addr: SocketAddr) -> io::Result<()> {
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
