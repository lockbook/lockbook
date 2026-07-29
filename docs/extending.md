# Extending Lockbook

Lockbook is built to be extensible at every level. 

## CLI

See [cli](cli.md) for information on:
* invoking your own text editor
* building on top of standard streams
* building on top of filesystem replication
* building on top of a virtual filesystem

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