# Edits
The overall feel is **Google Docs-ish**: markdown pretending to be rich text. Backspace a bullet away or have a next item inserted at the right moment.

The product is defined on the **parsed structure**, and every change still has to be a **source replacement** ([document-model](document-model.md): the file is the bytes). That translation is delicate but essential.

## Return
Return **inserts the next thing** — a new list item, another quoted line, and so on. Shift+Return **softens** that: extend the current thing, adding only the indentation that continuation needs.

## Backspace and Ranges
With a **single caret**, backspace generally peels **one nesting layer** — a whole prefix at a time (the bullet, the `>`, one level), innermost-first. With a **range**, delete that range — the smart behaviors are reserved for a caret (empty selection).

## Tab and NBSP
Tab **indents / nests** (and Shift+Tab outdents).

Tab characters made the editor unstable — GFM treats a tab as a variable number of spaces depending on column, and we had a stream of visually confusing bugs — so they are disabled. We would like to support them; “no `\t` in the file” is a workaround.

**Non-breaking spaces** are the same category. They often arrive from AI output; the parser does not interact well with them, so paste currently flattens U+00A0 to a normal space. We would like to keep them.

## Copy/Paste
Copying to clipboard takes the **source characters** you selected. With an empty selection, it takes the current source line.

Target feature set:
- Copy as **plain text** — drop the styling punctuation.
- Copy as **markdown** — even if the selection is only the inside of a bold, put the opening `**` on the clipboard too. This alleviates a [reveal](reveal.md) / [layout](layout.md) annoyance: you cannot select hidden characters (heading `#`s) during a drag, only after release when they appear.
- Similar options for paste, especially for rich text in the clipboard.
- OS **multi-format** clipboard, so wherever you paste we make a best effort to keep formatting.

## Toggle Style
Cmd+B / toolbar Bold (and the rest) wrap or unwrap the selection with the intended syntax. This logic should account for any existing styled regions in the selection which may need to be joined, split, shortened, or extended. If you are in a code block or anywhere else inlines cannot exist, the style toggle should be refused.

## Container Block Editing
For **multi-line blocks** — lists, quotes, tasks, and the like — you should be able to keep typing as if you were not in that container. What you write is exactly what you would have written, except it is in the quote (or list, or …). Return, for example, puts the quote prefix on the next line for you. This is a best effort and may not hold with empty lines where a return is intended to terminate the block (unless you use the shift modifier).

In nested container blocks, ambiguity in operations is resolved innermost-first.

Code blocks have their own indent habits and should behave more like a code editor on a best effort basis.