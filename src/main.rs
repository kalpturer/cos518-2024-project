pub mod network;
pub mod types;

use clap::Parser;
use network::replica::Replica;
use network::client;
use smol::io;
use std::net::SocketAddr;
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

    /// Client mode
    #[arg(short, long, default_value_t = false)]
    mode: bool,
    
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    if (cli.mode == false) && cli.connections.len() == usize::from(cli.n - 1)  {
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
                let listener = SocketAddr::from_str(&lst).unwrap();
                match cli.id {
                    Some(id) => {
                        let mut replica = Replica::new(id, listener, connections, cli.n);
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
    } else if cli.mode == false {
        println!("Number of connections does not match argument passed to --n (number of replicas)")        
    } else if cli.connections.len() == 1 {
        let socket = SocketAddr::from_str(&cli.connections[0]).unwrap();
        let _res = client::debugging_client(socket);
    } else {
        println!("Client must connect to exactly one replica")
    }

    Ok(())
}
