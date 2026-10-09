# Identity
Lockbook is a markdown knowledge-management app. The editor is what opens when you click a markdown file: one source-preserving WYSIWYG view that is both editing and preview, so you never bounce between plain source and a rendered pane.

We are GitHub-flavored markdown, native on every platform, with low-setup end-to-end encrypted sync.

## A text stack
The note tab is the main product, but similar use cases appear around the app. The target is an app-wide text stack for any showing or editing need: markdown, plaintext, or code; proportional or monospace; with emoji.

## Building blocks
A pile of flags that turn features on or off per use case is how the current code is shaped. The failure mode is machinery running that does not belong: code that is not needed for the job causing bugs, focus theft, or extra UI.  In the target state, each place assembles the pieces it needs, and which pieces a job uses can change without a maintenance burden.

A related composition appears in how a **code block** should reuse logic for a **code file**, a **markdown table** for a **CSV**, and a note should embed any tab type we know how to show (similar to search result previews). 

## Use cases
**Note:** Full edit-and-preview together. This is the editor. Folds, find, list reorder, toolbar, cards, images, completions are in.

**Composer** (chat; similar someday on canvas): Same typing, rendering, emoji, link completions, images, and link-preview cards. Folds and find are too much UI. List reorder is in: it adds no extra chrome, only progressive disclosure if you use it. A composer toolbar can wait; the stack should make one cheap to add, including behind a setting.

**Show** (chat messages, search and toolbar previews, and the like): rendered markdown. Links are clickable — especially in chat, so you can open what you or the agent just named. The caret stays away; the surface does not steal focus. You should be able to select and copy. 

**Plaintext / code:** They should share building blocks with the note, rather than running the markdown editor as a mode. Syntax highlighting matters; Tree-sitter is the direction. File extension as the switch cannot express a side-by-side source and preview workflow someone has already asked for.

**Single-line:** One unbounded line, for short fields, not only secrets. A masked secret (for example an API key) is the same layout with glyphs hidden and no markdown chrome.

## Read-only
Standout cases: a chat message already sent; a file shared read-only. Toggling a fold is viewing, so it is allowed on a read-only shared file; we refuse to save. Toggling a **task checkbox** is an edit, so it is not clickable in a read-only view.