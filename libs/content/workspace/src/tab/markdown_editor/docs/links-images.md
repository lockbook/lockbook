# Links and images
A link goes to another Lockbook file or a web URL. Syntax is a normal markdown link, wiki link, or autolink. On open, a Lockbook file stays in the app, while a web URL resolves through the host system.

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