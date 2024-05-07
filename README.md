# cos518-2024-project

### Instructions 
 1) run `cargo build` to build the project
 2) 
 ```
 Usage: project [OPTIONS]

Options:
  -l, --listener <LISTENER>
          Address on which to listen for incoming messages (format: e.g., 127.0.0.1:6000)
  -c, --connections <CONNECTIONS>...
          List of addresses to connect to (format: e.g., 127.0.0.1:6000)
  -i, --id <ID>
          Replica ID
  -n, --n <N>
          Total number of replicas [default: 3]
  -p, --public-ip <PUBLIC_IP>
          If provided, send other replicas this address to connect instead
  -d, --debug-client
          Client debug mode
  -g, --gen <GEN>
          Client request generator mode provide address
  -t, --time-sleep <TIME_SLEEP>
          Rate limit in seconds for request generating client [default: 200]
  -r, --rate <RATE>
          Conflict rate [0,1] [default: 0.02]
  -s, --save <SAVE>
          Save - instruct replica to save its state on local disk, all other flags are ignored
  -e, --experiment-time <EXPERIMENT_TIME>
          Generate requests as a client for this many seconds [default: 5]
  -h, --help
          Print help
  -V, --version
          Print version
 ```


### Example usage

1) run `cargo run -- -i 1 -n 3 --l 127.0.0.1:6000 -c 127.0.0.1:7000 127.0.0.1:8000` to start a replica that listens for incoming messages on 127.0.0.1:6000 and that repeatedly tries to establish a connection with 127.0.0.1:7000 and 127.0.0.1:8000 until it succceeds
2) run `cargo run -- -m -c 127.0.0.1:6000` to start a client that establishes a connection with 127.0.0.1:6000 in debug mode 

