# Editor

We need an editor that is:

+ ultra customizable
+ cross-platform
+ high performance

## Ultra customizable

In order of importance / when we would need it:

+ ability to do basic spell check
+ The ability to render markdown inline
+ Ability to automatically insert characters (auto-insert bullets)
+ The ability to render known characters as custom glyphs (`+` becomes `•`)
+ Ability to render images / drawings inline
+ The ability to render check lists for todo lists
+ Checklists be interact-able
+ Ability to be able to create multiple cursors that represent multiple concurrent collaborators

## Cross Platform

+ Without fully committing to a UI Framework (like React Native) we should be able to embed this UI Component in
  existing native frameworks. We want our macOS app to look like a macOS app, not like an electron app.
+ The user should not be able to tell that this isn't a "Normal input field" say on iOS if they try to use features like
  dictation, autocorrect, or text highlighting.

## High Performance

+ While designing the system from the ground up, we should be able to give special attention to the fact that we're
  going to be attempting to parse and render markdown or other such languages. We may want to customize those
  languages (links for example). Having control of where and how text is inserted gives us the unique ability to create
  a fully incremental text editing stack.
+ Should scale to large documents, whatever strings are reasonable to put into memory at once (in the megabytes range)
  the text editor should be able to handle seamlessly:
    + should be able to type, resize the window, and scroll the buffer seamlessly.

# Project structure:

+ `apple/` - contains the editor wrapped as a swiftui component for use on macOS and iOS
+ `android/` - contains the editor wrapped as an android view
+ `egui/` - contains an egui binary that exposes the editor. Also contains a library interface used by android and iOS

## Current attempt

+ Using egui create an editor, try to integrate it into existing UI frameworks like SwiftUI.
+ egui is particularly well suited for integrating into foreign environments (like game engines).
+ leverage egui's existing pipeline that renders to opengl based.
+ convert events from the host UI framework into ones egui or our editor expects.
+ Precedent:
    + https://developer.apple.com/documentation/uikit/uitextinput
    + https://developer.android.com/reference/android/view/inputmethod/InputMethodManager
    + For the more advanced smartphone case, we don't want to be passing the traditional egui events into our editor, we
      want to pass events that are optimized for smartphones. The above interfaces provide us with those things, they
      model the behavior of someone "composing" a word, the autocorrect behaviors specific to those platforms, and
      things like speech-to-text dictation.
    + Scroll calculations will also happen by the host system:
        + for instance, this is how android can tell us with what velocity to
          fling our view during
          scrolling: https://developer.android.com/reference/android/view/GestureDetector.OnGestureListener
        + and on iOS: https://whackylabs.com/metal/2021/05/16/metal-scroll-view/
+ Next unknown: How will text layout happen within egui? How do we answer questions around the widths of various
  characters across different font modes (monospace, bold, etc) and sizes (h1, h2, etc)?
+ `.ttf` files include data for the curves involved for drawing each character. Figuring out bounds is simply the
  process of seeing what the curve's bounding regions will be at a certain scale.
+ In egui, within epaint there is stuff that draws glyphs, does layout of text, and then ui components like `label`
  display these regions of text and handle user interactivity. Likely we'll want to re-implement this top most layer,
  and not go too much deeper.
+ How will the integration actually happen?
    + We've already demonstrated our ability to run rust code on foreign platforms (core).
    + But now we want rust to control the graphics layer on a foreign platform.
    + There's a good bit of precent here too:
        + On iOS, `MTKView` can be interacted with using objective-c (a C superset) which can trivially call out to
          rust. Here is a sample of someone controlling the graphics layer from rust within
          iOS: https://github.com/Gordon-F/miniquad_ios_example. This project also has an android example, and we'll
          talk about miniquad shortly.
    + So we can likely draw shapes, but what we'd actually like to do is reuse egui's model of what things look like, so
      how do we do that?
    + Let's review a handful of crates in this ecosystem:
        + [egui](https://github.com/emilk/egui) - the GitHub repo contains many sub crates, but conceptually egui refers
          to immediate mode gui library which is drawing "textured polygons" in an abstract way. It expects an "
          integration" to provide it with events that have occurred, and take those textured triangles and send it to a
          specified backend which will draw those things.
        + By default `egui` suggests you use `eframe` as the integration. `eframe` uses `glow` which is an `openGL`
          based rendering pipeline. But has optional support for using `wgpu` which would allow you to take advantage of
          platform specific rendering pipelines (like metal). Using the native pipelines leads to noticeable gains even
          for simple hello worlds.
        + `winit` one of many integration choices with the `egui` ecosystem. Provides an expression of what is common
          among platforms. It provides a pointer for initializing opengl and so when one uses this integration. It's
          unclear to me what exactly bridges the gap between the
          two. [this is how the gap is bridged](https://github.com/hasenbanck/egui_example/blob/master/src/main.rs),
          this is an example of how to launch a window with winit, use wgpu for gl handling, and egui for ui stuff.
          Studying this is probably going to be a good starting point for us. I wonder if we can use that example with
          metal (if winit is involved in the creation of the graphics context no, if wgpu handles this then it would be)
          . Winit also seems to have some level
          of [mobile support](https://github.com/rust-windowing/winit/blob/master/FEATURES.md#windowing-1), but I
          believe this support is more about window creation, querying current window and event state, it's not outright
          android support. This seems like an [android specific](https://github.com/rust-windowing/android-ndk-rs)
          implementation of winit, and [`eframe` uses this on android](https://github.com/emilk/egui/discussions/2053)
        + `wgpu` pure rust graphics abstraction layer. Abstracts over metal, vulkan, D3, opengl and wasm. Using this is
          almost certainly going to be a part of our implementation. Asking eframe to use `wgpu` instead of `glow` means
          I'm using metal instead of opengl which results in significant noticeable performance gains.
        + `miniquad` is competing with `wgpu` but only supports opengl. It likely a direct competitor to `glow` but has
          a stated goal of fast compilation speeds. It seems like
          it [messes up color](https://github.com/emilk/egui/issues/93#issuecomment-758106824) in some fundamental way.
          It also has a stated goal of fast compilation speeds which isn't something this (egui or rust) community
          [seems to care about](https://github.com/emilk/egui/issues/93#issuecomment-768389533). So likely `glow` became
          the default backend for eframe with an option to fall back to `wgpu`. `miniquad` however does have some good
          examples for:
            + running miniquad on mobile: https://github.com/not-fl3/miniquad
            + miniquad + egui: https://github.com/frgomes/egui-miniquad-demo
            + so while I don't think we'd use miniquad directly, it is probably a good implementation to study.

## Existing attempts

We've tried a handful of strategies so far:

+ Using cross-platform UI frameworks like flutter. Was generally a bad time, didn't feel like the flutter ecosystem was
  mature enough, and dart was not an inspiring language to use to contribute to that ecosystem. (Faced similar
  fundamental problems with the world of javascript)
+ Using native, community created widgets on Android and iOS. Generally very hard to customize. Projects require direct
  modification to suit our needs. And still those projects are built on top of retained-mode UI frameworks inside which
    + there are fundamental limits to what's possible (language limitations)
    + [very poorly documented](https://christiantietze.de/posts/2017/11/syntax-highlight-nstextstorage-insertion-point-change/)
    + unergonomic to work with (`NSString` vs `String` vs `Down`)