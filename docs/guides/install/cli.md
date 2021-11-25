# CLI Installing

## Package Managers

[Arch Linux](https://aur.archlinux.org/packages/lockbook): `yay -S lockbook`

[MacOS](https://github.com/lockbook/homebrew-lockbook/blob/master/Formula/lockbook.rb): `brew tap lockbook/lockbook && brew install lockbook`

[Snap Store](https://snapcraft.io/lockbook) **Not entirely secure, see below**

### Install from Snap Store Securely

The Snap Store does not automatically verify builds are of the publisher who made them. If you wish to
gain a level of security by verifying the snap build is compiled by the lockbook team, follow these steps.

- Run `snap download lockbook`. You should now have two files, `lockbook_<#>.snap` and `lockbook_<#>.assert`. 
- Enter the `.assert` file, search for the field `snap-sha3-384` and copy its value. 
- Run `snap known --remote snap-build snap-sha3-384=<value> lockbook.snap-build`, replacing `<value>` with 
what you copied last step. 
- Enter the `.sign-build` file, search for the field `sign-key-sha3-384` and copy its value. 
- Run `snap known --remote account-key public-key-sha3-384=<value> lockbook.account-key`, replacing `<value>` with 
what you copied last step. 
- Run `sudo snap ack lockbook_<#>.assert`. 
- Run `sudo snap ack lockbook.account-key`. 
- Run `sudo snap ack lockbook.snap-build`. 
- Run  `snap install ./lockbook_<#>.snap`, and now you have installed lockbook securely.

## Building from source

[See build instructions.](../build/cli.md)

## Quick Install

Our binary has no dependencies. Simply:
- `curl -O` the latest release for your operating system
- `tar -xzf` the download
- `sudo cp` the binary anywhere on your `$PATH`

Repeat the process to upgrade to newer versions.

[See our downloads.](https://github.com/lockbook/lockbook/releases)
