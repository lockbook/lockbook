# Sync v3

*this document is slightly out of date, it doesn't contain documentation on work
like `safe_write` which enables near-real-time collaboration. It will be updated
soon.*

## Overview
Sync v3 is the latest iteration of our logic that syncs users' file trees across
devices. It is designed under the following key constraints:
* The state of a user's file tree must satisfy certain invariants at all times,
  on all devices (including the server), even in the event of process
  interruptions and failures.
* Users' file contents and names must not be readable by the server, but users
  can share files and folders with other users.
* Users must be able to view and edit files offline, then sync frictionlessly,
  even with intervening edits made to files on other devices.
* The performance of the system must not suffer as a result of typical usage,
  which presents a number of challenges worth discussion

These design constraints motivate the following high-level design, which will be
flushed out in more detail after a further discussion of the constraints:
* The client maintains two versions of the file tree: one for the current local
  version (`local`), and one for the last version known to be agreed upon by the
  client and server (`base`).
* The server maintains one version of each user's file tree (`remote`)
* To perform a sync, the client pulls `remote` from the server, 3-way merges the
  changes with `local` and `base`, and pushes those updates to the server.

## Constraints
### File Tree Invariants
Before discussing the invariants, it is helpful to understand the data model.
For the purposes of Sync v3, each file has:
* `id`: an immutable unique identifier
* `file_type`: an immutable flag that indicates if the file is a folder or
  document. A _file_ is either a _document_, which has contents, or a _folder_,
  which can have files as children.
* `name`: the file's name. Clients encrypt and HMAC names before sending them to
  the server, so while the server cannot read file names, it can test them for
  equality.
* `parent`: the file's parent folder (specified by the parent's `id`). The
  `root` folder is the folder which has itself as a parent (it's name is by
  convention it's owners username and it cannot be deleted).
* `deleted`: a boolean flag which indicates whether the file is _explicitly_
  deleted. A file whose ancestor is deleted need not be explictly deleted; if
  not, it is considered _implictly_ deleted (all implicitly deleted files are
  eventually explicitly deleted). A file that is no longer known to a client at
  all is considered _pruned_ on that client.

This data model lends itself to four operations, which (aside from `create`)
correspond to modifications to the three mutable fields of a file:
* `create`: create a file
* `rename`: change the file's name
* `move`: change the file's parent
* `delete`: delete the file (cannot be undone)

With that data model in mind, these are the four main invariants that must be
true of a user's file tree on all devices at all times:
* Root Invariant: there must always be exactly one `root` and all modifications
  to `root`s are invalid. This is straightforward to enforce.
* Path Conflict Invariant: no two files may have the same name and parent. An
  example challenge is when a user creates a new file on each of two clients
  with the same name and parent, then syncs them both.
* Cycle Invariant: every non-root file must not have itself as an ancestor. An
  example challenge is when on one client the user moves folder A into folder B
  and on the other client moves folder B into folder A, then syncs them both.
* Orphan Invariant: every non-root file must have its parent in the file tree.
  To keep client storage from growing unboundedly, clients eventually prune
  deleted files (remove all mentions of them from durable storage). If the
  design does not pay careful attention to the disctinctions between explictly
  deleted, implicitly deleted, and pruned files, a client may prune a file
  without pruning one of its children.

### Privacy
Files, file names, and encryption keys never leave the client unencrypted. In
order to support [sharing](sharing.md), we give every document a symmetric key
(used to encrypt its contents) and every folder a symmetric key (used to encrypt
the keys of child folders and documents). This allows users to recursively share
folder contents performantly and consistently. It also creates an encryption
chain for each document: the document is encrypted with the document's key,
which is encrypted with its parent folder's key, which is encrypted with it's
parent folders key, all the way up to the root folder whose key is encrypted
with the user's account key which is stored on the user's devices and trasferred
directly between them as a string or QR code.

This encryption chain, and its interaction with the invariants, creates
programming challenges. Our general pattern is to decrypt keys, names, and files
as we read them from disk or receive them as responses from the server so that
we can write all the remaining logic without worrying about encryption. However,
most invariant violations (e.g. orphans and cycles) will cause us to fail to be
able to decrypt files, which in turn makes it difficult to detect and
appropriately resolve invariant violations.

### Offline Editing
The key challenge in offline editing is what to do when concurrent updates are
made to files. As previously mentioned, the client maintains two versions of the
file tree: one for the current local version (`local`), and one for the last
version known to be agreed upon by the client and server (`base`). Our ambition
is to define a 3-way merge operation for file trees which we can use after
pulling the server's version of the file tree (`remote`) which never results in
a invariant violation and which otherwise produces satisfying behavior to users.
This operation needs to take place on clients because the server does not have
access to decrypted files. Once a client uses the 3-way merge operation to
resolve conflicts, they can write their updates back to the server.

For file contents, we can use a standard 3-way file merge operation like the one
used by Git. If there are edit conflicts, we resolve them by leaving in both
versions along with Git-style annotations of which change came from `local` vs
`remote`. When a 3-way merge operation doesn't make sense (e.g. for binary
files), we can resolve edit conflicts by saving both copies of the file, with
one of the files renamed.

