FROM rust
RUN rustup override set nightly

RUN mkdir src && echo "fn main() {}" > src/lib.rs
COPY Cargo.toml .
RUN cargo build

COPY . .
RUN cargo build