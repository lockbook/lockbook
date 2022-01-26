# Sharing

The sharing UX needs to address the following concerns:

* share individual files or share folders recursively
* share files without moving them
* accept or deny share requests
* organize shared files in your own file tree
* readonly vs read/write access
* unambiguous relative paths (e.g. cli, markdown rendering)
* who is billed for shared documents?
* should users be able to reshare shared things?

## Sharing A File
To share a file, a user selects the "share" option from a context menu. They are
prompted to enter a username for a user to share the file with and a share mode
(read or write). The username is checked immediately - **the client must be
online to share**. The sharer is always considered the owner of the file is and
is billed for its storage.

Additional ideas for non-MVP:
* autocomplete with usernames of past sharees
* cache public keys of sharees - can share with these users offline

## Receiving A Shared File
When a user syncs and the client discovers new files shared with the user, a
non-intrusive UI element (e.g. a banner or badge) calls the user's attention to
a pending share. When the user engages with the pending share, the UI prompts
them to either reject the share or to place the shared file somewhere in their
file tree. They can name the file anything and put it anywhere - the original
file is unmodified from the perspective of the sharer. If the user deletes the
file, it becomes a pending share again. The only way to lose access to a shared
file is to reject the pending share. If a folder is shared with you and that
folder contains a link to a file that's not shared with you, you can't see the
contents of that linked file.

Additional ideas for non-MVP:
* the UI visually distinguishes shared files
* easily request access to linked unshared files
* files are signed by the most recent editor; UI shows time and author of last
  edit
