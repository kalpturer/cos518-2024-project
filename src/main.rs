pub mod network;
pub mod types;

use std::env;
use network::client;
use network::replica;
use network::replica::Replica;
use smol::io;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args[1] == "replica" {
        let mut replica = Replica::new(1, ([127, 0, 0, 1], 6000), ([127, 0, 0, 1], 6001));
        let _res = replica.start();
    } else if args[1] == "client" {
        let _res = client::run_client(([127, 0, 0, 1], 6000));
    } else {
        println!("not recognized: {}", args[1])
    }
    

    Ok(())
}
