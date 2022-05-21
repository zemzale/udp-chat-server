# Simple UDP server

This is a simple UDP server that has a messaging protocol, and allows clients to connect and talk to each other.

## How to run?

### Docker

````sh
docker pull zemzale/udp-server:latest
docker run -p 7878:7878/udp --rm --detach zemzale/udp-server:latest
```

### Rust 
```sh
cargo run
```
