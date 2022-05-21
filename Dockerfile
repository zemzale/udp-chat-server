FROM rust:1.60

WORKDIR /usr/src/udp-server
COPY . ./

RUN cargo build --release

EXPOSE 7878/udp

CMD ["./target/release/udp-server"]
