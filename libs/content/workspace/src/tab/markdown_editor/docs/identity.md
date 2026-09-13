# Identity

Lockbook is a markdown knowledge-management app. The editor is what opens when you click a markdown file: one source-preserving WYSIWYG view that is both editing and preview, so you never bounce between plain source and a rendered pane.

Obsidian is the reference. We are GitHub-flavored markdown, native on every platform, with low-setup end-to-end encrypted sync. If we had Obsidian’s editor feature set in those native apps with our sync, we would be in a strong place as a business. We have constraints and ambitions they don’t; those show up in later notes, not here.

## A text stack, not just an editor

The note tab is the product. Over time other places needed to *show* or *edit* text with the same qualities (WYSIWYG markdown, emoji, completions). The current types (`MdRender`, `MdEdit`, `Editor`, `MdLabel`) are a split from that pressure — chat especially — not a product taxonomy.

The target is an app-wide text stack for any showing or editing need: markdown, plaintext, or code; proportional or monospace; with emoji. In that state we would not keep a parallel egui or Glyphon label/text-edit outside these layered pieces.

Do not maintain an inventory of call sites. It would decay. Describe the *job* a place is doing, then compose the stack for that job.

## Compose building blocks; don’t flag a monolith

The short-sighted way to reuse a markdown editor is a pile of flags that turn features on or off per use case. That is how the current code is shaped, and it is an implementation detail. Users and this catalog do not care what the flags are called.

The scalable pattern: decompose into building blocks so each place assembles what it needs, and so which blocks a job uses can change without a maintenance burden. The failure mode we care about is machinery running that does not belong — code that isn’t needed for the job causing bugs, focus theft, or extra UI.

That composition is also how a **code block** should reuse a **code file**, a **markdown table** a **CSV**, and a note an **embedded SVG** (or any tab type we already know how to show). Parser ranges are the practical blocker — [links-images](links-images.md), [dialect](dialect.md).

Device differences (phone, iPad compact-as-phone, iPad full with touch-scroll, desktop where a drag is a selection) are real. How they are currently modeled is messy and not identity.

## Jobs

**Note.** Full edit-and-preview together. This is the editor. Folds, find, list reorder, toolbar, cards, images, completions — in.

**Composer** (chat; similar someday on canvas). Same typing, rendering, emoji, link completions, images, and link-preview cards. More edit than preview. Folds and find are too much UI and are out. List reorder is in: it adds no extra chrome, only progressive disclosure if you use it. A composer toolbar is not a requirement here; the stack should make one cheap to add (including behind a setting) without this note having to decide.

**Show** (chat messages, search/toolbar previews, and the like). Rendered markdown, not an editor. Links are clickable — especially in chat, so you can open what you or the agent just named. No caret, no focus theft.

**Plaintext / code.** A real goal (people open these files), currently a hack: too much of the markdown editor is in play. Syntax highlighting matters; Syntect is weak, Tree-sitter is the direction. Today’s architecture is a blocker — the two jobs are far apart and should share building blocks, not one widget with a mode. File extension as the switch is a stopgap; it cannot express a side-by-side source+preview workflow someone has already asked for.

**Single-line.** One unbounded line, not a wrapping note. Used for short fields, not only secrets. A masked secret (e.g. API key) is a variant: same layout, no markdown chrome, glyphs hidden. Neither is the note editor.

## Read-only

Standout cases: a chat message already sent; a file shared read-only.

Viewing is still allowed. Toggling a fold is viewing, so it is allowed on a read-only shared file; we just refuse to save. Toggling a **task checkbox** is an edit — not clickable in a read-only view. Read-only therefore has edges — it is not “the widget is inert.”

A separate “lock against accidental edits while previewing” mode has been considered and is not implemented. Not a current requirement.

## Out of scope for identity

- Flag names (`plaintext`, `mask`, `interactive`, `disable_images`, …)
- Host input plumbing (keyboard/IME). We care whether typing and the caret *feel* right in each job; the mechanism is a later note.
- Public-site WASM demo — incidental, likely going away.
- An exhaustive list of surfaces.
