+++
title = "Migrating Lockbook to a Better Key Format"
date = 2024-09-09
[extra]
author = "smail"
author_link = "https://barkouch.me/"
+++


On Lockbook, creating an account is simple. You choose a username, and Lockbook will generate an account key. This is what it looks like:
    
    
    CwAAAAAAAABzbWFpbHRlc3RzMR0AAAAAAAAAaHR0cHM6Ly9hcGkucHJvZC5sb2NrYm9vay5uZXQgAAAAAAAAAJ/1ORmw56YptpNdQvJmGNsE1Lh4qpyYxRl6pp5dE7z0

This key is the only thing you need to access your files on your device. You don't even need to remember your username. And besides, creating your own password is always less secure. Most people reuse passwords from other accounts or use weak passwords. But with a generated account key, Lockbook keeps your notes safe and secure against attacks like [credential stuffing](https://en.wikipedia.org/wiki/Credential_stuffing) and [dictionary attacks](https://en.wikipedia.org/wiki/Dictionary_attack).

The account key is a critical part of our [end-to-end encryption scheme](https://en.wikipedia.org/wiki/End-to-end_encryption). This ensures that no one besides you can see your notes, not even us. This account key corresponds to a private key [from the ](https://en.bitcoin.it/wiki/Secp256k1)`secp256k1` encryption standard. A user's files are encrypted hierarchically [using ](https://en.wikipedia.org/wiki/Advanced_Encryption_Standard)`AES`, another encryption standard, and the uppermost key is encrypted using the public key corresponding to your private key. This is called hybrid encryption, and it combines the benefits of both symmetric and asymmetric encryption capabilities. You can read more about it [here](https://en.wikipedia.org/wiki/Hybrid_cryptosystem).

Our current account key has some issues though. It contains unnecessary information, like what server your Lockbook communicates to, and the username. All of which is [Base64 encoded](https://en.wikipedia.org/wiki/Base64), resulting in a combination of random characters. This isn't necessarily bad, but for someone who might want to write their account key down, it is difficult and error prone. It reminds me of an xkcd comic:

[![](https://substackcdn.com/image/fetch/w_1456,c_limit,f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F50192293-c310-4ef7-9963-928b3b032e80_1480x1202.png)](https://substackcdn.com/image/fetch/f_auto,q_auto:good,fl_progressive:steep/https%3A%2F%2Fsubstack-post-media.s3.amazonaws.com%2Fpublic%2Fimages%2F50192293-c310-4ef7-9963-928b3b032e80_1480x1202.png)

## Passphrases

To solve this issue we took some inspiration from Bitcoin since they use the same private keys. [BIP (Bitcoin Improvement Proposal) 39](https://en.bitcoin.it/wiki/BIP_0039) described a way to turn bytes into a mnemonic phrase. Every combination of 11 bits corresponded to a unique word. The motivation was that words are harder to input incorrectly than random characters, and this is exactly what we need. In addition, these words were specifically chosen to be simple. Here's what it looks like:
    
    
    turkey, era, velvet, detail, prison, income, dose, royal, fever, truly, unique, couple, party, example, piece, art, leaf, follow, rose, access, vacant, gather, wasp, audit

Another aspect of this proposal was the inclusion of a checksum, which added a self-verifying mechanism to ensure that a phrase has been entered correctly. Since Lockbook uses 256-bit private keys with an additional 4 bits for the checksum, the total key length is 260 bits. This corresponds to a word count of 24 words. This is great and solves the writing issue we discussed before.

But sometimes users still want a compact key; like for a QR code or for a password manager. Our previous key had its advantages, and we can reduce its size by removing the username and server URL. This effectively halves the number of characters. It looks something like this:
    
    
    nvo7SItwXYmoxxzmOCUrNJiw85V8CwJ+SXb8cOHPIlo=

So, when you enter your settings, you have two choices. You can export your phrase or a compact version of your account key. Either will work across all your devices.

## Migration

Changing private keys is hard. If we just updated our code to use the new keys in the next release, people would have issues logging in. Additionally, some users may have the old account key saved in password managers, and it would be unacceptable if users couldn't sign in after returning to Lockbook.

As a result, because we take breaking changes very seriously, we decided to support the previous key format indefinitely. Once most users are on the new version, we will switch to using the new key format. Minimizing the risk of someone using a new key with an incompatible app. We'll also communicate this change across all our social media platforms.

## Coding

The majority of the business logic in our apps is written in Rust. This is thanks to `lb-rs`, [a shared library we use](https://blog.lockbook.net/cp/136569912) to iterate quickly across our platforms. This is where I wrote the new key logic. I started by importing a [library called ](https://github.com/vincenthz/bip39-dict/)`bip39-dict`, which provided the binary-to-word translations. In BIP 39, every 11 bits corresponded to a word, so I had to split up the `Vec<u8>` private key into 11-bit chunks. The implementation was simple. After implementing a method to convert the private key into a phrase, I made a method to turn the phrase into a private key. This simply involved turning each word it into a `u16`, and converting chunks of `u16`s (considering only the first 11 bits) into `u8`s. I added some tests to verify the inverse properties of these two functions. More details on my efforts can be found [here](https://github.com/lockbook/lockbook/pull/2811).

## Final Thoughts

With these updates in place, we're excited to continue improving Lockbook. As a unique open-source project, Lockbook has a myriad of interesting problems. If you're interested in contributing, feel free to check out the [GitHub](https://github.com/lockbook/lockbook/) and join our [Discord](https://discord.com/invite/lockbook). We would love to have you.
