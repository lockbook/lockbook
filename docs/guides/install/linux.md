# Linux Installing

## Building from source

[See build instructions.](../build/linux.md) 

## Package Managers

[Arch Linux](https://aur.archlinux.org/packages/lockbook-desktop): `yay -S lockbook-desktop`

[Debian Linux](https://github.com/lockbook/lockbook/releases)

[Snap Store](https://snapcraft.io/lockbook-desktop) **Not entirely secure, see below**

### Install from Snap Store Securely

The Snap Store does not automatically verify builds are of the publisher who made them. If you wish to
gain a level of security by verifying the snap build is compiled by the lockbook team, follow these steps.

- Run `snap download lockbook-desktop`. You should now have two files, `lockbook-desktop_<#>.snap` and
`lockbook-desktop_<#>.assert`. 
- Enter the `.assert` file, search for the field `snap-sha3-384` and copy its value. 
- Run `snap known --remote snap-build snap-sha3-384=<value> lockbook-desktop.snap-build`, replacing `<value>` with
what you copied last step. 
- Enter the `.sign-build` file, search for the field `sign-key-sha3-384` and copy its value. 
- Run `snap known --remote account-key public-key-sha3-384=<value> lockbook-desktop.account-key`, replacing `<value>` with
what you copied last step. 
- Run `sudo snap ack lockbook-desktop_<#>.assert`. 
- Run `sudo snap ack lockbook-desktop.account-key`. 
- Run `sudo snap ack lockbook-desktop.snap-build`. 
- Run  `snap install ./lockbook-desktop_<#>.snap`, and now you have installed lockbook desktop securely.