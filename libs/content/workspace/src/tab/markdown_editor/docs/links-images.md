# Links and images
A link goes to another Lockbook file or a web URL. Syntax is a normal markdown link, wiki link, or autolink. On open, a Lockbook file stays in the app, while a web URL resolves through the host system.

## Scope
A link means the same file, or fails the same way, for everyone who can read the note. A note's scope is the subtree of its innermost shared ancestor (the whole tree for a note under no share), and relative paths and wiki titles resolve inside it: wiki titles match below the top, nearest first, and as in Obsidian only `.md` may be left off (`[[Plan]]` is `Plan.md`, a drawing is `[[Sketch.svg]]`); equally near matches are a tie, which resolves to nothing. Inside the scope, where a recipient places a share changes nothing.

Green links open the same file for everyone who can read the note. Yellow links open different things for different people, so they ask first. Red links open nothing. Images are the same three: a green image shows, a yellow one is hidden like a spoiler (an eye-slash, "Click to show", a yellow dot) because in a write share a collaborator may have put a file of *yours* here; a tap shows it, and the picture then says nothing more; a red one has a red dot and shows why nothing resolves.

Green is anything found inside the scope, and an absolute path in a note no one else reads. Yellow is a relative path or wiki title found only past the scope, anywhere in the reader's files, an id link (`lb://<id>` or `https://app.lockbook.net/open/<id>`) to a file outside the scope, and an absolute path in a shared note, which each reader resolves from their own root. Yellow asks the owner too: with write access, anyone can add links to a note. Red is anything that resolves to nothing, which is never handed to the operating system.

## Writing links
A link can be typed by hand or picked. `[[`, `[`, `![`, or `](` opens a picker over the files the note can link to: those in its scope first, ranked by how the name matches what was typed (starts with it, then contains it, then has its letters in order along the path from the note), then by folders from the note, then by name. Within each class nearer files come first, then by name, so typing more can only drop or demote the files below the top one, never move them past it. Files outside the scope follow under a rule with a dot before their crumbs in the color the link will be drawn in: yellow where it reaches the file for this reader past the scope, red where nothing will (a wiki title for a file in another tree). Nothing you can see is hidden; the dot says what linking it gets you. The toolbar's link and image buttons write `[]()` or `![]()` with the cursor in the label and the picker open (files, or images); a selection becomes the label and the cursor goes to the destination.

A pick inserts a relative path from the note's folder inside the scope, the external URL (`https://app.lockbook.net/open/<id>`) outside it, and never an absolute path. A wiki pick inserts the shortest title unique in the scope: the name, else enough trailing folders (`[[design/Plan]]`), else the full name likewise, else the whole path from the scope's top (`[[/Plan]]`); for a file past the scope, the shortest title unique in your whole tree. A title with folders in it is read as a path from the note's folder, else as the end of a path anywhere in the scope, nearest first; `/` leads from the scope's top; case and the spaces around segments don't count.

Copy Link puts the file's external URL on the clipboard, any `#fragment` kept: the one form that means the same file wherever it lands. Pasting an `lb://` or external URL into a note writes the note's form: a relative path or the external URL by the rule above, labeled with the file's name or with the selection. Inside a link begun by hand (`[[`, `](`, a label) or inside code, the URL pastes as it is. A pasted image file links; `![` embeds.

A wiki title nothing matches is a placeholder, drawn in the theme's cyan: opening it creates the note, with any folders its title names, beside the note (`/` from the scope's top), never from a read-only view. A tie stays red.

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