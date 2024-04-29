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
    #[arg(short, long, default_value_t = 1)]
    id: u8,

    /// Total number of replicas
    #[arg(short, long, default_value_t = 3)]
    n: u8,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

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
            let mut replica = Replica::new(cli.id, listener, connections, cli.n);
            let _res = replica.start();
        }
        None => {
            if cli.connections.len() == 0 {
                println!("Client needs address to connect to")
            } else {
                let socket = SocketAddr::from_str(&cli.connections[0]).unwrap();
                let _res = client::run_client(socket);
            }
        }
    }

    Ok(())
}
