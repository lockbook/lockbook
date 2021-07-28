# First party support

The following files will receive first party support on all platforms:

+ Plain Text
+ Drawings
+ Todo lists

These are the set of file types (for now) that we will author UI elements for.

# Second party support

For arbitrary file types we have a few options.

## [FUSE](https://en.wikipedia.org/wiki/Filesystem_in_Userspace)

Use FUSE to create a mountpoint which represents lockbook's file structure.

Pros:

+ Transparent to other applications
+ Multi-platform support

Cons:

+ Requires FUSE kernel drivers (sudo and maybe restart to install).
+ Possibly would benefit from an isolated application
+ A bit of a hassle on macOS & Windows where a package manager is not necessarily used.

We could do this in a self standing way. Or we could try to integrate this into our existing applications. If we
integrate it in, double-clicking unsupported files within our app can launch their on-disk location.

## Explicit file sync

Replicate a particular file to a temporary location. Monitor it for changes. Could use a dialog to make the current
status of the file as obvious as possible.

Pros:

+ Transparent
+ Multi-platform
+ No strange deps
+ More straightforward user experiences (right-click a folder -> sync to computer, double-clicking a pdf)

Cons:

+ Requires duplicating file contents between lockbook-dir & on disk location. This is fine for 1-off temp files created
  to edit a libre-office document. Could be more problematic if people use lockbook to backup their home folder.

## 1 Way export

If you double-click on a file that we don't support, that file is moved to your downloads and is opened. Can attempt to
mark the file as read-only. This is *somewhat* the experience people are used to that backup finalized projects to
google drive. But Google Drive offers a FUSE mount as well. 