# Linux

## Install

[Arch Linux](https://aur.archlinux.org/packages/lockbook-desktop): `yay -S lockbook-desktop`

[Debian Linux](https://github.com/lockbook/lockbook/releases)

## From Source

If you are on Arch Linux or any of its derivatives, such as Manjaro, you can simply install the aur package mentioned 
above. It builds from source.

Before you can build the binary, you must ensure that you have the necessary build tools and dependencies.

```
apt install devscripts build-essential lintian debhelper equivs
mk-build-deps --install utils/dev/build-lockbook-debian/ppa-lockbook-desktop/debian/control -r
```

Get the Rust toolchain (`rustup`) and ensure `cargo` is in your path.

```
cd clients/linux
cargo build --release
```

In the `target/release` folder, you'll find the `lockbook` binary. Place it
anywhere in your `$PATH`. To upgrade, `git pull origin master` and repeat the
process.

If you want lockbook to be integrated into your desktop environment, you will have
to configure `lockbook-desktop.desktop` located in `utils/dev/build-lockbook-debian/ppa-lockbook-desktop`.
Depending on where you placed your binary, you will need to edit the `Exec` field to reflect that path.
By default, it is `/usr/bin/lockbook-desktop`

You will also notice there is a field called `Icon`. This field will determine what icon
is shown in your desktop environment.

Then, `lockbook-desktop.desktop` is `/usr/share/applications/`.

In addition to this,


