# Linux Installing

## Building from source

[See build instructions.](../build/linux.md) 

## Package Managers

[Arch Linux](https://aur.archlinux.org/packages/lockbook-desktop): `yay -S lockbook-desktop`

[Debian Linux](https://github.com/lockbook/lockbook/releases)

[Snap Store](https://snapcraft.io/lockbook-desktop) `snap install lockbook-desktop`

### Verify snap package prior to installing it

Snap does not have a package signature verification method, you could download the snap and verify it using the command
below, but future auto updates from snap will not continue to check that the package was authored by lockbook. For
this reason, if the integrity of your packages is important, you should probably not use snap until they have integrity
verification infrastructure in place.

- Run `curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/SmailBarkouch/snap-verify/main/snap-verify.sh | sh -s lockbook-desktop fqikNXeTHExTwMsksdo78qofrP5HwVISTlZz2dFDsNeNzY_Z6FEki9w9CWqXoDLU`