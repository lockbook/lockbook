+++
title = "Introducing Lockbook"
date = 2022-11-29
[extra]
author = "parth"
author_link = "https://github.com/Parth"
+++

# A new note-taking app

Many moons ago my friends and I found ourselves frustrated with our computing experience. Our most important tools: our messengers, our note-taking apps, and our file storage all seemed to leave us with the same bitter taste in our mouths. Most of the mainstream solutions were trapped inside a browser, the epitome of sacrificing quality for the lowest common denominator. They didn’t pass muster for basic security and privacy concerns. Building on top of these systems was a painful experience. We were also growing concerned that we did not share the same ethics and values as the [large companies](https://bigtech.fail/) running these platforms.

So we ventured out into the land of FOSS. We built our systems from the ground up and experimented with self-hosted solutions. While we learned a lot from this experience, these solutions didn’t stand the test of time. As we brought our friends into this world, we found ourselves constantly apologizing for the sub-par experience. It felt like we solved some of our problems, but made other ones (UX) worse.

At this point, we took a step back. I knew we could do better. The apps we were being critical of weren’t some of the fresher ideas in computing. They are things that we’ve been doing for decades. Why don’t these products feel more mature? What should software that’s been around this long feel like?

## What is ideal?

Software that’s been around this long shouldn’t be trapped in the browser. A browser is a convenient place for the discovery of new information, it’s not the place I want to visit for heavily used, critical, applications. When I look at my devices, whether on my iPhone or my Linux laptop, the apps I can use with the least friction are simple, native applications. They have the largest context about the device I’m using encoded into the application. This friction-free experience is why people reach for Apple Notes on their iPhones. And when they open those same notes on their iPad they find rich support for their Apple Pencil. For me, a minimal, friction-free context-aware experience is more valuable than feature richness.

Whatever experience I have on one device should carry over seamlessly to any device I may end up owning in the future. My notes shouldn’t be trapped on Apple Devices should I want to transition to Linux. Very few actions should require a network connection, and any network interactions should be deferrable so people outside of metropolitan areas don’t have a poor experience.

For now, likely most of these services will have to interact with some sort of backend. Everything that backend receives should be encrypted by the customer themselves, in a manner that nobody besides them and the people they give access to can see that content. We shouldn’t ask the customer for any information the service doesn’t require. There is very little reason that a user _must_ provide an email address or a _phone number_ to use a note-taking app. This level of security and privacy shouldn’t _cost_ the user anything in terms of quality. Our customers may be whistle-blowers, journalists, or citizens living under oppressive regimes. They simply cannot afford to trust and they shouldn’t have to.

This software is too important to not open source. Any software claiming to be secure needs to be open source to prove that claim. Sensitive customers need the ability to build minimal clients with small dependency trees from sources on secure systems. Open-sourcing components like your server signal to the world that they can host critical infrastructure themselves, even if the people behind the product lose the will to keep the lights on. Open source doesn’t end in making the source code available, this software should be built out in the open with help from an enthusiastic community. People should be able to extend the tools for fun or profit with minimal friction.

## Reaching for an ideal

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fbucketeer-e05bbc84-baa3-437e-9518-adb32be77984.s3.amazonaws.com%2Fpublic%2Fimages%2Fb958cf3d-c608-4398-8d56-ca3429d86e26_1920x1216.jpeg)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fbucketeer-e05bbc84-baa3-437e-9518-adb32be77984.s3.amazonaws.com%2Fpublic%2Fimages%2Fb958cf3d-c608-4398-8d56-ca3429d86e26_1920x1216.jpeg)

After much discussion, we decided that the best place to start was a note-taking app. We felt it was the product category with the largest room for growth. Architecturally it also paves the way for us to tackle storing files. And so began the three-year-long journey to create [Lockbook](https://lockbook.net/) a body of work I’m proud to say has stayed true to the vision outlined above. At the moment, Lockbook is not quite ready for adoption as it’s in the early stages of alpha testing. But I’d like to use this space to share updates on our progress as well as document how we overcame some interesting engineering challenges like:

  * Productively maintaining several native apps with a small team

  * How we create rich non-web cross-platform UI elements.

  * How we leverage a powerful computer to find bugs.




If you’d like to learn more about Lockbook you can:

  * [Checkout our website](https://lockbook.net/)

  * [Browse our source code](https://github.com/lockbook/lockbook)

  * [Join our discord](https://discord.gg/lockbook)




If you’d like to take an early look at Lockbook, [we’re available on all platforms](https://github.com/lockbook/lockbook/tree/master/docs/guides/install).

  * [Github Releases](https://github.com/lockbook/lockbook/releases)

  * Apple App Store

  * Google Play

  * Brew

  * AUR

  * Snap




Parth’s Corner is a reader-supported publication. To receive new posts and support my work, consider becoming a free or paid subscriber.

Subscribe
