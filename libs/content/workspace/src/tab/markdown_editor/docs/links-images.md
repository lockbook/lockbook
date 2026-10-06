# Links and images
A link goes to another Lockbook file or a web URL. Syntax is a normal markdown link, wiki link, or autolink. On open, a Lockbook file stays in the app, while a web URL resolves through the host system.

## Scope
A note's links resolve inside its scope, so that a link means the same file to everyone who can read the note, wherever they put the share. With nothing shared, the scope is your whole tree. Inside a shared folder it is that folder, and with shares inside shares, the innermost one: the tree every reader of the note holds. Paths stop at the scope's top, `/` starts there, and a wiki title means the nearest file of that name within it.

`lb://` is the form that leaves the scope, and only to a file the note's owner can see. It wears a warning: some readers will not be able to follow it. Images stay inside the scope.

## Writing a link
Completions list the scope first and what only `lb://` reaches after it. Each is ranked by how the name matches what you typed, then by folders away, then by name, so the file on top stays on top while you type the rest of its name. They insert a relative path, or the shortest wiki title that is unique in the scope.

Copying a link to a file copies its external URL, the one form that means the same file wherever it lands. Pasting one writes the form the note would have: a path inside the scope, `lb://` out of it, with any heading kept.

## Broken, missing, unshared
Obsidian-shaped wiki: people deliberately leave unresolved wiki links as placeholders; several notes pointing at the same missing title are linked together; click creates the note. For wiki links, "cannot resolve" may be normal.

We still like broken-link checking so documents don't decay (markdown paths that rot). Red-if-unresolved is probably too blunt. Not shared with you → that link is broken.

When you are about to share a file, tell you it links to files you are not also sharing, and offer to share them together (possibly special case to images). Resolve links inside Shared with me. Wiki click-to-create is an edit, so it is off in a read-only view.

## Upkeep
Every link in every note is indexed, by the note it is in and by the file it reaches, and the index follows each note as it is written and the tree as it changes. Reading every note means every note's text is fetched to the device. Each link remembers the file it reached. When a rename, a move, a share, or a nearer file of the same name takes links from their files, we say so in one line and offer the rewrite, with the list a click away; every rewrite waits for a yes, and is checked to reach the file before it is written. A link whose destination is written apart from it, as a reference's is, is left for a hand to update. A link that a share left outside its scope becomes `lb://`.

A pasted image whose last link is removed is offered for deletion to whoever pasted it; a drawing beside it that still shows it keeps it. A deleted file's linkers are named. Files are renamed, moved, shared, and deleted in each client's own file tree, so upkeep works from what changed in the tree, and behaves the same whichever client or device made the change.

## Fetch
Outbound fetch for card titles/thumbnails is default off: someone can put a URL in a note they share with you; if we fetch on preview, that site learns you opened the note. Web images follow the same switch in a note someone else can write, and wait for a click. As long as we negotiate privacy well, retry/backoff/cache are details.

## Getting an image in
Support all the usual ways, especially on mobile (camera, paste, picker, share sheet). Drop onto the editor is a design goal.

## Attachments
Grouping attachments is good. Constraints:
- Sharing a note should share its attachments as one extra unit, not a slew of unrelated shares.
- Ideally attachments are per note, not one folder for every file in the parent — otherwise you cannot share note A without note B's images.
- Links need a path that still resolves, including in Shared with me.

Pasted images land in a sibling `imports/` folder today; the name is not intuitive, and the path is not frozen.

## Embeds
We are open to embedding more than images. It is oddly specific that an image embeds and a drawing or a table file does not — we already show arbitrary tab types in search previews. SVG is particularly exciting.