For file metadata, we prefer the remote version of a file's `name` and `parent`
because we think it makes the most sense for the client whose updates reached
the server first to have its changes preferred. If either the server or the
client deleted a file and the other did not, we resolve the conflict by deleting
the file so that file deletions are permanent. File metadata also contain other
fields (discussed in the next section) which require careful consideration
during a merge.

As mentioned, some invariant violations arise from concurrent multi-device
edits, even if the individual devices maintain those invariants locally.
Therefore the 3-way merge operation requires additional logic to resolve these
constraints in a user-friendly way.

### Performance
There are a handful of performance pitfalls which we need to avoid. We pay
attention to the time it takes to perform certain operations and the storage
space used on users' devices.

When pulling the state of a user's file tree from the server, we need to avoid
pulling the whole file tree. We intend for the system to scale to thousands or
millions of files per user - the user might have access to a shared folder
containing files for their entire company. To this end we track a version for
the file in its metdata (`metadata_version`). The `metadata_version` for a file
is simply the server-assigned timestamp of the file's most recent update,
represented as a Unix epoch. When a client fetches updates from the server, it
passes the most recent version it knowns of, and the server replies with the new
metadata for all files that have been updated since that point. This way, the
volume of data pulled scales with the velocity of updates to the user's file
tree rather than the size of the file tree.

Pulling the metadata for a file and pulling the content for a file are, in terms
of performance, categorically different. A file's contents are generally many
orders of magnitude larger than file metadata. When we pull an update for a
file, we need to avoid re-pulling the content for the file if it hasn't changed
(e.g. if the file was only renamed or moved since the client last checked with
the server). To this end, alongside `metadata_version`, we store a
`content_version` in each metadata which is a server-assigned timestamp of the
file's most recent content update. When a client pulls a metadata update, it
also pulls the file content if and only if the `content_version` has changed
since it last pulled.

Clients store two versions of the file tree, `base` and `local`. In order to
save space, we use semantics similar to copy-on-write where the version of a
file on `local` is not saved if it is identical to the version on `base`.
Clients interpret the absence of a `local` version to mean that `local` and
`base` are identical. Additionally, we compress files, both before saving to
disk and before pushing to the server.

The metadata for deleted files must be eventually pruned on clients so that the
amount of space taken on users' devices scales with the number of not-deleted
files rather than the total number of files ever created. Deleted files cannot
be immediately pruned because the client needs to remember to push the deletion
to the server on the next sync. The server keeps track of all file metadata
forever (though it deletes contents of deleted files) so that clients can sync
after being offline indefinitely and still receive deletion events without the
complexity of the server needing to know which clients exist and which updates
they have each received.

## Design By Concern
Sync v3 operates in the context of many concerns. We'll discuss the design first
in terms of how the concerns are addressed one concern at a time, then in terms
of the client and server routines that address all these concerns all together.
These are the concerns we'll discuss:
* file deletion
* invariant violation resolution
* consistency

### File Deletion
As mentioned, files have various states of deletion:
* not deleted (the file has `false` value for `deleted` and no deleted
  ancestors)
* explicitly deleted (the file has `true` value for `deleted`)
* implicitly deleted (the file has `false` value for `deleted` but has an
  explicitly deleted ancestor)
* pruned (there is no mention of file on disk, although it once existed)

First we'll justify the distinction between deleted and pruned. If the file
deletion system is working properly, then after a user deletes a file on a
device and syncs that device, that device will prune the file and any other
devices that sync will prune that file. What's required to make that happen is
that **the client which deletes the file remembers that the file exists (and is
deleted) until that client syncs its changes.** Once the changes are synced, the
client can prune the file knowing that the server will remember the deletion
forever and will inform the other clients of the deletion. Note that deleted
documents can have their contents pruned immediately - it is only important to
remember the metadata until the deletion is pushed.

