# Design Goals and Constraints

+ Respect the privacy & security constraints laid out in the [readme](README.md). This means, for example, that API nodes cannot perform conflict resolution on behalf of clients as they don't have access to the decrypted content.
+ Allow every operation to be performed offline and synced whenever the user connects to the network.
+ If the user allows it, be able to perform sync in the background without the need for the user's input on how to resolve conflicts (merge the content, let the user resolve the diff at their convenience).
+ All else being equal, preserve the creation of content in the case of conflicts.
+ If online, and actively using the app, minimize time spent out of sync.
+ (For the time being) integrity checks and signature verification are done in the content of the file. 

## High level strategy 

### Initial Sync
To initially sync:
* Clients hit the server's `/get-updated-metadata` endpoint passing a `since_version` of 0, indicating that they want the metadata of all files (with any version since the beginning of time), and save the result.
* The metadata includes the `file_id` of each file; clients use this to fetch file contents from `FilesDB`. This can be done lazily or up-front to sync for offline editing.
* The `/get-updated-metadata` endpoint also returns an `update_version` whose value the client copies to `last_updated_version`. This should be done after files are synced because if it is done before and the client is terminated before files are synced, the client will think it has all updates through `last_updated_version`, when in fact some updates were not persisted.

### Subsequent Syncs
To sync after the initial sync:
* Clients hit the server's `/get-updated-metadata` endpoint passing a `since_version` of `last_updated_version`, indicating that they want the metadata of all files that have changed since the client last sucessfully checked for updates, and save the result.
* The metadata includes the `file_metadata_version` of each file. If this is different from the locally stored `file_metadata_version`, the file metadata has changed and will be overwritten.
* The metadata includes the `file_content_version` of each file. If this is different from the locally stored `file_content_version`, the file contents have changed and the new contents should be retrieved from `FilesDB`. If there are local changes, the user will be prompted to merge the different versions of the file (if it cannot be done automatically).

### Writing Changes
When a user makes a metadata-only change, the server responds with the metadata of the affected file, which the client saves. When the user makes a content change, the server still responds with the metadata of the affected file, but the client must also pass the `file_content_version` of the file. This version must match the server's current version in order for changes to be written. If the file has been updated by another client since this client last synced, the versions will not match, which the server will indicate. The client needs to sync, and because there are local changes to the file, the sync will prompt the user to merge the different versions of the file (if it cannot be done automatically).

## Implementation Details

Broadly speaking there are 2 changes a file can undergo, a `metadata` change (name / location) or a `content` change. And this change change can happen locally, or on another device. 

If a change happens locally, during sync it has to be pushed up to the server, and if it happens on another device it needs to be pulled locally. There are many cases where this is a simple operation. When you create a new file, for example, syncing it is trivial: you push the new file up. Things get a bit trickier when you delete a file that has changes that you don't know about.

Naively we could just respect whatever event happened most recently. Practically speaking this is a bad strategy because, the new contents of the file may cause you to not want to delete it! In this particular situation your delete (metadata operation) is ignored in favor of someone's edit (content operation).

The following describes how we exhaustively model every scenario and how we resolve it with respect to the Design goals listed above.

## Exhaustively generating a list of scenarios

Sync is performed by diffing local state and server state. 

Locally, clients maintain the following struct for each file:

```rust
struct  FileMetadata {
    file_id: String,                // Immutable unique identifier for everything related to this file, TODO UUID
    file_name: String,              // Human readable name for this file. Does not need to be unique TODO make this encrypted / hashed / etc. 
    file_path: String,              // Where this file lives relative to your other files. TODO make this encrypted / hashed / etc.
    file_content_version: u64,      // DB generated timestamp representing the last time the content of a file was updated
    file_metadata_version: u64,     // DB generated timestamp representing the last time the metadata for this file changed
    content_needs_sync: bool,       // True if there are changes to content that need to be synced
    metadata_needs_sync: bool,      // True if there are changes to metadata that need to be synced
    deleted: bool                   // True if the user attempted to delete this file locally. Once the server also deletes this file, the content and the associated metadata are deleted locally. 
}
```

Analogously the server maintains a similar struct

```rust
struct FileMetadata {
    file_id: String,
    file_name: String,
    file_path: String,
    file_content_version: u64,
    file_metadata_version: u64,
    deleted: bool,
}
```

Separating sync into two distinct problems: calculating work involved, and performing the actual sync is useful for a number of reasons:
+ In certain settings, for certain types of users (CLI?) it gives the user a chance to see what's going to happen before it does.
+ For FFI clients (most clients) it allows them to expose progress to the user without the need for callbacks.
+ A bit easier to maintain.

However it comes with the trade-off that information can go "stale", generally this isn't a problem except under 1 circumstance. 

Most situations are resolved in a way that's "preferable / acceptable". For example:
You have version 9, sync calculates that you need version 11, that work unit is not executed for a few seconds (for whatever reason) and in the meantime the most recent server version becomes 12. When you do your update you go from 9 -> 11 instead of 9 -> 12.

The one situation where this is a poor experience is:
+ You have edits for a file that is actively edited by a large number of people and everyone's edits are resulting in conflicts all the time.

Now this would only happen if people are editing the same line of a file repeatedly. 
And the benefit of security drops with every new collaborator (as the chance of your file being leaked increases).
We've discussed having unencrypted modes in the app before, helping people publish to censorship resistant locations (public documents) for example. It's possible that this is a more reasonable way to do documents where the number of people who edit the document simultaneously exceeds 10. We can also explore P2P editing sessions which may allow us to maintain similar security gaurentees (primarily no need to trust server).
Though in all the work settings I've been a part off: startups of my own (this one), smaller companies (Gemini, ~200 people), large companies (SAP) and massive companies (JP Morgan) I've never come across this use case. Generally the more people have read access to a document (Covid response plan for example) the fewer people have write access to the document.  

### Calculating work involved for a sync

We begin by taking all the things `server` says we need to pull and all the things `client` says we need to push and putting it through the truth table below. Files not in either set, don't need to be touched by either party. Files in both sets are processed first and removed from both sets.