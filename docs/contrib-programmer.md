# Contributing To Lockbook as a Programmer!
We're excited you want to contribute. Before you dive into code you should understand our [values](values.md) and you should be [be a user](contrib-user.md). You should also be in our [discord](https://discord.gg/lockbook). This page aims to be a live document you can use as a source of inspiration, in addition to browsing all our [github issues](https://github.com/lockbook/lockbook/issues), especially the ones tagged [good first issues](https://github.com/lockbook/lockbook/issues?q=is%3Aissue%20state%3Aopen%20label%3A%22good%20first%20issue%22). 

Lockbook is a project with massive surface area with the need for all types of contributors. Most of the codebase is in Rust, but there is critical code in Swift, Kotlin and other languages as well. Our workload includes: 
* UX thinking
* UI development (Native & Cross Platform Rust)
* Data Structures and Algo work (we model a lot of trees)
* Scaling our libraries and backend
* Devops and Internal tooling

# Rust Contributions
`lb-rs` is the heart of our Rust operation. It's a library that's responsible for our cryptography, offline operations, networking, storage, and more. All lockbook clients, regardless of what language they're written in use this library (with the power of foreign function interfaces). See the [lb-rs label](https://github.com/lockbook/lockbook/issues?q=is%3Aissue%20state%3Aopen%20label%3Alb-rs) for more info.

Similarly `workspace` is the cross platform UI implementation of our tab strip, editing experience and more. It's presently written using [egui](https://github.com/emilk/egui), an [immediate mode ui framework](https://en.wikipedia.org/wiki/Immediate_mode_(computer_graphics)) (similar to how game dev methodology). See the [workspace label](https://github.com/lockbook/lockbook/issues?q=is%3Aissue%20state%3Aopen%20label%3Aworkspace) for more info. Workspace houses our [markdown editor](editor.md) as well as our [canvas](canvas.md), some of our core editing experiences.

Our egui implementations are wrapped in *integrations* written by our team to provide our app with a rich user experience and populate events missing from `winit`. We send custom events through to `egui` for things like image paste, and we'd like to invest deeper in these integrations to provide a richer stylus integration on Windows. The windows & linux labels refer to these integrations.

`lb-rs`, `server`, and `cli` make use of some more exciting technologies documented in our blog:
- [db-rs](https://lockbook.net/blog/db-rs/)
- [lb-editor](https://lockbook.net/blog/lockbooks-editor/)
- [defect finder](https://lockbook.net/blog/defect-finder/)
- [cli-rs](https://lockbook.net/blog/creating-a-sick-cli/)

Much of our infrastructure operations are described in [`lbdev`](lbdev.md) powering most of our developer operations. We'd like to automate the process of shipping our code more often and to more places.

# Native UI Expertise (Swift / Kotlin)
Our apps on Android, and on Apple Platforms have a significant amount of *native* code to power a better user experience. When trying to decide whether we're going to implement something in Rust or in a Native framework we just ask the question which will lead to a better user experience. On most platforms we've implemented the file tree, search, settings and much more in in the native platform.

There are plenty of bugs to fix, and deeper connections to the host device to make. We'd like to have widgets, assistant integrations, and more on these host platforms.

# Reach Out!
All of these resources help you understand the various ways you could contribute, but if you're serious about joining our team, start a conversation with us! In that conversation we can understand your experience level, your interests and your goals and get you pointed in the right direction. 