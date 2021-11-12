# CLI Releasing

## Github Releases

- Install [github-release](https://github.com/github-release/github-release).
- [Generate a personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token)
  and set it to the environment variable `GITHUB_TOKEN`.

### Linux Binary
- Run [release_linux_cli.sh](/utils/dev/release_linux_cli.sh) to release a linux binary.

### Debian Package
- Install a series of build tools.
Debuild
Dh
Equivis-build
- Run [release_linux_cli_debian.sh](/utils/dev/build-lockbook-debian/release_linux_cli_debian.sh).