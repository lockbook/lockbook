# Sync v3
## Overview
Sync v3 is the latest iteration of our logic that syncs users' file trees across devices. It is designed under the following key constraints:
* The state of a user's file tree must satisfy certain invariants at all times, on all devices (including the server), even in the event of process interruptions and failures.
* Users' file contents and names must not be readable by the server, but users can share files and folders with other users.
* Users must be able to view and edit files offline, then sync frictionlessly, even with intervening edits made to files on other devices.
* The performance of the system must not suffer as a result of typical usage, which presents a number of challenges worth discussion

These design constraints motivate the following high-level design, which will be flushed out in more detail after a further discussion of the constraints:
* The client maintains two versions of the file tree: one for the current local version (`local`), and one for the last version known to be agreed upon by the client and server (`base`).
* The server maintains one version of each user's file tree (`remote`)
* To perform a sync, the client pulls `remote` from the server, 3-way merges the changes with `local` and `base`, and pushes those updates to the server.

## Constraints
### File Tree Invariants
Before discussing the invariants, it is helpful to understand the data model. For the purposes of Sync v3, each file has:
* `id`: an immutable unique identifier
* `file_type`: an immutable flag that indicates if the file is a folder or document. A _file_ is either a _document_, which has contents, or a _folder_, which can have files as children.
* `name`: the file's name. Clients encrypt and HMAC names before sending them to the server, so while the server cannot read file names, it can test them for equality.
* `parent`: the file's parent folder (specified by the parent's `id`). The `root` folder is the folder which has itself as a parent (it's name is always it's owners username and it cannot be deleted).
* `deleted`: a boolean flag which indicates whether the file is _explicitly_ deleted. A file whose ancestor is deleted need not be explictly deleted; if not, it is considered _implictly_ deleted. A file that is no longer known to a client at all is considered _pruned_ on that client.

This data model lends itself to three operations, which correspond to modifications to the three mutable fields of a file:
* `rename`: change the file's name
* `move`: change the file's parent
* `delete`: delete the file (cannot be undone)

With that data model in mind, these are the four main invariants that must be true of a user's file tree on all devices at all times:
* Root Invariant: there must always be exactly one `root` and all modifications to `root`s are invalid. This is straightforward to enforce.
* Path Conflict Invariant: no two files may have the same name and parent. An example challenge is when a user creates a new file on each of two clients with the same name and parent, then syncs them both.
* Cycle Invariant: every non-root file must not have itself as an ancestor. An example challenge is when on one client the user moves folder A into folder B and on the other client moves folder B into folder A, then syncs them both.
* Orphan Invariant: every non-root file must have its parent in the file tree. To keep client storage from growing unboundedly, clients eventually prune deleted files (remove all mentions of them from durable storage). If the design does not pay careful attention to the disctinctions between explictly deleted, implicitly deleted, and pruned files, a client may prune a file without pruning one of its children.

_Concern: does the server check that a user's root folder is named after the username?_

### Privacy
Files, file names, and encryption keys never leave the client unencrypted. In order to support sharing, we give every document a symmetric key (used to encrypt its contents) and every folder a symmetric key (used to encrypt the keys of child folders and documents). This allows users to recursively share folder contents performantly and consistently. It also creates an encryption chain for each document: the document is encrypted with the document's key, which is encrypted with its parent folder's key, which is encrypted with it's parent folders key, all the way up to the root folder whose key is encrypted with the user's account key which is stored on the user's devices and trasferred directly between them as a string or QR code.

This encryption chain, and its interaction with the invariants, creates programming challenges. Our general pattern is to decrypt keys, names, and files as we read them from disk or receive them as responses from the server so that we can write logic without worrying about encryption. However, most invariant violations (e.g. orphans and cycles) will cause us to fail to be able to decrypt files, which in turn makes it difficult to detect and appropriately resolve invariant violations.

_Todo: write and link the sharing design doc_

### Offline Editing
The key challenge in offline editing is what to do when concurrent updates are made to files. As previously mentioned, the client maintains two versions of the file tree: one for the current local version (`local`), and one for the last version known to be agreed upon by the client and server (`base`). Our ambition is to define a 3-way merge operation for file trees which we can use after pulling the server's version of the file tree (`remote`) which never results in a invariant violation and which otherwise produces satisfying behavior to users. This operation needs to take place on clients because the server does not have access to decrypted files. Once a client uses the 3-way merge operation to resolve conflicts, they can write their updates back to the server.

For file contents, we can use a standard 3-way file merge operation like the one used by Git. If there are edit conflicts, we resolve them by leaving in both versions along with Git-style annotations of which change came from `local` vs `remote`. When a 3-way merge operation doesn't make sense (e.g. for binary files), we can resolve edit conflicts by saving both copies of the file, with one of the files renamed).

