# Linux Installing

## Building from source

[See build instructions.](../build/linux.md) 

## Package Managers

[Arch Linux](https://aur.archlinux.org/packages/lockbook-desktop): `yay -S lockbook-desktop`

[Debian Linux](https://github.com/lockbook/lockbook/releases)

[Snap Store](https://snapcraft.io/lockbook-desktop) `snap install lockbook-desktop`

### Verify snap package prior to installing it

Snap does not check if a package was built by the publisher. As a result, there needs to be an alternative method
to verify this. We have bundled this check into a single command.

- Run `curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/SmailBarkouch/snap-verify/main/snap-verify.sh | sh -s lockbook-desktop fqikNXeTHExTwMsksdo78qofrP5HwVISTlZz2dFDsNeNzY_Z6FEki9w9CWqXoDLU`