Next we'll justify the the distinction between implicit and explicit deletions.
What it would mean to not have this distinction is that a file would only be
deleted if it is marked explicitly deleted, so in order to delete a folder
either the user would have to delete all its children first or the client would
have to automatically delete all its children first e.g. recursively delete
folders (otherwise those children would become orphans, violating the Orphan
Invariant). This would be undesirable because of the behavior of folder
deletions in the presence of multiple clients. Consider the case when one client
moves a document out of a folder and syncs, then another client deletes the
folder and syncs. If a client applied explicit deletions recursively, the second
client would delete both the folder and the document, then the next time the
other client synced, the document would be deleted even though it was not in the
deleted folder. We decided this was unacceptable behavior.

In order to prune a file, we need it to be explictly deleted (and have no
descendants that are not explictly deleted and would therefore become orphans),
and for its explicit deletion to be synced to the server. If it's only
implicitly deleted, then it potentially could have been moved out of its deleted
ancestor in an update that we have not yet pulled, so a client cannot yet prune
it. This will perpetually be the case for implicitly deleted files unless they
are at some point explictly deleted. This means the server is the entity which
must ultimately explicitly delete implicitly deleted files. **Anytime a set of
file updates is pushed to the server, the server explicitly deletes all
implicitly deleted files. The next time they pull, clients receive these
deletions as updates, which indicates that it is safe for them to prune the
files.**

Revisiting the earlier example, when a client moves a document out of a folder
and syncs, the server becomes aware of that move. Then when another client
deletes the folder and syncs, the server marks all implicitly deleted files as
explicitly deleted. This does not include the document because the server is
already aware that it has been moved. If the first client hadn't synced until
after the second client, then the document would have been deleted. This is one
of the ways in which we resolve conflicts in favor of the first client to sync.

_Note: when a document is implicitly deleted on a device, the device can prune
the documents' contents and just keep the metadata which is much smaller. If,
after syncing, the document is no longer implicitly deleted, the device can
re-pull the contents from the server._

### Invariant Violation Resolution
Even if clients individually enfoce invariants, violations can still occur in
the presence of concurrent updates. This is a complicated matter.

First, we have to be concerned about the Cycle Invariant. A cycle can occur if a
user has two folders, moves the first folder into the second on one client and
moves the second folder into the first on another, then syncs them both. A cycle
can also involve more folders, if a first folder is moved into a subfolder of a
third folder and the third folder is moved into the first folder concurrently.

We also have to be concerned about the Path Conflict Invariant. A path conflict
can occur if two clients create files with the same name in the same folder and
then sync. Occurrences can also involve a client renaming an existing file in
that folder, moving a file into a folder, or after a file edit conflict is
resolved by saving a copy of the local version with a new name. Additionally, it
must be valid for two files to share a name and parent if one or both of them
are deleted.

Finally, we have to be concerned about the Orphan Invariant. An orphan can occur
if a client deletes a folder and syncs, then another client moves a file into
that folder and syncs, then the initial client syncs again. The initial client
will prune the folder after pushing its deletion, then the second client will
push the move and the server will mark it as deleted, then the initial client
will receive an update to a file for which it does not have the parent. In this
situation the client needs to resolve the invariant violation without decrypting
the update because the encryption key to decrypt that update lives in the
folder's metadata which was already pruned.

There are enough corner cases here to drown in. To handle them, we implement a
design where, **rather than prevent invariant violations from occurring, the
system repairs invariant violations that result from certain combinations of
edits from different devices.** It does this as part of the sync routine. We
found that there's generally a satisfactory way to do this.

To resolve cycles, we rely on the fact that `base`, `local`, and `remote` all
individually do not contain any cycles and design the resolution to preserve
that. If the result of merging the file trees has a cycle, it must involve an
unsynced local move and a remote move the client just pulled from the server. In
particular, one of the files in the cycle must have been the subject of an
unsynced local move. Our policy to resolve this is, **if we discover locally
moved files which create a cycle with other files, we unmove all the locally
moved files** (by setting the parents to their values in `base`).

