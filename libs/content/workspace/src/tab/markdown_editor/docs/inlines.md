# Inlines
Styled text inside a paragraph. Images, chips, cards, and `···` fold chips are [rich-inlines](rich-inlines.md).

**Note and composer:** all of this. **Read-only:** rendered styles, tappable spoilers, links open, select and copy ([selection](selection.md)).

## Circumfix
Emphasis, strong, strikethrough, highlight (`==`), underline (`__`), subscript, and superscript hide their markers, look like rich text, and nest. Toolbar / Cmd+B wraps or unwraps ([edits](edits.md)). Applying a style should show the syntax in that moment so it can teach ([reveal](reveal.md)).

Two leaves: plain text, and **inline code**. Inline code does not nest. Combinations of the others are allowed; missing font variants (italic + mono + bold) are a UI gap, not a nesting rule.

Copy the *contents* of an inline code span, as a feature, has been asked for (e.g. a bulleted list of commands to paste into a terminal).

## Spoilers
Tap or click to show, caret inside to edit — whatever's normal. This is a different feature from syntax [reveal](reveal.md).

## Math
Target: **inline math**, in the line, something like an inline image. Until then, code-styled `$…$` is the stand-in.

## Emoji
Parse `:smile:` so typing and paste from elsewhere keep working. Completions may insert the emoji character so shortcodes mostly leave the file.

## Breaks
If the user hits Return, a newline goes in the file and they see it. Source newlines already render as hard breaks everywhere ([dialect](dialect.md)), so CommonMark's invisible hard-break (two trailing spaces, or a backslash) barely shows up.

## Links as text
**Show and read-only:** click opens. **Editing:** tap currently puts the caret on the URL (Slack-like); title vs destination is easy to mix up, and more chrome would help. Chip vs card is [rich-inlines](rich-inlines.md).

## HTML
Inline HTML shows as source unless it is a type we explicitly support; GitHub's finite vocabulary is the target ([dialect](dialect.md)).
