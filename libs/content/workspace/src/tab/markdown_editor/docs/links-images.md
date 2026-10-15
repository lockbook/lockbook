# Links and images
A link goes to another Lockbook file or a web URL. Syntax is a normal markdown link, wiki link, or autolink. On open, a Lockbook file stays in the app, while a web URL resolves through the host system.

## Scope
A link means the same file, or fails the same way, for everyone who can read the note. A note's scope is the subtree of its innermost shared ancestor (the whole tree for a note under no share), and relative paths and wiki titles resolve inside it: wiki titles match below the top, nearest first, and as in Obsidian only `.md` may be left off (`[[Plan]]` is `Plan.md`, a drawing is `[[Sketch.svg]]`); equally near matches are a tie, which resolves to nothing. Inside the scope, where a recipient places a share changes nothing.

Green links open the same file for everyone who can read the note. Yellow links open different things for different people, so they ask first. Red links open nothing. Only green images show.

Green is anything found inside the scope, and an absolute path in a note no one else reads. Yellow is a relative path or wiki title found only past the scope, anywhere in the reader's files, an id link (`lb://<id>` or `https://app.lockbook.net/open/<id>`) to a file outside the scope, and an absolute path in a shared note, which each reader resolves from their own root. Yellow asks the owner too: with write access, anyone can add links to a note. Red is anything that resolves to nothing, which is never handed to the operating system.

## Broken, missing, unshared
Obsidian-shaped wiki: people deliberately leave unresolved wiki links as placeholders; several notes pointing at the same missing title are linked together; click creates the note. For wiki links, "cannot resolve" may be normal.

We still like broken-link checking so documents don't decay (markdown paths that rot). Red-if-unresolved is probably too blunt. Not shared with you → that link is broken.

When you are about to share a file, tell you it links to files you are not also sharing, and offer to share them together (possibly special case to images). Resolve links inside Shared with me. Wiki click-to-create is an edit, so it is off in a read-only view.

## Fetch
Outbound fetch for card titles/thumbnails is default off: someone can put a URL in a note they share with you; if we fetch on preview, that site learns you opened the note. Maybe default-off only for shared notes. As long as we negotiate privacy well, retry/backoff/cache are details.

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