To resolve path conflicts, we rely on the fact that `base`, `local`, and
`remote` all individually do not contain path conflicts, and therefore that one
of the files involved in the conflict was somehow modified locally. Our policy
to resolve this is, **if we discover a locally modified file whose new name
conflicts with another file in the same folder, we rename the locally modified
file by appending a number to the pre-extention file name.** For example, if a
folder contains a new file called `notes.txt` from the server and a new local
file called `notes.txt`, we rename the local file `notes-1.txt`. If there is
already a `notes-1.txt`, we instead rename it `notes-2.txt`.

To resolve orphaned files, we rely on the fact that an update for a file will
never be pulled without the parent folder being pulled first. If the parent
folder no longer exists locally, it's because the parent folder has been pruned,
which only happens if the deletion of that folder has been synced. If the folder
deletion has been synced, then the server has explicitly deleted all descendants
of the folder, including the orphans. Because deletions can never be undone,
**any updates to orphaned files can be safely ignored.**

I know what you're wondering - does the order of these conflict resolutions
matter? Yes it does. Any content conflicts need to be resolved before path
conflicts because resolving content conflicts can create new files which can
cause path conflicts. Any cycles need to be resolved before path conflicts
because resolving cycles can move files which can cause path conflicts. Updates
to orphaned files can be ignored at any time because no resolution policy
depends on or modifies orphaned files.

### Consistency
A great amount of care needs to go into making sure that the invariants are
satisfied at all times. The system needs to recover from process interruptions
and failures gracefully. We'll inspect the following consistency-related
concerns:
* batch operations
* concurrent syncs
* open editors during a sync
* client crash/interrupt recovery
* server crash/interrupt recovery

#### Batch Operations
A key element of the design is that **metadata operations are processed in
batches. What this means is that all the changes that are applied to a file tree
- moves, renames, and deletes - are applied with all the invariants evaluated at
the end, rather than evaluated after each operation.** For instance, the server
has a single endpoint for uploading a set of changes which are applied
atomically, with invariants checked after applying all of them rather than being
checked in-between each one. In clients, all new updates from the server are
applied before checking invariants and resolving conflicts. To see why this
helps, let's consider what our design would look like otherwise.

Consider the case where a user moves a file out of a folder, then creates a new
file with the same name in that folder. If these changes are not processed
together, then they need to be processed in the order in which they happened,
otherwise the new file could be created first which would violate the Path
Conflict invariant. Therefore, clients would need to be deliberate about the
order in which changes are uploaded to the server. Rather than track the `base`
and `local` states of the file tree, clients would need to track either `base`
or `local` and an ordered collection of the changes made (henceforth the
_changeset_) that turned `base` into `local`. This would allow the client to
reproduce `base` and `local` for the sake of 3-way merging; the client could
produce `local` from `base` and the changeset or could produce `base` from
`local` and the changeset. The client could also still explicitly store `base`
and `local` but care would need to be taken to keep them consistent with the
changeset.

The changeset cannot be append-only. If a user renames a file, then renames it
back to the original name without syncing, the changeset should be empty. This
is because the app should not incur the performance cost of syncing an
unnecessary pair of changes, as well as because the app should not inidicate to
the user that they have outstanding changes to be synced when they do not. In
this situation, the combination of renames should cancel out, with both
operations being removed from the changeset. However, this can cause problems.
Consider the case when the user renames file A, creates a file B in the same
folder with the original name, moves file B to a different folder, then renames
file A back to its original name. If the client simply removes both renames,
then the changeset includes just creating and moving file B. In-between creating
and moving file B, file B is still in the same folder as file A with the same
name, which violates the Path Conflict invariant. While we could develop a
system which cleverly maintains the changeset so that the file tree upholds the
invariants in-between each change, it's easier to process operations in batches.

#### Concurrent Syncs
Sync is not an atomic operation; it consists of multiple steps. For the purposes
of this section, it consists of the following 5 steps (the sync routine is
discussed in more detail later):
* _pull all updates 1_: The client pulls the latest updates to metadata and
  document content. This is generally when metadata and documents are 3-way
  merged as clients pull updates made by other clients.
* _push metadata updates_: The client pushes metadata updates, which are the
  local changes that have been made on this client since the last sync. They
  have been merged with the latest server changes during the previous pull.
* _pull all updates 2_: The client pulls the latest updates again. Generally
  there have been no updates made by other clients and this step just pulls the
  latest server-assigned `metadata_version` as well as the deletions that the
  server made after finding non-deleted files in deleted folders in the previous
  push.
