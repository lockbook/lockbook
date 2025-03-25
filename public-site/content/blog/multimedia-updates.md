+++
title = "Multimedia Updates!"
date = 2024-07-06
[extra]
author = "parth"
author_link = "https://github.com/Parth"
+++


In a few videos we've expressed that we've been focusing on building up our infrastructure to better support [multimedia](https://www.youtube.com/watch?v=5w-kDNu5rz0) and increase overall platform stability. Today I'd like to share some exciting updates on the multimedia front!

## Lockbook Workspace

In a previous post, we shared details about our [cross-platform markdown editor](https://blog.lockbook.net/cp/136569994), which allowed us to increase the complexity and interactivity of our editor. After working out the kinks with the initial implementation we've doubled down on this strategy and expanded it to the entire tab strip and all content displayed within Lockbook. We call this component the _Lockbook Workspace_. As a portion of the team brought workspace to all of the platforms, Adam redesigned our drawing experience from the ground up to use SVGs instead of a proprietary drawing format. **Canvas** deserves its own post, so stay tuned for that. In addition to SVGs and Markdown, workspace brought image and PDF previews to all platforms.

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2Ff16e346f-be0a-4731-bdcb-9ba4c3625654_2598x1754.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2Ff16e346f-be0a-4731-bdcb-9ba4c3625654_2598x1754.png)

Markdown and SVG have crystalized as the 2 main document formats that will be editable inside Lockbook. These open formats are a natural fit for our customer-centric values. They both offer broad compatibility across the internet and don't lock you into our platform. The freeform expression of SVGs complements the restricted feature set of Markdown nicely. SVGs are also trivially embedded inside markdown documents as images.

Supporting these two document types offers our platform access to two very diverse types of note-takers. On one hand, there's the sort of people who like apps like OneNote, Notability, and GoodNotes for their unstructured creative surface. On the other hand, some people like the structure of apps like Notion, Obsidian, and Google Docs for their searchable, linkable, and collaborative experience.

A key idea of our platform is to not split your life among different apps all fundamentally storing bytes for you and your team. My co-founder Travis and I are members of both of these modalities. Users like my Dad are heavy OneNote users, while there are plenty of users who are pretty firmly in the "strongly typed" category of markdown.

## Lockbook Filesystem

When we were making early design decisions for Lockbook's system architecture, we considered the many ways we could allow our users to organize their thoughts. We considered a tag-like system similar to Bear that optimizes for flexibility (a note could live in two folders at once). We also considered a Google Keep-like single-note experience. But I strongly advocated for a "Files and Folders" hierarchy. I knew I wanted Lockbook to integrate deep into the traditional computing experience seamlessly. There are times when you want to edit Markdown using our fancy Markdown editor. And there are times when you want to edit spreadsheets and photos and have all the durability and security guarantees of Lockbook. Again the key idea here is to not split your life across various apps, we don't want you to have to use Google Drive / DropBox for one type of content and Lockbook for another.

As Lockbook emerges out of its state of running an ungodly amount of experiments, this was another area I wanted to de-risk and see how our platform and infrastructure performed. Tax season was also approaching and we had similarly flavored requests from people who wanted to use the types of apps we're never going to be in the business of creating.

We _somewhat supported_ the basic workflow of dropping files in and out of Lockbook. But this is clunky and our users expect more from our ragtag crew of part-time developers. So I took a long weekend and tried to determine if we could do better.

The first thing that came to mind was to use the [FUSE](https://en.wikipedia.org/wiki/Filesystem_in_Userspace) protocol. This would allow our users to seamlessly **mount** their Lockbook as if it's a flash drive on their computer. Any requests for bytes would be fielded directly by the Lockbook instance running on your computer and we don't need to be in the business of watching directories for changes and trying to reconcile changes. This was our target UX but unfortunately, FUSE only works well on Linux, and on macOS users would have to install some pretty invasive 3rd party software.

A close second flavor of the same thing is the [NFS](https://en.wikipedia.org/wiki/Network_File_System) protocol. Offering a similar experience to FUSE, with some additional baggage associated with the network. Fortunately, this works pretty seamlessly on both macOS and Linux. I stumbled upon an [NFS-Server](https://github.com/xetdata/nfsserve) crate which made prototyping a very productive experience.

In a couple of days, I had a high-performance implementation ready. Mounting my whole Lockbook directory and seeing it work effortlessly with Lightroom, Keynote, and CAD software was a pretty magical moment for me as an engineer.

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2Fb03f860f-f925-488d-bc35-f9ddc8883d0a_1908x1266.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2Fb03f860f-f925-488d-bc35-f9ddc8883d0a_1908x1266.png)

We shipped an early preview of lb_fs in our `CLI` in 0.9.0. So far we've experienced some good feedback. We even had one of our users use the filesystem as part of an [automation pipeline](https://coreycc.com/md/blog/backup-dotfiles-with-lockbook.md) for their dotfiles!

We're excited about the future of lb-fs. We intend to integrate it directly into our desktop apps -- allowing you to click on any document not supported by Workspace and open it natively on your computer. We also want you to be able to click any folder and "Open Externally". Allowing you to seamlessly backup the decrypted contents as easily as copying them to a different location on your computer.

But at the moment we're pausing for some reflection: should lbfs continue investing in NFS? On Windows, it requires Windows Pro (which most users don't have, and there is no 3rd party stopgap). Potentially, we could seek a higher quality platform-specific interface like [ProjFS](https://learn.microsoft.com/en-us/windows/win32/projfs/projected-file-system). If we were to explore something like that on macOS we could add nice touches like showing the sync status or collaboration details within Finder itself.

This could also be a cool opportunity to create a state-of-the-art, cross-platform virtual file system abstraction for the Rust ecosystem. If you'd be interested in pursuing something like that please [join our Discord](https://discord.gg/lockbook) and reach out! We'd be happy to support you in any way we can.

## Other Infrastructural updates

Further investments in multimedia right now are bottlenecked by our current networking implementation. We expect it to be a small lift to unlock the potential of ws and fs by using a slightly more sophisticated approach to networking:

  * don't sync all your files all the time -- don't sync large files to my iPhone, let me log in immediately and lazily fetch files as needed (still have a well-managed cache for great offline support).

  * be able to more reliably sync large files -- presently, especially under adverse network circumstances, large files are particularly problematic.

  * fetch and push files in parallel.




These features and a few others comprise our [Sync 4.0 tracker](https://github.com/lockbook/lockbook/issues/2214), the product of which has significant implications for everything mentioned above. Sync 4.0 should also allow us to sync way more aggressively (where appropriate) -- a long-standing request from almost every type of user.

Once we have this situation under control we can start to look toward a future where we have a richer set of tools for collaboration:

  * document revisions

  * document comments

  * author history (like git blame)

  * dare I say -- Google Docs style real-time collaborative editing?




These features will likely be presented in workspace itself, but will be present for all file types fundamentally.

You can track the broader multimedia efforts [here](https://github.com/lockbook/lockbook/issues/1947).

If you have a specific use case that we didn't cover above we'd love to hear from you! [Join our Discord](https://discord.gg/lockbook) and share your thoughts!

## Footnote: Community Document Types

Another interesting opportunity that workspace presents our community is the ability to author custom new document types. If you can come up with a data format, and write an [egui](https://github.com/emilk/egui) widget then you can contribute a new file type to Lockbook workspace.

Presently we have some community members pursuing interesting visualizations of disk space and links within markdown documents inspired by _Disk Usage Analyzer_ on Linux and the _[Obsidian Graph View](https://help.obsidian.md/Plugins/Graph+view)_.

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F11fb7816-0c6b-4f6f-9e73-6b8a258b5f36_957x625.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F11fb7816-0c6b-4f6f-9e73-6b8a258b5f36_957x625.png)

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F678cce90-3b8d-44cf-97f6-be5050e36f03_1199x980.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F678cce90-3b8d-44cf-97f6-be5050e36f03_1199x980.png)

We've also heard lots of interesting ideas about building habit trackers and to-do lists this way as well. If you're interested in whipping up something like this [join our Discord!](https://discord.gg/lockbook) For now, we will play an active role in what file types make it to all users, but one day this structure may grow into a formal plugin system!
