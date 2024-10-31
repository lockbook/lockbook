# Contributing Guidelines

We're honored that you want to contribute to Lockbook.

This document outlines several approaches to contributions, as well as codifying some standards.

# Communication and Documentation

We store all of our documentation in the `docs/` folder. This includes design documents and guides for processes like building from source.

We use a [discord server](https://discord.gg/lockbook) for all our communications. **Don't hesitate to drop by and discuss anything that would help you contribute.**

Explore our github issues for an up-to-date catalogue of potential areas of contributions.

## QA & Product Oriented Thinking

We're striving to create a high quality note taking platform. Simply putting Lockbook on your devices, trying to integrate it into your life, and sharing feedback with our team is very valuable. Join our discord, tell us what you like and tell us what you're longing for!

If you encounter a problem, the value of your find is tightly tied to the reproducibility of the problem. A screen capture that reliably triggers the problem, a series of CLI commands, or a failing unit test are some very valuable ways to make us aware of problems.

You may find a workflow that's particularly clunky. Sharing the specifics of your workflow in Discord can be very helpful. Sometimes it's not clear what the right solution is and thinking through your target state user experience can greatly improve the quality of discourse.

If you think Lockbook would be a good fit at your place of work and would like our team to demo the product let's connect.

## Engineering contributions

The surface area of valuable engineering contributions is very large. We're happy for this to be a conversation in Discord. But here is a non-exhuastive list of contributions to draw inspiration from:

+ cli-rs, and the various CLIs that use them: We've created our [own cli parsing library](https://blog.lockbook.net/cp/137878891), and have a handful of CLIs that use it. Contribution opportunties exist in the library itself to expand upon functionality as well as in the CLIs themselves.
+ [lb (formerly core)](https://blog.lockbook.net/cp/136569912) the library that contains our platform's core logic that is shipped everywhere (all clis, our server, and all clients). This library was one of the first pieces of our infrastructure. It stabilized before we knew what some of the best practices were for such a library. Contribution opportunities here include straight forward code cleanup, better FFI, and performance improvements.
+ lb-sdk: presently we have wrappers for the lb libraries in Swift, Kotlin, and C. The packaging of these libraries are optimized for our consumption, but with some effort they could be packaged for consumption by the wider community. Addtional languages that we don't need could be added (js, python, etc). 
+ Native Apps: We have native iOS, macOS, and Android apps. There exist large areas of contributions for both of these platforms, these are fairly well catalogued in our github issues. 
+ db-rs: we've written a [tiny database optimized for our productivity](https://blog.lockbook.net/cp/136569984). As it's abilities grow so do our platform's, if you have experience with database systems implementation, let's talk!
+ server: our server is written in async rust. Similar code cleanup opportunites exist along with interesting approaches to managing documents which could help us offer more competitive pricing.
+ infra: We depend on a number of infrastructure elements to continuously test, monitor, and release code. Significant areas of contribution exist in the realm of packaging lockbook and helping more communities easily access our software.
+ 3rd party apps: build something interesting using the lockbook sdk and write a blog post about it! Share it with us and share it with the world! Reach out to us if lb-sdk doesn't meet your needs in some way.
+ egui: we bring a UI compment we've engineered in rust to all the platforms. This presents a unique opportunity to author specific support for other data formats (todo lists, workout trackers, etc). Also provides a unique opportunity to visualize data (graph) or even provide a view that lets someone interact with an LLM. Our windows and linux apps are also written entirely in egui, similar to Native apps, significant contribution opportunities exist on those platforms as well.
+ egui-integrations: to support some of our specific application requirements (stylus support, embedding inside swiftui) we've created egui integrations for android, windows, linux, and apple. If you have experience with the low level APIs of these platforms you could help us support more features like stylus support for our drawing experience.
+ "fuzzer": we wrote a [highly optimized program that tries to find bugs within `lb`](https://blog.lockbook.net/cp/136570081) trying many permutations of actions a user could take to put their or other people's file trees into invalid states. Example contributions here could include: making it even faster (at this point requires making lb faster which is win win), making it stricter, making it smarter, making it more productive for triaging bugs.
+ many more things, reach out!

## Marketing contributions

We're trying to build our product as transparently and publicly as possible. We have various team members working on various social media platforms to help grow our community, and share updates as they come. You can help us out by doing whatever algorithmic things that platform requires. Star our github repository, subscribe to us on youtube, follow us on instagram, etc. Share our posts with networks of people whos values align with ours!

## Financial contributions

At the moment we're a completely out-of-pocket funded operation and your financial support goes a long way. Use the app and convert to premium if you're sold on our product.

If you'd like to make a more substantial contribution reach out to us on Discord.

# Standards

## Commit Formats

Todo's in code should appear as `// todo` to make automated searching as easy as possible.

PRs should adhere to the following format:
```
category: a summary

~~~~~~~~~~~~~
~~~context~~~
~~~~~~~~~~~~~

[closes / fixes statements]
```

PRs should also outline what level of QA was completed on what platforms. PRs merged to `master` should be immediately releasable or, in rare circumstances should outline their risks and outline what level of `master` dogfooding is required before release.

Descriptions generally should contain some element of a demo if appropriate.

Possible categories include the following:
- all: significant, multicomponent update
- docs
- canvas
- editor
- workspace
- public_site
- lb-rs
- server
- cli
- linux
- android
- iOS
- macOS
- windows
- infra