* _push content updates_: The client pushes content updates, which generally
  resulted from the merge during the initial pull. This is done after the
  previous step so that we do not push updates to deleted files for performance.
* _pull all updates 3_: The client pulls the latest updates again. Generally
  there have been no updates made by other clients and this step just pulls the
  latest server-assigned `metadata_version` and `content_version` for each file
  after the updates from the previous push.

Problems can happen when two devices sync concurrently. In particular, problems
can happen when one device pushes an update while another device is between the
first and second pulls or between the second and third pulls. Consider the case
where a user moves a file on device A, renames it on device B, then syncs the
devices concurrently such that devices A and B both pull updates, then A pushes
metadata updates, then B pushes metadata updates. When device B pulls, the
version it pulls will not be affected by the move done on device A because
device A has not pushed yet. When device A pushes, it will push a version
affected by the move, then when device B pushes, it will push a version affected
by the rename but not the move. When A pulls again, it will pull the version
affected by the rename but not the move. Effectively, the move will be undone.

To solve this, we implement a basic precondition system. **When clients push
metadata updates, they are required to pass the current name and parent of the
file. If those values have changed since the client last pulled, the client is
considered to be pushing changes to an out-of-date version of the file, and the
server rejects the updates. Similarly, when clients push document content
updates, they are required to pass the current `content_version` of the
document, and the server rejects the update if this is incorrect.** When the
server rejects the update, sync fails and must be retried, which can happen
automatically or be requested by the user. The first step of the next sync pulls
whatever updates happened concurrently to block the previous sync and merges
those changes in with the local changes before attempting to push again.

A number of alternate preconditions are valid, with each creating a different
experience for the user. We chose not to force clients to have the most
up-to-date version of the `deleted` field, instead allowing updates to deleted
files, because these operations would make no effect and there is no need to
force a new sync. We also chose not to force clients to have the most up-to-date
version of the file's contents to update the metadata or to have the most
up-to-date version of the file's metadata to update the contents. We made these
design choices to balance a strong level of consistency with a lightweight sync
process in the event of concurrent syncs.

#### Open Editors During A Sync
Care must be taken to avoid losing a user's changes to a document when they are
editing the document while a sync occurs. Consider the case where a user opens a
document for editing on device A, then edits and syncs the same document on
device B, then syncs device A while the document is still open for editing. The
sync will overwrite the local version of the document with the 3-way merge of
the document, but when the user saves the document, the 3-way merge result will
be overwritten by the newly edited version, which does not include the changes
made on device B. When the user syncs, the new state of the document will be the
newly edited version, and other devices that sync will have their versions
overwritten to this version i.e. the changes made on device B will be lost
forever.

**The simplest way to avoid losing changes to document contents that are pulled
while a user has an open editor is to lock the editor, which is not ideal.** The
client should save the document, lock the editor, perform the sync, then re-open
the document and unlock the editor. The editor will be updated with content that
is the 3-way merge of the pulled change with any local changes, including the
changes just made in the editor, and the user can safely resume work.

**An alternative design is for the client to hold in-memory the version of the
document contents that were read from disk alongside the version the user is
editing. When the user saves, read the most up-to-date version of the document
from disk (including potential changes pulled during a sync), 3-way merge the
up-to-date disk version, editor version, and the inital disk version (using the
initial disk version as the base), update the editor to contain the result of
the merge, then save the result of the merge.** While this complicates the
implementation, the changes pulled by any syncs are incorporated both into the
open editor and into the version of the document that is saved to disk (and
eventually pushed) without locking the UI during a sync.

If desired, a similar process can be used **to update the editor after a sync:
when the user syncs, read the most up-to-date version of the document from disk
(including potential changes pulled during a sync), 3-way merge the up-to-date
disk version, editor version, and the inital disk version (using the initial
disk version as the base), then update the editor to contain the result of the
merge.**

#### Client Recovery
The three key sync-related things that the client stores on disk are file
metadata, file contents, and the last synced time. If these are managed
properly, then clients are resilient to interruptions. Ultimately, **if updates
to data on disk are atomic (e.g. by using an embedded database with transaction
support), client recovery is not difficult.** Let's look at some of the problems
we would encounter otherwise.