For file metadata, we prefer the remote version of a file's `name` and `parent` because we think it makes the most sense for the client whose updates reached the server first to have its changes preferred. If either the server or the client deleted a file and the other did not, we resolve the conflict by deleting the file so that file deletions are permanent. File metadata also contain other fields (discussed in the next section) which require careful consideration during a merge.

As mentioned, some invariant violations arise from concurrent multi-device edits, even if the individual devices maintain those invariants locally. Therefore the 3-way merge operation requires additional logic to resolve these constraints in a user-friendly way.

### Performance
There are a handful of performance pitfalls which we need to avoid.

When pulling the state of a user's file tree from the server, we need to avoid pulling the whole file tree. We intend for the system to scale to thousands or millions of files per user - the user might have access to a shared folder containing files for their entire company. To this end we track a version for the file in its metdata (`metadata_version`). The `metadata_version` for a file is simply the server-assigned timestamp of the file's most recent update, represented as a Unix epoch. When a client fetches updates from the server, it passes the most recent version it knowns of, and the server replies with the new metadata for all files that have been updated since that point. This way, the volume of data pulled scales with the velocity of updates to the user's file tree rather than the size of the file tree.

Pulling the metadata for a file and pulling the content for a file are, in terms of performance, categorically different. A file's contents are generally many orders of magnitude larger than file metadata. When we pull an update for a file, we need to avoid re-pulling the content for the file if it hasn't changed (e.g. if the file was only renamed or moved since the client last checked with the server). To this end, alongside `metadata_version`, we store a `content_version` in each metadata which is a server-assigned timestamp of the file's most recent content update. When a client pulls a metadata update, it also pulls the file content if and only if the `content_version` has changed since it last pulled.

Clients store two versions of the file tree, `base` and `local`. In order to save space, we use semantics similar to copy-on-write where the version of a file on `local` is not saved if it is identical to the version on `base`. Clients interpret the absence of a `local` version to mean that `local` and `base` are identical. Additionally, we compress files, both before saving to disk and before pushing to the server.

The metadata for deleted files must be eventually pruned on clients so that the amount of space taken on users' devices scales with the number of not-deleted files rather than the total number of files ever created. Deleted files cannot be immediately pruned because the client needs to remember to push the deletion to the server on the next sync. The server keeps track of all file metadata forever (though it deletes contents of deleted files) so that clients can sync after being offline indefinitely and still receive deletion events without the complexity of the server needing to know which clients exist and which updates they have each received. We are not concerned about storage usage on the server growing with the number of total files ever created.

## Design Concerns
Sync v3 operates in the context of many concerns. We'll discuss the design first in terms of how the concerns are addressed one concern at a time, then in terms of the client and server routines that address all these concerns all together. These are the concerns we'll discuss:
* file deletion
* invariant violation resolution
* consistency

### File Deletion
As mentioned, files have various states of deletion:
* not deleted (the file has `false` value for `deleted` and no deleted ancestors)
* explicitly deleted (the file has `true` value for `deleted`)
* implicitly deleted (the file has `false` value for `deleted` but has an explicitly deleted ancestor)
* pruned (there is no mention of file on disk, although it once existed)

First we'll justify the distinction between deleted and pruned. If the file deletion system is working properly, then after a user deletes a file on a device and syncs that device, that device will prune the file and any other devices that sync will prune that file. What's required to make that happen is that the client which deletes the file remembers that the file exists (and is deleted) until that client syncs its changes. Once the changes are synced, the client can prune the file knowing that the server will remember the deletion forever and will inform the other clients of the deletion. Note that deleted documents can have their contents pruned immediately - it is only important to remember the metadata until the deletion is pushed.

Next we'll justify the the distinction between implicit and explicit deletions. What it would mean to not have this distinction is that a file would only be deleted if it is marked explicitly deleted, so in order to delete a folder either the user would have to delete all its children first or the client would have to automatically delete all its children first e.g. recursively delete folders (otherwise those children would become orphans, violating the Orphan Invariant). This would be undesirable because of the behavior of folder deletions in the presence of multiple clients. Consider the case when one client moves a document out of a folder and syncs, then another client deletes the folder and syncs. If a client applied explicit deletions recursively, the second client would delete both the folder and the document, then the next time the other client synced, the document would be deleted even though it was not in the deleted folder. We decided this was unacceptable behavior.

