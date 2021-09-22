# First party support

The following files will receive first party support on all platforms:

+ Plain Text
+ Drawings
+ Todo lists

These are the set of file types (for now) that we will author UI elements for.

# Second party support

For arbitrary files within clients we'll a configurable process with a temporary file, and watch that file for changes.

We'll choose some defaults on a per-platform and binary-availability basis. We can explore using tools like `open`,
and `xdg-open` that push the program selection responsibility to the operating system. But these processes generally
give us less control over controlling how the resulting program runs. Further investigation here is required.

# Lockbook Drive

Later, we'll create Lockbook Drive, a program that mounts a FUSE drive that contains lockbook file contents.

We'll likely want to be able to control which files get synced to what device before taking on this feature set to allow
people to make sure they're not saving the same files twice on their machine.