During a sync, clients check in with the server to pull all updates pushed by
other clients since this client last synced. It's okay for a client to pull
updates it's already received because updates are idempotent. The server doesn't
store and serve every update; it only stores and serves the most recent versions
of files. This means that clients are pulling only the latest state of files and
it is a no-op to overwrite the latest state of a file with the latest state of
that file. While it's okay to receive updates twice, it's not okay for clients
to miss updates. Users expect to see the most up-to-date versions of their file
trees after syncing. Beyond that, the server checks that clients have a
reasonably up-to-date version of the metadata being changed, which prevents
updates to out-of-date files in certain cases.

If the metadata updates during a sync are not saved atomically with the update
to the last synced time, the update to the last synced time must be saved after
the metadata updates. If it's saved first and then the process is interrupted
before the metadata updates are saved, then on the next sync, the client will
pass a last synced time greater than the `metadata_version` of any of those
files and will not receive updates to those files (until new updates to the
files are pushed to the server). The user will be told that their files are
up-to-date even though they aren't, and the server will sometimes consistently
reject updates to the out-of-date files. If the metadata updates are synced
first and the process is interrupted before the new last synced time is saved,
then on the next sync, the client will pass a lasy synced time less than the
`metadata_version` of those files and receive the lastest states of them again,
which is okay.

We also have to be concerned with synchronizing file metadata and contents. In
our implementation we separate the storage of metadata from contents because
many operations, like listing all files, can be performed without reading
document contents from disk which is more expensive.

If the content updates during a sync are not saved atomically with the metadata
updates, the metadata updates must be saved after the document updates. Recall
that when a client pulls updates from the server, it pulls metadata updates, and
if a metadata update includes a new `content_version` then it additionally pulls
content updates on a file-by-file basis. If the client saves metadata first,
then is interrupted before the content updates are saved, then the client will
think that the file content is up-to-date when it is not. The user will see an
out-of-date version of the files contents, and if they edit it, the client will
sync the edits, overwriting the unsaved changes. If the client saves the
document contents first, then is interrupted before the metadata updates are
saved, then the client will think that the file content is out-of-date when it
is not. The user will see that there is an unsynced change even though there
isn't, which isn't ideal, but it will go away after this next sync. During the
next sync the client will pull the new metadata again, pull the same document
content again because the metadata indicates that it has changed, and save the
same document content as was already saved. If there has been a local or remote
edit to the document in the meantime, the two versions of the documents will be
3-way merged.

These concerns are managable (though they complicate the implementation), but
further concerns would need to be raised and addressed about the ordering of
saves to `base` vs `local` versions of metadata and documents. Overall, atomic
writes solve this problem best.

_Concern: we don't currently have atomic writes._

#### Server Recovery
While client recovery is well-served by atomic writes, on the server atomic
writes are not possible. This is because document contents and metadata are
stored on different databases for performance and cost. All the metadata is
stored in one database which supports transactions and all the document contents
are stored in a key-value object storage database like S3. The server needs to
decide how to save metadata and contents to make sure it can always recover
without adverse effects.

_Note: server interruptions only happen in the infrequent situations of
infrastructure outages, bugs, and planned maintenance. It is acceptable for
human intervention to be required, but it is not acceptable for cleanup &
reconciliation to be impossible._

If the server saves the metadata for a document, then is interrupted before
saving the content, then it will serve the old file contents. If the server
saves the content for a document, then is interrupted before saving the
metadata, then it will not report the change to the document when clients pull
updates. Because of this, there is not simply an order of saves that will keep
the server recoverable at all times; something more clever is required.

**The server creates and deletes objects rather than mutating them because we
need multiple versions to exist to ensure recoverability.** To differentiate
versions, and to ensure that we are reading document contents which are
consistent with the `content_version` of its metadata, we use the `id` and
`content_version` as the key for the contents in object storage. When clients
request document content, they specify a `content_version`.

**To atomically save metadata and content, the server first saves the new
version of the content, then saves the metadata, then deletes the old version of
the content.** Each of these operations is atomic. If the server is interrupted
after only saving the new content version, the new metadata is never saved with
the new content version, so clients never request it and the contents are
effectively not updated. If the server is interrupted after the new metadata is
saved, clients receieve the new metadata with the new content version and the
content is effectively updated, but the old version of the file is never cleaned
up from object storage. If the update is a deletion, the first step is skipped,
but the rest works the same way. The server saves the metadata, then deletes the
old version of the content. If the server is interrupted after only saving the
metadata, clients will receive the deletion and prune the file when they pull
updates, but like with other edits, the old version of the file is never cleaned
up from object storage. So, **under this design, the system operates safely but
will sometimes leave unnecessary objects in storage.** The last piece of the
server recovery design is to figure out how to remove these old content versions
from object storage in the event of an interruption. Note that this process is
not urgent; it only affects infrastructure costs to the maintainers to have
unnecessary objects in storage.

