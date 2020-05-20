FROM rust
RUN rustup override set nightly

RUN mkdir src && echo "fn main() {}" > src/lib.rs
COPY Cargo.toml .
COPY Cargo.lock .
RUN cargo build

COPY . .
RUN cat src/lib.rs > src/lib.rs
RUN cargo build
