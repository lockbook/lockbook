# Search

+ Filename search -- `fzf` search via path (ctrl + l)
+ Full lockbook content search (ctrl + shift + f)
+ Text editor search (ctrl + f)

# Filename search

You hand core a string, core will hand you back a list of paths that this string matches. Should use some sort of `fzf`
style search. Can be integrated into quick-navigation of applications like it's done in the Linux application.

# Lockbook Content Search

Be able to quickly find files that match a certain criteria inside your lockbook. Not all the files in your Lockbook are
guaranteed to be available on disk. And not every file type lends itself well to search-time scans.

I propose we add a field to `FileMetadata` that contains `tags` or some other search term. Upon document writes these
tags are updated. [Stop words](https://en.wikipedia.org/wiki/Stop_word) can be filtered out, in the future we can
perform OCR on images and drawings. This field of `FileMetadata` gets encrypted and synced. When a user goes to perform
a content search, all of these fields are scanned.

After this tag based search, we should expand the search criteria and decrypt & scan the contents of all the files if
that's what the user desires. We should communicate the various steps of the search process to the user as the operation
proceeds.

# Text editor search

Implementation will be upto the text editors in question, will perform similar to "Search this page" in your web
browser.

# OS integrations

Some applications can, if appropriate integrate with OS specific omni-search mechanisms, this could be spotlight on
macOS or Gnome shell on linux.

# Performance considerations

My thoughts on how we'd go about improving the performance of these searches should that become an issue:

+ implement these with progress indication in mind. A communicative UI that's slow is better than a confusing UI that'
  s overall quick.
+ consider changes to on-disk format
+ consider caching some of these things into static space.
    + caching into static space allows us to not require ffi callers to hang on to a pointer
    + complicates things within core because multiple processes may change what's on disk.