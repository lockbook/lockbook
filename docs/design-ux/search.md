# Search

+ Filename search -- `fzf` search via path (ctrl + l)
+ Full lockbook content search (ctrl + shift + f)
+ Text editor search (ctrl + f)

# Filename search

You hand lb-rs a string, lb-rs will hand you back a list of paths that this string matches. Should use some sort of `fzf`
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

## Content Search Text Splitting

We don't apply our matching algorithm to all the text of a file. Instead, we split a file into sections separated by 
`\n\n` (much like markdown). When a section has a match, we pre-process the text so that we only return the most 
relevant part of the section to a client. Ideally, we want to produce a 150-character window containing all 
matched indices.

When processing the text, different situations may arise, and in each one, we produce that window of characters 
differently.
1. The section matched on is less than 150 characters, so we return the whole area.
2. The matched indices are less than 150 characters, so we return the first 150 characters.
```markdown
Example
__Matched section:__
*Lorem ipsum dolor sit amet, consectetur adipiscing elit.* Vivamus lorem purus, malesuada a dui a, auctor lobortis dolor. 
Proin ut placerat lectus. Vestibulum massa orci, fermentum id nunc sit amet, scelerisque tempus enim. Duis tristique 
imperdiet ex. Curabitur sagittis augue vel orci eleifend, sed cursus ante porta. Phasellus pellentesque vulputate ante 
id fringilla. Suspendisse eu volutpat augue. Mauris massa nisl, venenatis eget viverra non, ultrices vel enim.
__Returned Window:__
*Lorem ipsum dolor sit amet, consectetur adipiscing elit.* Vivamus lorem purus, malesuada a dui a, auctor lobortis dolor.
Proin ut placerat lectus. Vest...
```
3. The section matched on is greater than 150 characters, but most matched indices lie at the beginning or end.
We remove the minimum number of characters, starting from the beginning, to get that 150-character window. 
Suppose there are not enough characters before the first matched index. In that case, we leave an 8-character buffer 
before the first matched index and remove as many characters as possible after the last matched index, following the 
same rules.
```markdown
Example
__Matched section:__
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vivamus lorem purus, malesuada a dui a, auctor lobortis dolor.
Proin ut placerat lectus. Vestibulum massa orci, *fermentum id nunc sit amet, scelerisque tempus enim.* Duis tristique
imperdiet ex. Curabitur sagittis augue vel orci eleifend, sed cursus ante porta. Phasellus pellentesque vulputate ante
id fringilla. Suspendisse eu volutpat augue. Mauris massa nisl, venenatis eget viverra non, ultrices vel enim.
__Returned Window:__
...a orci, *fermentum id nunc sit amet, scelerisque tempus enim.* Duis tristique imperdiet ex. Curabitur sagittis augue 
vel orci eleifend, sed cursus a...
```
4. If we still have more than 150 characters after step 3, we will only take the first 400 characters and matched 
indices; if it is less, we will leave it as is.
```markdown
Example
__Matched section:__
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vivamus lorem purus, malesuada a dui a, auctor lobortis dolor.
*Proin ut placerat lectus. Vestibulum massa orci, fermentum id nunc sit amet, scelerisque tempus enim. Duis tristique
imperdiet ex. Curabitur sagittis augue vel orci eleifend, sed cursus ante porta. Phasellus pellentesque vulputate* ante
id fringilla. Suspendisse eu volutpat augue. Mauris massa nisl, venenatis eget viverra non, ultrices vel enim.
__Returned Window:__
... dolor. *Proin ut placerat lectus. Vestibulum massa orci, fermentum id nunc sit amet, scelerisque tempus enim. Duis tristique
imperdiet ex. Curabitur sagittis augue vel orci eleifend, sed cursus ante porta. Phasellus pellentesque vulputate* ante
id...
```

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
    + complicates things within lb-rs because multiple processes may change what's on disk.