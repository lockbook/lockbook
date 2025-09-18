# Extending Lockbook

Lockbook is built to be extensible at every level. 

## CLI
Lockbook's CLI is built around a sophisticated tab completion behavior. Install the cli using your favorite [package manager](installing.md) or reach out to us if yours isn't listed there. You can configure our CLI to open your favorite text editor allowing you to rapidly jump to your desired note and edit it quickly.

`lockbook stream` subcommand can receive and send bytes to and from other terminal programs. See what you can do with [dmenu](https://www.youtube.com/watch?v=4JWeU78A95c) for inspiration.

`lockbook export` can also be used to snapshot directories to disk, [this website](https://github.com/lockbook/lockbook/blob/master/docs/update.sh) is powered by such a script. Here is [another example](https://github.com/Parth/parth.cafe/tree/master/.github/workflows).

# lb-rs
All of our core functionality is packaged in a rust crate: [lb-rs](https://crates.io/crates/lb-rs). See api docs [here](https://docs.rs/lb-rs/). Use this library to perform any operation you can do with a lockbook client. View, edit and organize your content. 

# lb-sdk
- lb-rs has C bindings which can be found [here](https://github.com/lockbook/lockbook/tree/master/libs/lb/lb-c)
  - [Swift Bindings are built from the C bindings](https://github.com/lockbook/lockbook/blob/master/libs/content/workspace-ffi/SwiftWorkspace/Sources/Workspace/Lb/Lb.swift)
  - [WIP Go Bindings from the C bindings](https://github.com/steverusso/lockbook-x/tree/master/go-lockbook)
- lb-rs also has JVM bindings which can be found [here](https://github.com/lockbook/lockbook/tree/master/libs/lb/lb-java)

Many languages will make it easy to consume C bindings. If you'd like specific support for your programming language reach out to us.

# lb-fs
lb-fs is an experimental virtual file system implementation backed by lockbook. Presently the implementation uses nfs, though platform specific implementations may be explored later to deliver a better user experience. We also intend to include this functionality directly in our desktop apps when it's more stable.

Currently `lb-fs` can be invoked from within our CLI on Linux & macOS, see `lockbook fs` for more information. Using `lockbook fs` like this can enable you to experiment directly with file types that don't have dedicated experiences associated with them (think CAD, music scores, or other obscure third party formats and workflows).

lb-fs may also be the best way to use your own text editor as markdown link resolution works as expected there.