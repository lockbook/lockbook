# Linux

## From Source

Get the rust toolchain (rustup) and ensure `cargo` is in your path.

```
cd clients/linux
cargo build --release
```

In the `target/release` folder you'll find the `lockbook` binary. Place it
anywhere in your `$PATH`. To upgrade, `git pull origin master` and repeat the
process.
