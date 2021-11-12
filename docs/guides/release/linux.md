# Linux Releasing

- Set up your [build environment](/docs/guides/building/linux.md:3).

## Github Releases

- Install [github-release](https://github.com/github-release/github-release).
- [Generate a personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token)
  and set it to the environment variable `GITHUB_TOKEN`.

### Debian Package
- Install a series of build tools.
Debuild
Deb Helper (`dh`)
Equivis (`equivis-build`)
- Run [release_linux_desktop_debian.sh](/utils/dev/build-lockbook-debian/release_linux_desktop_debian.sh).