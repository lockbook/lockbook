# Document links
A note links to another Lockbook file by relative path, absolute path, wiki title, or `lb://<uuid>`, and to the web by URL. Relative links survive publishing and export to other tools and are what completions insert inside one tree. `lb://` survives renames and moves and is what completions insert across trees. Wiki links resolve by title within one tree. The editor's side of this — reveal, fetch privacy, images, attachments — is in the editor's [links-images](../../../content/workspace/src/tab/markdown_editor/docs/links-images.md).

## Trees
A link resolves within one tree: your own files, or one subtree shared with you, as the outermost share defines it. Relative, absolute, and wiki forms never cross a tree boundary; an absolute path is anchored at your own root and never enters a share. `lb://` crosses trees, which is why it exists in notes at all. A note in a shared tree that links outside it is marked with a warning: others reading the note may not see the destination. A link that resolves to nothing, wiki or otherwise, is marked broken and clicking it does nothing. A document link never resolves to a folder.

## The external link
`https://app.lockbook.net/open/<uuid>` is the shareable form, a URL that chat apps linkify where `lb://` is not. The server answers it with a page that hands off to `lb://` and publishes the Apple site association, so on Apple the link opens the app directly: sync, then open. It is served by the API server rather than a static redirect so the same URL can later serve a page and link previews.

## Rough edges
- Target: a shared note's links keep resolving for everyone it is shared with and survive the destination being renamed or moved (#5141); sharing a note warns about links to files not also shared and offers to share them together (#5143); an unresolved wiki link is a placeholder that creates the note on click (#5142); attachments travel with their note as one unit.
- Which form a client should write across a share — relative or `lb://` — is undecided; the yellow cross-tree warning is where that stands today.
- Folder links were a September side quest and are not resolved by notes.
- Android has no App Links file or intent filter and desktop only copies the external URL (#5157).
- "Anyone with the link" is a stated direction with no design; whether it shares a data model with [sharing](sharing.md) is open.