**If the maintainer is willing to take server downtime, object storage can be
cleaned after a server interruption by deleting from object storage all the
objects whose `id`, `content_version` pairs no longer exist for a non-deleted
file in metadata storage.** The objects that will be deleted are the ones that
have been superceded by new versions or that are the contents of deleted files.

If the maintainer is not willing to take server downtime, they need to be
careful of ongoing server operations. If they follow the previous process,
deleting from object storage all the objects whose `id`, `content_version` pairs
no longer exist for a non-deleted file in metadata storage, they may delete an
object which is the new version of a document currently being updated. If they
do, the server will still save the new content version to metadata storage, and
clients will request the new version from the server and not find it. The
maintainer can tell which files are part of ongoing transactions because the
version that no longer exists for a non-deleted file in metadata storage will
have a more recent content version than another object that does exist
non-deleted in metadata storage. Both files should be left in place, as the
server will clean up the remaining file after finishing the ongoing update (or
the maintainer will clean up the file in a subsequent cleaning). Therefore, if
the maintainer is not willing to take server downtime, object storage can be
cleaned after a server interruption by deleting from object storage all the
objects whose `id`, `content_version` pairs no longer exist for a non-deleted
file in metadata storage and for which there is not another object with the same
`id` and a more recent `content_version` which does exist for a non-deleted file
in metadata storage.

However, this is tricky, as one cannot atomically query the state of metadata
and object storage (committing the object cleanup can happen separately because
once an object is safe to delete, it is forever safe to delete). **Further
investigation is required to understand whether there is an appropriate way to
perform object cleanup in a safe way with zero downtime.** For now, downtime is
recommended.

## Design Routines
With the designs around some of our concerns worked out, we now turn to the
specific routines involved in Sync v3 to see what each routine must do to
address all concerns. We'll examine the following routines:
- local metadata edits
- local content edits
- push metadata to server (`UpsertFileMetadata` endpoint)
- push file content to server (`ChangeDocumentContent` endpoint)
- pull metadata updates from server (`GetUpdates` endpoint)
- pull document contents from server (`GetDocument` endpoint)
- sync

### Local Metadata Edits
When a user creates, renames, moves, or deletes a file, the client validates the
operation, then updates the `local` version of the file tree to reflect the
operation.

For `create`, the client validates that:
- the file is not its own parent
- the file's name conforms to some rules (e.g. it cannot be empty or contain a
  slash)
- the file's parent exists and is a folder
- there are no files of the same name in its parent folder

For `rename`, the client validates that:
- the file is not the root folder
- the file's new name conforms to some rules (e.g. it cannot be empty or contain
  a slash)
- there are no files of the same name in its parent folder

For `move`, the client validates that:
- the file's new parent is not itself
- the file's parent exists and is a folder
- there are no files of the same name in its parent folder
- the file is not being moved into one of its subfolders

For `delete`, the client validates that:
- the file is not the root folder

_Concerns: `create` unnecessarily checks for invalid cycles_

### Local Content Edits
When a user modifies a document's contents, the client checks that the file
exists and is not deleted, then updates the `local` version of the file's
contents.

### Upsert File Metadata
The server exposes an endpoint, `UpsertFileMetadata`, which accepts a batch of
file metadata updates. The file metadata updates are just a file metadata, as
well as the name and parent that the client expects the server to have. The
endpoint checks the precondition that the existing name and parent are what the
client expects - otherwise the server responds with `GetUpdatesRequired`
indicating that the client should pull the latest changes and try again. The
server also checks for cycles, path conflicts, root modifications, new roots,
and orphaned files - if any of these checks fail, the update is rejected and the
server returns `GetUpdatesRequired`.

If the checks pass, the endpoint inserts or overwrites the metadata for each
file with the metadata supplied in the request. It sets the `metadata_version`
to the current timestamp. It explicitly deletes all files with deleted
ancestors. Then, it removes any newly deleted files from document storage.