In order to prune a file, we need it to be explictly deleted (and have no descendants that are not explictly deleted and would therefore become orphans), and for its explicit deletion to be synced to the server. If it's only implicitly deleted, then it potentially could have been moved out of its deleted ancestor in an update that we have not yet pulled, so a client cannot yet prune it. This will perpetually be the case for implicitly deleted files unless they are at some point explictly deleted. This means the server is the entity which must ultimately explicitly delete implicitly deleted files, which is the design we implemented. Once the server explictly deletes all implicitly deleted files, clients receive these deletions as updates the next time they sync, which indicates that it is safe for them to prune the files.

Revisiting the earlier example, when a client moves a document out of a folder and syncs, the server becomes aware of that move. Then when another client deletes the folder and syncs, the server marks all implicitly deleted files as explicitly deleted. This does not include the document because the server is already aware that it has been moved. If the first client hadn't synced until after the second client, then the document would have been deleted. This is one of the ways in which we resolve conflicts in favor of the first client to sync.

Note: when a document is implicitly deleted on a device, the device can prune the documents' contents and just keep the metadata which is much smaller. If, after syncing, the document is no longer implicitly deleted, the device can re-pull the contents from the server.

### Invariant Violation Resolution
Even if clients individually enfoce invariants, violations can still occur in the presence of concurrent updates. This is a complicated matter.

First, we have to be concerned about the Cycle Invariant. A cycle can occur if a user has two folders, moves the first folder into the second on one client and moves the second folder into the first on another, then syncs them both. A cycle can also involve more folders, if a first folder is moved into a subfolder of a third folder and the third folder is moved into the first folder concurrently.

We also have to be concerned about the Path Conflict Invariant. A path conflict can occur if two clients create files with the same name in the same folder and then sync. Occurrences can also involve a client renaming an existing file in that folder, moving a file into a folder, or after a file edit conflict is resolved by saving a copy of the local version with a new name. Additionally, it must be valid for two files to share a name and parent if one or both of them are deleted.

Finally, we have to be concerned about the Orphan Invariant. An orphan can occur if a client deletes a folder and syncs, then another client moves a file into that folder and syncs, then the initial client syncs again. The initial client will prune the folder after pushing its deletion, then the second client will push the move and the server will mark it as deleted, then the initial client will receive an update to a file for which it does not have the parent. In this situation the client needs to resolve the invariant violation without decrypting the update because the encryption key to decrypt that update lives in the folder's metadata which was already pruned.

There are enough corner cases here to drown in. Rather than employ a team of talented engineers to discover and diagnose them, we aim to implement a robust system which repairs the violations in a user-friendly way as part of the sync routine.

To resolve cycles, we rely on the fact that `base`, `local`, and `remote` all individually do not contain any cycles and design the resolution to preserve that. If the result of merging the file trees has a cycle, it must involve an unsynced local move and a remote move the client just pulled from the server. In particular, one of the files in the cycle must have been the subject of an unsynced local move. Our policy to resolve this is to unmove (by referring to `base`) all the locally moved files that are involved in any cycles that result after a file tree merge.

To resolve path conflicts, we rely on the fact that `base`, `local`, and `remote` all individually do not contain path conflicts, and therefore that one of the files involved in the conflict was somehow modified locally. Our policy to resolve this is to rename the locally modified file by appending a number to the pre-extention file name. For example, if a folder contains a new file called `notes.txt` from the server and a new local file called `notes.txt`, we rename the local file `notes-1.txt`. If there is already a `notes-1.txt`, we instead rename it `notes-2.txt`.

To resolve orphaned files, we rely on the fact that an update for a file will never be pulled without the parent folder being pulled first. If the parent folder no longer exists locally, it's because the parent folder has been pruned, which only happens if the deletion of that folder has been synced. If the folder deletion has been synced, then the server has explicitly deleted all descendants of the folder, including the orphans. Because deletions can never be undone, any updates to orphaned files can be safely ignored.

I know what you're wondering - does the order of these conflict resolutions matter? Yes it does. Any content conflicts need to be resolved before path conflicts because resolving content conflicts can create new files which can cause path conflicts. Any cycles need to be resolved before path conflicts because resolving cycles can move files which can cause path conflicts. Updates to orphaned files can be ignored at any time because no resolution policy depends on or modifies orphaned files.

_Concern: we don't currently respect "Any cycles need to be resolved before path conflicts because resolving cycles can move files which can cause path conflicts." A failing test case is left as an exercise for the reader_

### Consistency
todo:
- batch operations (unordered)
- core crash/interrupt recovery
- local files (embedded dbs)
- remote dbs (2PC)

## Design Routines
todo:
- user metadata/content edits
- upsert metadata
- get updates
- sync
- ...