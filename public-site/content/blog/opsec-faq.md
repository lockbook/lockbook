+++
title = "OpSec FAQ"
date = 2024-10-03
[extra]
author = "parth"
author_link = "https://github.com/Parth"
+++

# What does secure mean?

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F16b81878-96c9-4e65-bc75-e88c829a572a_1604x716.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F16b81878-96c9-4e65-bc75-e88c829a572a_1604x716.png)

We call [Lockbook](https://blog.lockbook.net/cp/136569024) a secure product. This isn't a unique claim to make as you can see below.

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2Fc953a99c-81ef-4325-9690-687e0ac4e26b_1202x1042.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2Fc953a99c-81ef-4325-9690-687e0ac4e26b_1202x1042.png)

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F3cb8991a-aedb-4244-b958-8a689882b9fc_872x444.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F3cb8991a-aedb-4244-b958-8a689882b9fc_872x444.png)

So what's the difference between Notion's claim and ours? It's simple, we can't see your notes even if we wanted to, and Notion can. There may be policies, protections, and certifications to ensure they don't do anything _improper_ with your data, but they have the _ability_ to see your content. In contrast, we don't have that ability.

Before your content departs from your device it's _encrypted_ in such a way that only you or any collaborators you chose can _decrypt_ the data. Our server never receives the decryption keys so we can't spy on you, our hosting provider can't spy on you and the government can't spy on you. This is called [end-to-end encryption](https://en.wikipedia.org/wiki/End-to-end_encryption). You can learn more about this [here](https://www.youtube.com/watch?v=jkV1KEJGKRA).

# How do I know I can trust Lockbook?

Because we don't believe in [security through obscurity](https://en.wikipedia.org/wiki/Security_through_obscurity), all of our code is [open source](https://github.com/lockbook/lockbook). We want to make it as easy as possible for someone to audit our code. All of the code that would be relevant to an audit is written in a single language and relies on well-established cryptographic primitives like [eliptic curve cryptography](https://www.youtube.com/watch?v=NF1pwjL9-DE) and [AES](https://www.youtube.com/watch?v=O4xNJsjtN6E).

The keen-eyed among us ask: how do we know you're running and shipping the code that's on GitHub? You don't, but we engineered lockbook so that the app running on your phone (the client) doesn't trust our server either. It doesn't send any secrets and verifies (cryptographically) the information it receives.

To be frank -- we don't trust our server either! We consider our cloud infrastructure to be an adversarial environment and have engineered the whole product to be resilient to snooping and unexpected downtime. This is part of the reason we placed such a great emphasis on offline support. Long term we envision a decentralized future (much like the email protocol), but that's a daydream for the time being.

For your convenience, we build and publish our apps to a variety of marketplaces of varying trustworthiness (like Apple's App Store). But we've also made it as easy as possible for you to build any of our apps directly from source to cut out all the middlemen -- allowing you to be absolutely certainty that you're running the code you expect.

# Why should I care if Google can see my content?

Some people are shocked to find out that most companies can just see all your content.

Others don't really care if some select group of people have access to their documents. They don't fear the government because they don't think they are or ever will be a target (I hope they're right).

Despite this, they still don't want their friends, families, or enemies to see their content. It's private and they'd like to keep it that way.

Companies, however, [get ](https://en.wikipedia.org/wiki/List_of_data_breaches)**[hacked](https://en.wikipedia.org/wiki/List_of_data_breaches)**[ all the time](https://en.wikipedia.org/wiki/List_of_data_breaches). They leak passwords emails, and content **all the time**. The perpetrator could be anyone from a foreign government to a talented prepubescent basement dweller. It could be a disgruntled employee or a misconfigured database. When the data sitting on the server is compromised do you want it to be a garbled mess of encrypted data, or do you want it to be every photo you've taken in the last 15 years?

# So is my Lockbook unhackable?

A useful mental model for determining how secure you are is something like "How much would it cost to compromise me?".

To compromise most traditional companies the exploit may be as simple as waiting for an employee to make a mistake and expose secrets. If that doesn't work an attacker could explore bribing an employee or even infiltrating the company.

Because the data sitting on our server is encrypted, for someone to gain access to your content they have to compromise your individual device. Hacking into an individual's device is several orders of magnitude more expensive. Apple is willing to pay up to [$2M bounties](https://security.apple.com/bounty/categories/) for certain types of device compromises. Widescale compromises of devices like the iPhone are far more rare than the compromises of online services.

Often with these types of compromise physical access is required. So if someone wants to get your content they may need to hire a thief to break into your home, and then use an exploit that may be worth millions of dollars.

Additionally, Lockbook goes to some lengths to make sure you don't compromise yourself. We generate a key for you with 256 bits of entropy (a very long unguessable password). This means you can't accidentally re-use a password and compromise yourself.

This is an irrecoverable key eliminating the chance of an [email or phone compromise](https://medium.com/@CodyBrown/how-to-lose-8k-worth-of-bitcoin-in-15-minutes-with-verizon-and-coinbase-com-ba75fb8d0bac) additionally compromising your Lockbook.

You're not, however uncompromisable. There is a strong element of personal responsibility when it comes to security. If you're out there downloading random `cracked-photoshop.exe`s much of your protection goes out the window.

Security is a chain, and attackers will seek to exploit the weakest (cheapest) link in your setup.

# How do I use Lockbook Securely?

The design of Lockbook makes your content as secure as your devices are. Here are some very broad general recommendations for how I think about security.

## Mobile Devices

Mobile devices are generally inherently secure devices. Apps are running in a very limited execution environment and can't access each other's data. Use popular devices, keep them fresher for ~4 years, update your software, and use the security features your phone ships with (1111 is not a good passcode). If you do all this, you're at a pretty strong starting point.

As I mentioned above it takes a fair amount of sophistication for someone to break into an iPhone that's up to date. Your local police department can't do it, and probably not all of the 3 letter federal agencies can (but a few probably can). I think it's probably best to consider big tech and government synonymous from an adversarial perspective.

Google has some incredibly strong incentives to spy on you. They don't make the slightest effort to implement end-to-end encryption in their messengers and often they send computations to a server that Apple performs locally. Apple is incentivized to sell devices not ads, Apple has formed a reputation for simplicity and security.

Apple rules the app store supply chain with an iron fist which introduces attack vectors for supply chain attacks and censorship.

Both iOS and Android have open-source roots but neither of these platforms are practically open source. Both platforms have huge amounts of unremovable infrastructure that is closed source.

There are, however, secure phone implementations like Graphene, Librem, Liberty, and the Pinephone. The theme is generally: reasonably secure hardware and fully open-source software. Configured by default to make it easier to use securely and privately.

There is a school of thought that says phones are unsecurable. It's just too hard to have a cell plan that isn't coarsely tracking you with cell tower metadata. This way of thinking also considers phones to be toxic machines of addiction and control and recommends using dumb phones or no phones at all.

## Computers

The surface area of attack on computers is far broader and we depend on computers for essential activities.

The security situation on Windows is a bit of a disaster, many Windows computers come pre-packaged with a [large amount of bloatware](https://arstechnica.com/gaming/2015/05/humanity-weeps-as-candy-crush-saga-comes-pre-installed-with-windows-10/). This increases the surface area of attack for devices. Windows is a fragmented ecosystem so suggestions to "only install _sandboxed_ apps from the Microsoft Store" are effectively impractical. The normal way to install and update things is to _download an exe from a website_ , a pretty bad security default.

In my opinion, Macbooks are a step up in security from Windows. Things on macOS are closer to iPhones, by default, the installation comes with minimal bloat and the security defaults are reasonable. Apps have to ask permission to access various resources. Additionally, you can turn off some security features and download apps from arbitrary locations. However, both Windows and macOS are largely closed source leaving room for "bugs" and backdoors.

The next step up here would be to run an open-source Linux flavor on commonly available hardware (Thinkpads, Dell laptops, or a Desktop computer). "Distributions" like Ubuntu, Fedora, and Manjaro provide a gentle introduction to Linux. These days the transition may be easier than you'd expect as most computing can happen in the browser.

This is a pretty solid place to arrive. Most of what you're running is open source and inherently more secure. You'll likely install and update all your software through your distro's package manager, either graphically or through the command line.

From here our next stop is minimal, hand-crafted Linux distros. On distros like Arch, and Gentoo you gain an understanding of what all the moving pieces of your computer are, and you play an active role in their assembly. Once you're setup you have a strong understanding of everything that's running on your system. What's running on your system is likely just what you need, and nothing you don't. This means your surface area for attack is as small as possible. At this step, you're probably already familiar with command line interfaces, and can likely use the _Lockbook CLI_ rather than the desktop version -- further reducing the surface area of attack.

At this stage you may also consider exploring the BSD variants (historically more secure kernel than Linux), and specialized operating systems like QubesOS / Tails. You may also consider specialized hardware that seeks to remove eyebrow-raising hardware components like the [Intel Management Engine](https://www.youtube.com/watch?v=HNwWQ9zGT-8).

## Physical Security

It's also worth noting that there are very few software solutions to the wrench attack:

[![Security](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F50549fd7-8719-4ae1-a3fa-d4fd2bca53c8_448x274.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F50549fd7-8719-4ae1-a3fa-d4fd2bca53c8_448x274.png)

If your secrets are [valuable enough](https://github.com/jlopp/physical-bitcoin-attacks/blob/master/README.md) to guard, consider investing in some [real](https://en.wikipedia.org/wiki/Second_Amendment_to_the_United_States_Constitution) [physical security](https://blog.casa.io/a-home-defense-primer/). Understand that your adversaries may be immoral actors who will happily kidnap your family members to make you do what they want.

# Is Lockbook secure enough for me?

Lockbook is a young piece of software, and as such may contain bugs. We believe we're secure by design, we've designed everything around making sure that you'll never leak information because of a bug. Rather our worst bugs should be crashes, missing documents, or the inability to communicate with our server. We do believe we're in a fundamentally different category of security than products that aren't end-to-end encrypted and open source and you are better off using us compared to those solutions.

But if your life literally depends on the security of your technology there's no avoiding dramatically investing a thorough knowledge of security and evaluating your chosen solution (lockbook, signal, PGP, local-only solutions, literally not using technology) at a very deep and fundamental level.

# How do I learn more about security?

Here are some resources I've found valuable for better understanding the world of security:

  * [YT: MentalOutlaw](https://www.youtube.com/@MentalOutlaw)

    * OpSec news

    * Analysis of how various people have been compromised

    * Instruction for higher security setups

  * [Podcast: Darknet Diaries](https://open.spotify.com/show/4XPl3uEEL9hvqMkoZrzbx5)

    * Interviews of hackers, spies, and an exploration of hacking culture

  * [Blog: Jameson Lopp](https://www.lopp.net/articles.html)

    * Crypto-focused OpSec discussion

  * [Blog: bigtech.fail](https://bigtech.fail/)

    * "Shining a light on the censorship, propaganda, and mass surveillance from today's tech corporations and governments."

  * [YT: Low Level](https://www.youtube.com/@LowLevelLearning)

    * Technical analysis of high-profile security vulnerabilities



