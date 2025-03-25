+++
title = "Why Lockbook chose Rust"
date = 2023-01-02
[extra]
author = "parth"
author_link = "https://github.com/Parth"
+++

[Lockbook](https://lockbook.net/) began it’s [journey](https://parth.cafe/p/introducing-lockbook) as a bash script. As it started to evolve into something more serious, one of our earliest challenges was identifying a UI framework we were willing to bet on. As we explored, we were weighing things like UI quality, developer experience, language selections, and so on.

Our choice of UI framework had implications for our server as well. If we chose JavaFX and native Android, we would likely want to choose a JVM-based language for our server to share as much code as possible.

As we wrote and re-wrote our application, we discovered that most of our effort, even on our clients, was not front-end code. When we were implementing user management, billing, file operations, collaboration, compression, and encryption, the lion’s share of the work was around traditional backend-oriented tasks. Things like data modeling, error propagation, managing complex logic, handling database interactions, and writing tests were where we were spending most of our time. Many of these things had to take place on our clients because all our user data is [end-to-end-encrypted](https://en.wikipedia.org/wiki/End-to-end_encryption). Additionally, some of these operations were sensitive to slight differences in implementation. If your encryption and decryption are subtly different across two different clients, your file may be unreadable.

It was also becoming clear to us that the applications that looked and felt the best to us were created in that platform’s native UI framework. So our initial investigation around UI frameworks morphed into an inquiry into what the best repository for business logic was. Ideally, this repository would give us great tools for writing complex business logic and would be ultimately portable.

[![Screenshot for ](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fbucketeer-e05bbc84-baa3-437e-9518-adb32be77984.s3.amazonaws.com%2Fpublic%2Fimages%2F8e8f9520-2b63-4f2c-ac05-20a12f699a82_612x408.jpeg)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fbucketeer-e05bbc84-baa3-437e-9518-adb32be77984.s3.amazonaws.com%2Fpublic%2Fimages%2F8e8f9520-2b63-4f2c-ac05-20a12f699a82_612x408.jpeg)

## Tools for managing complexity

Our collective experience made us gravitate towards a particular spirit. At [Gemini](https://www.gemini.com/), [Raayan](https://raayan.net/) and I saw how productive we were within a foreign, large-scale, Scala codebase. Informed by the experience we were looking for a language with an expressive, robust type system.

A “robust type system” goes beyond what you’d find in languages like Java, Python, or Go. We were looking for type systems where `null` or `nil` were the exception, rather than the norm. We want it to be apparent when a function could return an error, or an empty value, and have ergonomic ways to handle those scenarios.

We wanted to have sophisticated interactions with things like `enums`, specifically, we wanted to be able to model the idea of exhaustivity. When an idea we were working with evolved to have more _cases_ we wanted our compiler to guide us to all the locations that need to be updated.

There were a handful of other features we were looking for which can broadly be categorized into two similar ideas:

We wanted to express as much as we could in our primary programming language. Things that would traditionally be documented ( _this fn will return null in this situation_ ) or things that would be expressed in configuration (TLS configuration handled by a different program in YAML) would ideally be expressed in a language that contributors understood intimately. Ideally in a language where the compiler was providing strong guarantees against mistakes and misuse.

We wanted our language and tools to help us detect defects as [early as possible](https://en.wikipedia.org/wiki/Shift-left_testing) in the development lifecycle. Most software developers are used to trying to capture defects at test time, but we found that trying to capture defects even earlier, at compile time, allowed us to drop into [flow](https://en.wikipedia.org/wiki/Flow_\(psychology\)) more easily. The following is our preference for when we’d like to catch defects:

  1. at compile time

  2. test time

  3. startup time

  4. pr time

  5. internal test time

  6. by a customer




Our strongest contenders for languages here were Rust, Haskell, and Scala.

## Ultra-portability

Ideally, this repository would not place constraints on where it could be used. If our repository was in Scala, for instance, we’d be able to use it on Desktop, our Server, and Android, but we’d run into problems on Apple devices.

We could use something like JS, virtually every platform has a way to invoke some sort of WebView which allows you to execute JS. But we’d had plenty of [bad experiences](https://www.destroyallsoftware.com/talks/wat) with vanilla javascript. We found that evolutions on JS like Typescript were also on a [shaky foundation](https://www.youtube.com/watch?v=jjMbPt_H3RQ). Despite the JS ecosystem being popular and old, it didn’t feel very mature. Finally, we didn’t like the way most JS-based applications, whether [Electron](https://medium.com/commitlog/why-i-still-use-vim-67afd76b4db6) or React Native [felt](https://parth.cafe/i/87442376/what-is-ideal).

Both JS and Scala would require tremendous overhead due to the default environments in which they run. We needed something lighter weight than invoking a little browser every time we wanted to call into our _core_. Our team members were pretty experienced in Golang, and [Cgo](https://go.dev/blog/cgo) was an ideal fit for what we were looking for. It would allow us to ship our _core_ as a C library accessible from any programing language we were interested in inter-operating with. There were some concerns we had about the long-term overhead of [cgo](https://github.com/dyu/ffi-overhead) and garbage collection generally, but those wouldn’t be immediate concerns.

Similarly, Rust had a pretty rich collection of tools for generating C bindings for Rust programs and a pretty mature conceptualization of [FFI](https://en.wikipedia.org/wiki/Foreign_function_interface). Though it wasn’t an immediate criterion we were inspired by the fact that most everything in Rust was a [zero-cost abstraction](https://stackoverflow.com/questions/69178380/what-does-zero-cost-abstraction-mean). In that spirit, FFI in Rust would have virtually no additional overhead when compared to a C program. We were also drawn to [Cargo](https://doc.rust-lang.org/cargo/) which felt like the package manager for a language we were waiting for, particularly useful for our complicated build process.

Our strongest contenders for languages here were Rust, Go, and C.

## Taking the plunge

Learning Rust wasn’t a smooth process, but solid documentation helped us overcome the steep learning curve. Every language I’ve learned so far has shaped the way I view programming, it was refreshing to see the interaction of high-level concepts like Iterators, Dynamic Dispatch, and pattern matching discussed alongside their performance implications.

Rust has an interesting approach to memory management: it heavily restricts what you can do with references. In return, it will guarantee all your references are always valid and free of race conditions. It will do this at compile time, without the need for any costly runtime abstraction like Garbage Collection.

Once we were over the learning curve we prototyped the [core library](https://github.com/lockbook/lockbook/tree/master/core) we’d been planning, a CLI, and a Server that used it. During a period when many of us were rapidly prototyping many different solutions, this was the one that stood the test of time. Soon after the CLI, a C binding followed, then an [iOS and macOS application](https://apps.apple.com/us/app/lockbook/id1526775001). Today we have a [JNI bindings](https://en.wikipedia.org/wiki/Java_Native_Interface) and an [Android app](https://play.google.com/store/apps/details?id=app.lockbook&pli=1) as well. This core library will one day be packaged and documented as the _Lockbook SDK_ allowing you to extend Lockbook from any language (more on this later).

## Further personal reflections

You can probably predict what your experience with Rust is going to be based on how you felt about the above two priorities. Rust is an experiment in the highest-level features implemented at no runtime cost. If you feel like the `Option<T>` is not a useful construct, you’re not likely to appreciate waiting for the compiler. If you don’t mind the latency introduced by garbage collection you’re not going to enjoy wrestling the [borrow checker](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html).

I wasn’t specifically seeking out performance, but before Rust, while programming there was always a slight uncertainty about whether I would have to re-write a given component in C, or spend time tuning a garbage collector. In Rust I don’t write everything optimally initially, when I need to, I’ll `clone()` things or stick them in an `Arc<Mutex<T>>` to revisit at a later time, but I appreciate that all these artifacts of the productivity vs. performance trade-offs are explicitly present for me to review, rather than implicitly constrained by my development environment.

For our team, learning Rust has certainly been a dynamic in onboarding new contributors. Certainly, we’ve lost contributors who didn’t buy into the ideas and were turned away because of Rust. But we’ve also encountered people who are specifically seeking out Rust projects because they share our excitement. It’s hard to tell what the net impact here is, but as is the case every year: Rust is a [a language a lot of people love](https://survey.stackoverflow.co/2022/). Significant Open Source and Commercial entities from Linux to AWS are making permanent investments in Rust.

This excitement does however bring a lot of Junior talent to the ecosystem, subsequently, even though it’s roughly as old as Go, many of Rust’s packages feel like they’re not ready for production. By my estimation, this is because in addition to understanding the subject matter of the package they’re creating a maintainer of a library needs to understand Rust pretty deeply. Additionally, within the Rust ecosystem, some people are optimizing for different things. Some people are optimizing for compile times and binary size, while others are optimizing for static inference and performance, in many cases these are mutually exclusive values.

In some cases, this is a short-term problem as features are stabilized, and best practices are identified. In other cases, this is an irreconcilable aspect of the ecosystem that will simply result in lots of packages that are solving the same problem in slightly different ways.

This is something we should expect, as Rust is a language that’s trying to serve all programmers from UI developers to OS designers. And though it may cost me some productivity in the short term while I’m forced to contend with this nuance, in the long term it massively broadens my horizons as a software engineer.

Personally what got me over the steep learning curve is a rare feeling that the knowledge I’m building while learning Rust is a permanent investment in my future, not a trivial detail about a flaw of the tool I’m using. I’m very excited to see where Rust takes us all in the future.
