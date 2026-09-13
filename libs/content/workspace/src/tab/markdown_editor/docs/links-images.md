# Links and images

A rich design space, still moving. Chip vs card, fetch **setting**, and `\|WxH` are [rich inlines](rich-inlines.md) / [dialect](dialect.md). This note is **what a destination is**, how it gets into the file, privacy, sharing, and (at the end) embedding more than images.

Open then save still must not rewrite the file ([document-model](document-model.md)). Completions UI is chrome; the *choice* of markdown vs wiki vs autolink is here.

## Destinations

A link goes to **another Lockbook file** or a **web URL**. Syntax is a normal markdown link or a **wiki link**. Both round-trip as the user typed them.

Wiki vs `[text](url)` vs a bare autolink is not a settings choice. In practice people pick by how they start: `[[` → wiki, `[` → markdown. Completions follow that. Hasn’t been a real concern. More ways to get a link in are better; select-text-and-paste-a-URL stays. Completions are a standout in the composer.

**Open:** a Lockbook file stays in the app — don’t hand it to the system. A web URL goes to the web; new tab vs current tab is whatever is normal.

## Broken, missing, unshared

Uncertain, still settling.

**Obsidian-shaped wiki:** people *deliberately* leave unresolved wiki links as placeholders; several notes pointing at the same missing title are linked together; click **creates** the note. For wiki links, “cannot resolve” may be **normal**, not red-broken.

We still like **broken-link checking** so documents don’t decay over time (markdown paths that rot, etc.). Current “it’s red if we can’t resolve” is probably too blunt.

**Not shared with you** → that link **is** broken.

**Aspiration:** when you are about to share a file, tell you it links to files you are not also sharing, and offer to share them together so the links keep working.

Resolve links **inside Shared with me**. We should; may not today.

## Fetch (privacy)

Outbound fetch for card titles/thumbnails is **default off**. Someone can put a URL in a note they share with you; if we fetch on preview, that site learns you opened the note. Not private.

Maybe default-off only for shared notes. As long as we negotiate privacy well, the rest (retry, backoff, cache) is details.

## Getting an image in

Support **all the usual ways**, especially on mobile (camera, paste, picker, share sheet, …). **Drop onto the editor** is a design goal — not quite there (winit drop location was a problem; progress since).

Paste-as-link for URLs is already [edits](edits.md).

## Attachments and `imports/`

Today pasted images land in a sibling **`imports/`** folder. Grouping attachments is good; the **name** is not intuitive.

**Constraints** (design not settled):

- Sharing a note should be able to share **its** attachments as **one** extra unit, not a slew of unrelated shares.
- Ideally attachments are **per note**, not one `imports/` for every file in the folder — otherwise you cannot share note A without note B’s images.
- Links need a **path** that still resolves, including in Shared with me.

## Embeds and shared building blocks

We are open to embedding **more than images**. It is oddly specific that an image embeds and a drawing or a table file does not — we already show arbitrary tab types in search previews. Anything we know how to show, we could show in a note (focus issues if we also make it editable). **SVG** (already a first-class editable type) is particularly exciting.

Same idea, inverted: a **Rust code file** should look and behave like a **Rust code block**, and a **CSV** like a **markdown table**, sharing code. Tables are about to get real ([blocks](blocks.md)); bringing that to a standalone CSV would be very cool.

The practical blocker is the **parser**. A fenced code block inside a quote does not occupy one contiguous byte range — quote prefixes are interleaved. A parser that treats those ranges as first-class would let the code-block widget reuse the code-file widget. Another motivator for our own parser ([dialect](dialect.md)) — still not a Lockbook-only language.

## Jobs

**Note and composer:** links, wiki, images, cards (if fetch allows). Completions especially in the composer.

**Show:** clickable links; images if we have them. Don’t fetch if privacy says no.

## Not this note

- Chip vs card layout.
- Completion popup chrome.
- Exact `imports/` naming and folder layout — constraints above, not a frozen path.
