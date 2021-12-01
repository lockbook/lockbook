# CLI Installing

## Package Managers

[Arch Linux](https://aur.archlinux.org/packages/lockbook): `yay -S lockbook`

[MacOS](https://github.com/lockbook/homebrew-lockbook/blob/master/Formula/lockbook.rb): `brew tap lockbook/lockbook && brew install lockbook`

[Snap Store](https://snapcraft.io/lockbook) `snap install lockbook`

### Verify snap package prior to installing it

Snap does not have a package signature verification method, you could download the snap and verify it using the command 
below, but future auto updates from snap will not continue to check that the package was authored by lockbook. For 
this reason, if the integrity of your packages is important, you should probably not use snap until they have integrity 
verification infrastructure in place.

- Run `curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/SmailBarkouch/snap-verify/main/snap-verify.sh | sh -s lockbook fqikNXeTHExTwMsksdo78qofrP5HwVISTlZz2dFDsNeNzY_Z6FEki9w9CWqXoDLU`

## Quick Install

Our binary has no dependencies. Simply:
- `curl -O` the latest release for your operating system
- `tar -xzf` the download
- `sudo cp` the binary anywhere on your `$PATH`

Repeat the process to upgrade to newer versions.

[See our downloads.](https://github.com/lockbook/lockbook/releases)
