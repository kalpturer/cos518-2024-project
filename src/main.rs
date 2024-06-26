pub mod network;
pub mod types;

use clap::Parser;
use network::client;
use network::replica::Replica;
use smol::io;
use std::net::SocketAddr;
use std::process::exit;
use std::str::FromStr;

///
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Address on which to listen for incoming messages (format: e.g., 127.0.0.1:6000)
    #[arg(short, long)]
    listener: Option<String>,

    /// List of addresses to connect to (format: e.g., 127.0.0.1:6000)
    #[arg(short, long, value_delimiter = ' ', num_args = 1..)]
    connections: Vec<String>,

    /// Replica ID
    #[arg(short, long)]
    id: Option<u8>,

    /// Total number of replicas
    #[arg(short, long, default_value_t = 3)]
    n: u8,

    /// If provided, send other replicas this address to connect instead
    #[arg(short, long)]
    public_ip: Option<String>,

    /// Client debug mode
    #[arg(short, long, default_value_t = false)]
    debug_client: bool,

    /// Client request generator mode provide address
    #[arg(short, long)]
    gen: Option<String>,

    /// Rate limit in seconds for request generating client
    #[arg(short, long, default_value_t = 200)] // find the val [FIXME]
    time_sleep: u64,

    /// Conflict rate [0,1]
    #[arg(short, long, default_value_t = 0.02)]
    rate: f64,

    /// Save - instruct replica to save its state on local disk, all other flags are ignored
    #[arg(short, long)]
    save: Option<String>,

    /// Generate requests as a client for this many seconds
    #[arg(short, long, default_value_t = 5)] // find the val [FIXME]
    experiment_time: u64,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match (cli.save, cli.gen) {
        (Some(addr), _) => {
            let socket = SocketAddr::from_str(&addr).unwrap();
            let _res = client::save_replica_state(socket);
        }
        (None, Some(addr)) => {
            let socket = SocketAddr::from_str(&addr).unwrap();
            let listen_socket = match cli.listener {
                Some(l) => SocketAddr::from_str(&l).unwrap(),
                None => {
                    println!("Listening address required for reply");
                    exit(0);
                }
            };
            let public = match cli.public_ip {
                Some(p) => SocketAddr::from_str(&p).unwrap(),
                None => listen_socket,
            };
            let _res = client::generator_client(
                socket,
                listen_socket,
                public,
                cli.rate,
                cli.time_sleep,
                cli.experiment_time,
            );
        }
        (None, None) => {
            if (cli.debug_client == false) && cli.connections.len() == usize::from(cli.n - 1) {
                match cli.listener {
                    Some(lst) => {
                        // debugging
                        println!("listener: {}", lst);

                        let mut connections = Vec::new();
                        for addr in cli.connections.iter() {
                            // debugging
                            println!("connection: {}", addr);

                            // convert String into SocketAddr into Address
                            let connection = SocketAddr::from_str(addr).unwrap();
                            connections.push(connection);
                        }

                        // convert String into SocketAddr into Address
                        match cli.id {
                            Some(id) => {
                                let public_addr = match cli.public_ip {
                                    Some(ar) => ar,
                                    None => lst.clone(),
                                };
                                let listener = SocketAddr::from_str(&public_addr).unwrap();
                                let local_addr = SocketAddr::from_str(&lst).unwrap();
                                let mut replica = Replica::new(id, listener, local_addr, connections, cli.n);
                                let _res = replica.start();
                            }
                            None => {
                                println!("Need to specify replica id")
                            }
                        }
                    }
                    None => {
                        println!("Replica needs address to listen on")
                    }
                }
            } else if cli.debug_client == false {
                println!("Number of connections does not match argument passed to --n (number of replicas)")
            } else if cli.connections.len() == 1 {
                let socket = SocketAddr::from_str(&cli.connections[0]).unwrap();
                let _res = client::debugging_client(socket);
            } else {
                println!("Client must connect to exactly one replica")
            }
        }
    }

    Ok(())
}
