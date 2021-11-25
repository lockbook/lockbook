# CLI Releasing

## Github Releases

- Install [github-release](https://github.com/github-release/github-release).
- [Generate a personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token)
  and set it to the environment variable `GITHUB_TOKEN`.
- [`cd utils/dev`](/utils/dev/)

### Linux Binary
- Run [`./release_linux_cli.sh`](/utils/dev/release_linux_cli.sh) to release a linux binary.

### Debian Package
- Install the following build tools: `debuild`, `dh`, and `equivs-build`
- Run [`./release_linux_cli_debian.sh`](/utils/dev/build-lockbook-debian/release_linux_cli_debian.sh).

### macOS (and brew)

- From a `macOS` computer with `lipo` run [`./release_macos_cli_bump_brew.sh`](/utils/dev/build-lockbook-debian/release_linux_cli_debian.sh).

## Snap Store

You must be on an Ubuntu distribution to release Snap packages, whether it be natively or through a
Virtual Machine. On an Ubuntu distribution, snap is already going to be installed, unless one is on an old release.

- Run `sudo snap install snapcraft --classic`.
- Enter the [snap package folder](/utils/dev/snap-packages/lockbook) and run `snapcraft` to build the package.
- Run `snapcraft upload --release=stable <.snap file>` to release the snap package to the store.

You also want to upload a `.sign-build` file to the Snap Sotre, which helps validates the package has been built 
the lockbook team.

- Run `snapcraft create-key` to create a key to sign the snap package.
- Run `snapcraft register-key` to register that key.
- Run `snapcraft sign-build <.snap file>` to generate a `.sign-build` file and upload the snap store.