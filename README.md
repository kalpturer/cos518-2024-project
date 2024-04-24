# cos518-2024-project

1) run `cargo run -- -l 127.0.0.1:6000 -c 127.0.0.1:7000 127.0.0.1:8000` to start a replica that listens for incoming messages on 127.0.0.1:6000 and that repeatedly tries to establish a connection with 127.0.0.1:7000 and 127.0.0.1:8000 until it succceeds
2) run `cargo run -- -c 127.0.0.1:6000` to start a client that establishes a connection with 127.0.0.1:6000