### Change Document Content
The server exposes an endpoint, `ChangeDocumentContent`, which accepts the `id`
of a document to modify, the expected `metadata_version`, and the new contents
of the document. The server checks that the document exists and that the
`metadata_version` matches (otherwise it responds with `GetUpdatesRequired`),
writes the new version of the document to document storage, increments the
`metadata_version` and `content_version` for the document, and deletes the old
version of the document from document storage. The endpoint returns the new
`content_version` for the document.

### Get Updates
The server exposes an endpoint, `GetUpdates`, which accepts a timestamp
representing the most recent `metadata_version` of any update the client has
received. The endpoint returns the metadata for all files with a greater
`metadata_version` i.e. all files which have been updated since the client last
pulled updates.

### Get Document
The server exposes an endpoint, `GetDocument`, which accepts an `id` and
`content_version` for a document and returns that version of the document.

### Sync
The client has a routine, `sync`, which is responsible for pulling new updates
from the server, resolving conflicts, and pushing local updates. It consists of
5 steps (and additionally prunes deleted files at the end):

1. _pull all updates 1_
2. _push metadata updates_
3. _pull all updates 2_
4. _push content updates_
5. _pull all updates 3_

These 5 steps run 3 subroutines:
- pull
- push metadata
- push content

#### Pull
The pull routine is where most of the work in `sync` happens. First, it pulls
updates from the server. If applying those updates to `base` would cause any
updates to apply to orphaned files, those updates are dropped.

Then, each update (`remote`) is 3-way merged with the `base` and `local`
version. The 3-way merge resolves conflicts to `parent` and `name` in favor of
remote changes, if any, and resolves conflicts to `deleted` by preferring to
delete the file. The merge is designed so that if there are no local changes,
the result is `remote`, and if there are no `remote` changes (this can happen if
another client renames a file then renames it back to the original name), the
result is `local`. The `remote` metadata are buffered as a set of changes to
apply to `base` because after the pull, `remote` will be a version which has
been accounted for by both the client and server on a metadta-by-metadata basis.
The metadata resulting from the merge are buffered as a set of changes to apply
to `local` because they represent a version of the metadata that account for
local changes that have not yet been synced to the server.

If the `content_version` in a remote metadata update is greater than the
`content_version` in the `base` version of that file, then the file is a
document that has been updated on the server since the last pull. The client
pulls the document, 3-way merges it's contents with `local` and `base`, buffers
the pulled version as an update to the `base` version of the document and
buffers the merged version as an update to the `local` version of the document.
For file types that are not mergable (inferred by their file extension), a new
file is created with the `local` contents and the `local` and `base` contents of
the existing file are set to `remote` version, which is left unmodified as it
was read from the server. The new file has the same parent and a file name based
on but different from the existing file and all other files in that folder.

Before saving the buffered updates, invariant violations need to be resolved.
Cycles are resolved first - any files moved locally that would be involved in a
cycle after updates are saved have their parents reset to `base`. Then, path
conflicts are resolved - any locally created or modified file with the same name
and parent as a pulled file is renamed. Finally, all metadata and document
updates for `base` and `local` are saved.

_Concern: the logic to determine whether to pull a document currently references
the `local` `content_version`, which is unnecessary and may introduce unknown
bugs._

_Concern: should cycle resolution not reset the parents of locally moved files
which are also move remotely? is it sufficient to reset the parent of just one
file per cycle?_

#### Push Metadata Updates
To push metadata, the client simply collects all metadata which has changed (or
been created) since the last push and sends them to the server's
`UpsertFileMetadata` endpoint. If any preconditions fail, `sync` is aborted, and
can be safely retried. The server updates the `metadata_version` but does not
return it; this means that the updates pushed by the client will be pulled
during the next update. Then, all files have their `base` metadata set equal to
their `local` metadata (which are the metadata that were sent to and accepted by
the server).

_todo: consider optimizing the `metadata_version` mechanics so that the client
doesn't pull every update it pushes_

#### Push Content Updates
To push document content, the client collects all documents which have changed
since the last push and sends them to the server's `ChangeDocumentContent`
endpoint. If any preconditions fail, `sync` is aborted, and can be safely
retried. The `content_version` of each document is used to update it's `base`
and `local` metadata. Then, all documents have their `base` contents set equal
to their `local` contents (which are the contents that were sent to and accepted
by the server).
