+++
title = "Lockbook's Editor"
date = 2023-06-12
[extra]
author = "parth"
author_link = "https://github.com/Parth"
+++



[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2Fa916acc9-8271-495e-b20c-ae853d06a34f_2000x1500.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2Fa916acc9-8271-495e-b20c-ae853d06a34f_2000x1500.png)

As [Lockbook](https://parth.cafe/p/introducing-lockbook)'s infrastructure stabilized, we began to focus on our markdown editing experience. Across our various platforms, we've experimented with a few approaches for providing our users with an ergonomic way to edit their files. On Android, we use [Markwon](https://github.com/noties/Markwon), a community-built markdown editor. On Apple, we initially did the same thing but found that the community components didn't have many of the features our users were asking for. So as a next step, we dove into Apple's [TextKit](https://developer.apple.com/documentation/appkit/textkit) API to begin work on a more ambitious editor.

Initially, this was fine, but as we worked through our backlog of requests, I found things that were going to be very time-expensive to implement using this API. We were having performance problems when editing large documents. The API was difficult to work with, especially because there were no open existing bodies of work that implemented features like automatic insertion of text (when adding to a list), support for non-text-characters (bullets, checkboxes, inline images), or multiple cursors (real-time collaboration or power user text editing). Even if we did invest the effort to pioneer these features using TextKit, we would have to replicate our efforts on our other platforms. And lastly, none of my other teammates knew the TextKit API intimately, so I wouldn't be able to easily call on their help for one of the most important aspects of our product. We needed a different approach.

In the past, I've discussed our [core library](https://parth.cafe/p/why-lockbook-chose-rust) \-- a place we've been able to solve some of our hardest "backend" problems and bring them to foreign programming environments. We needed something like this for a UI component we needed a place where we could invest the time, build an editor from the ground up, and embed it in any UI library.

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F31cca6aa-cf31-4f08-9a9e-f440fb533048_3000x2000.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F31cca6aa-cf31-4f08-9a9e-f440fb533048_3000x2000.png)

We considered creating a web component. Perhaps we could mitigate some of the downsides of web apps if we were only presenting a web-based view when a document was loaded. Maybe we could leverage Rust's great support for web assembly for the complicated internals. Ultimately I felt like we could do better, so I continued thinking about the problem. On Linux, we'd begun experimenting with [egui](https://github.com/emilk/egui): a lightweight, Rust, UI library. Their README had a long list of places you could embed egui, and I wondered if I could add SwiftUI or Android to that list.

And so began my journey of gaining a deeper understanding of [wgpu](https://wgpu.rs/), [immediate mode UIs](https://eliasnaur.com/blog/immediate-mode-gui-programming), and how this editor might work on mobile devices.

Most UI frameworks have an API for directly interfacing with a lower-level graphics API. In SwiftUI, for instance, you can create an `MTKView` which gives you access to [MetalKit](https://developer.apple.com/documentation/metalkit) (Apple's hardware accelerated graphics API). Using this view, you can effectively pass a reference to the GPU into Rust code and initialize an egui component. In the host UI framework you can capture whichever events you need (keyboard & mouse events for instance) and pass them to the embedded UI framework. It's the simplicity of immediate mode programming which enables this to be achievable in a short period, and it's the flexibility of immediate mode programming which makes it a great choice for complex and ambitious UI components. The approach seemed like it held promise so we gave it a go.

After a month of prototyping and pair programming with my co-founder Travis, we had done it. We shipped a version of our Text Editor on macOS, Windows, and Linux which supported many of the features our team and users had been craving. The editor was incredibly high-performance, easily achieving 120fps on massive documents. Most importantly we have a clear picture of how we would go about implementing our most ambitious features over the next couple of years.

After we released the editor on the desktop, we began the process of bringing it to mobile devices. This was a new frontier for this approach. On macOS, we just had to pass through keyboard and mouse events. On a mobile device, there are many subtle ways people can edit documents. There are auto-correct, speech-to-text, and clever ways to navigate documents. After some research, we found a neatly documented protocol -- `UITextInput` \-- which outlines the various ways in which you can interact with a software keyboard on iOS. We also found a [corresponding document](https://developer.android.com/reference/android/view/inputmethod/InputMethodManager) in Android's documentation.

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F5308aaf8-09b9-440e-802b-209b32204490_600x1300.gif)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F5308aaf8-09b9-440e-802b-209b32204490_600x1300.gif)

 _(on iOS you can quickly move the cursor by long-pressing the spacebar)_

So back to work we went. We expanded on our SwiftUI <\--> egui integration giving it the ability to handle events that egui doesn't recognize. We piped through these new events, refined the way we handle mouse/touch inputs, and a couple of weeks ago, we merged our iOS editor bringing many of our gains to a new form factor.

We're very excited about the possibilities this technique opens up for us. It allows us to maintain the look & feel that users crave while giving us an escape hatch down into our preferred programming environment when we need it. Once our editor is more mature and the kinks of our integration are worked out, we plan to apply this strategy to more document types. Long term we're interested in making it easy for people to quickly spin up their own SwiftUI component backed by Rust (as presently this still requires a lot of boilerplate code).

On net, the editor has been a big step forward for us. It's already live on desktops and will be shipping on iOS as part of our upcoming 0.7.5 release. It's a large and fresh body of work, so we anticipate some bugs. If you encounter any, please report them to our [Github issues](https://github.com/lockbook/lockbook/issues). And, as always, if you'd like to join our community, we'd love to have you on our [Discord server](https://discord.gg/lockbook).
