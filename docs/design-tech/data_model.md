# Data Model and Procedures

Thinking and reasoning about things related to Lockbook requires understanding the following two structs and the associated operations we do on them. 

## [Accounts](https://github.com/lockbook/lockbook/blob/master/libs/lb/lb-rs/libs/shared/src/account.rs)

+ `username` - unique identifier for a user, `[a-z][0-9]`
+ `private_key` - RSA Keypair, generated by clients, the public component is uploaded to the server.

## [FileMetadata](https://github.com/lockbook/lockbook/blob/master/libs/lb/lb-rs/libs/shared/src/file_metadata.rs)

+ `id` - [Uuid](https://en.wikipedia.org/wiki/Universally_unique_identifier)
+ `file_type` - A `File` could represent a `Folder` (which contains more files) or a `Document`
+ `parent` - The `id` of the parent folder which this files resides inside.
+ `name`
+ `owner`
+ `signature` - Proof that the `owner` created this file.
+ `metadata_version` - A `u64` that represents this struct's version.
+ `content_version` - If the `file_type` is a `Document` this corresponds to that content's `version`. `id` also represents 
the Document's `id` if this is a `Document`. 
+ `user_access_keys` - The decryption key for this `File` encrypted by the `public_key` of a particular user.
+ `folder_access_keys` - The decryption key for this `File` encrypted by the encryption key of the `parent` of this 
folder.
+ `archived`
+ `deleted`

### Versions

Every change to a `File` increments its `metadata_version`. This ensures that if you `delete` a file, you have the most 
recent version of the file. The primary motivation is to prevent race conditions. The server is responsible for doing 
checks and incrementing versions.

Similarly, every change to a `Document`'s content increments its `content` version. The reason this is its own field is
to only download a file out of S3 (which is expensive) if we need to do so.

### Access Keys

When a client authors a file they generate a 256 bit `AES` key. Content is encrypted with this key. Each `File` gets 
its own key. To decrypt a file, you need the `access_key` of that file. These keys, however, are stored in an encrypted manner.

They are either encrypted for a particular user (with their public key) or with the access key for a folder.

This means you can give someone cryptographic access to a particular file or an entire folder. In most situations, folder
based decryption is how the `lb-rs` library decrypts the contents of files. Let's dive a bit deeper into how that works.

During account creation, every user generates a `root` folder (locally). An `access_key` -- `a` is generated and encrypted with
their public key -- `b = user_keys.encrypt(a)`. `b` is what is stored locally in `user_access_keys` and sent to the server. Direct descendants 
of `root` (`folder1` for example) will generate new `access_keys` for their files -- `c`. `c` will be encrypted with `a`
and stored in `folder_access_keys`. 

The procedure for decrypting a given file is:
```
1. Is there an entry with my username in the user_access_keys for this file? If so decrypt the key, use that key to 
decrypt the file. <Done>
2. Grab the decryption key for this file's parent. Decrypt the key with the parent's key, use that key to decrypt this 
file. <Done>
```

In this procedure, #2 is a recursive action. Decrypting a file often means walking through your folder structure to your 
root folder where you'll find the 1 key you can decrypt with your `private_key`.

The motivation for this design is the flexibility it gives us around sharing. You can share a file or folder directly with 
someone, without having to re-key one or many files. The proposed share procedure looks something like this:

```
1. Decrypt the key for this file, let's refer to this as *secret*. (See above)
2. Lookup their public key. Encrypt *secret* with their public_key.
3. Add an entry to user_access_keys with their username -> encrypted *secret*
```

Parallel to their root folder, in a `shared_with_me/` folder, this `File` will show up.

This works for `Document`s as well as `Folder`s. Consider the procedure for decrypting a given file that is shared with 
you and is a folder with hundreds of documents.

Other constraints apart from sharing flexibility for the design include:
1. Is trustless -- Lockbook server will never handle these secrets, all secrets stay in an encrypted form.
2. Is efficient -- Share a folder with hundreds of documents and sub-folders without having to perform more than 1 
cryptographic operation.

There is an implicit `name`->`public_key` resolution attack vector here -- Lockbook servers could lie about what a 
`username`'s `public_key` is. The only solution to this problem is out of band (ideally in person) verification of 
`public_key`s. Our process will likely look something [like Signal's][signal-link] except the `username -> public_key` 
relationship is immutable within Lockbook.

### Sync

When a client goes to sync for the first time (let's pretend their account has several files), they will `/get-updates` 
with an input value of `0`. This indicates they want to receive the record of everything that has happened. They'll 
receive a `Vec<FileMetadata>` and will retrieve the corresponding files out of `S3`.

As someone continues using both devices, online and offline, the sync process becomes a bit more complex.

Clients keep track of what has changed locally. These changes look something like:
+ `Rename(old, new)`
+ `Edit(old_checksum, new_checksum)`
+ Etc

When they call `get-updates`, they pass the largest `metadata_version` (see [sync.md](sync.md) for more details) they have. Any files they don't know about (are
new) are resolved trivially, and any local changes are pushed up. For files that have changed both remotely and
locally, an attempt to merge the changes is made. If you moved a file remotely and renamed it locally, there is no
conflict. If you have local edits and remote edits, they are merged `git` style. In cases where there's a truly
unresolvable conflict (remote and local both rename a file), the server wins.

All edits are cryptographically signed, and clients verify that they are genuine before altering their local state. 
[No one should ever be able to impersonate a user of Lockbook.](https://en.wikipedia.org/wiki/2020_Twitter_bitcoin_scam)

Specifics of how sync handles certain situations can be found in [sync.md](sync.md)

#### Archived Files

Archived files provide the user an opportunity to clean up their Lockbook directory without permanently deleting 
information.

Most clients will elect to "hide" `archived` files and delete `deleted` files.

Clients may also treat archived files specially during search or sync operations.

Archive & Delete are the only operations that do not require any metadata version to be passed before being synced to
other devices.

Archive does not require a metadata version check because it's inherently a safe operation and there's no other operation
that it could conflict with.

#### Deleted Files

Delete is the only "unsafe operation" since you can delete a file without having the most recent version of that file. 
Archiving a file is the "safe" version of deleting a file. 

[signal-link]: https://support.signal.org/hc/en-us/articles/360007060632-What-is-a-safety-number-and-why-do-I-see-that-it-changed-
[sync-service]: https://github.com/lockbook/lockbook/blob/master/libs/lb/lb-rs/src/service/sync_service.rs
