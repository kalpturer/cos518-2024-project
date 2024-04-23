pub mod network;
pub mod types;

use std::env;
use std::str::FromStr;
use network::client;
use network::replica;
use network::replica::Replica;
use smol::io;
use clap::Parser;
use std::net::SocketAddrV4;


/// 
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// List of replica and API address pairs (format: e.g., 127.0.0.1:6000/127.0.0.1:6001)
    #[arg(short, long, value_delimiter = ' ', num_args = 1..)]
    replicas: Vec<String>,

    /// List of client addresses (format: e.g., 127.0.0.1:6000)
    #[arg(short, long, value_delimiter = ' ', num_args = 1..)]
    clients: Vec<String>,
}


fn main() -> io::Result<()> {
    let cli = Cli::parse();

    for addrs in cli.replicas.iter() {
        // debugging
        println!("replica input: {}", addrs);

        let addrs = addrs.split("/").collect::<Vec<&str>>();

        // debugging
        println!("replica address: {}", addrs[0]);
        println!("api address: {}", addrs[1]);
        
        let replica_socket = SocketAddrV4::from_str(addrs[0]).unwrap();
        let api_socket = SocketAddrV4::from_str(addrs[1]).unwrap();
        let mut replica = Replica::new(1, (replica_socket.ip().octets(), replica_socket.port()), (api_socket.ip().octets(), api_socket.port()));
        let _res = replica.start();
    }

    for addr in cli.clients.iter() {
        // debugging
        println!("client address: {}", addr);
        
        let socket = SocketAddrV4::from_str(addr).unwrap();
        let _res = client::run_client((socket.ip().octets(), socket.port()));
    }

    Ok(())
}
