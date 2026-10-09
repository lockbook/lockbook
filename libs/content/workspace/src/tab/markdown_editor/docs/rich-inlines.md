# Rich inlines
An image, a link chip, a link card, and a fold `···` chip each stand in for source you usually don't want to look at. Reveal is conservative because showing that source jumps layout ([reveal](reveal.md)).

Select as a unit, like a rich-text glyph ([layout](layout.md), [selection](selection.md)). Click has a job (open, unfold, menu). Backspace deletes the whole thing. Copy should eventually take the pretty thing *and* the source (image pixels and `![]()`), same family as copy-as-plain / copy-as-markdown ([edits](edits.md)).

Mobile "Edit" puts the caret in the hidden destination so you can change a URL without the stand-in vanishing for lack of an interior.

**Note:** the full family. **Composer:** images and link cards/chips. **Read-only:** pretty form; click opens links; images are inert; click-to-open; fold toggle is viewing ([folding](folding.md), [identity](identity.md)).

## Chip vs card
A **bare autolink** alone on its line is a block **card**; inside a broader paragraph it is a wrappable **chip**. `[label](url)` stays labeled text. `<https://…>` opts out of the pretty form.

## Images
Work like a glyph in the wrap. Obsidian `|WxH` is a feature we intend ([dialect](dialect.md)). Alt text: whatever is normal — likely shown on hover or as a placeholder if the image cannot be shown. First-load size jump is unavoidable but best effort caching mitigates it on subsequent loads ([layout](layout.md)).

## Fold chip
Modeled on Bear. Click unfolds. Select as one unit. Stands in for the hidden HTML comment and contents; never show the tag source. Caret-in-fold unfolds for real ([reveal](reveal.md), [folding](folding.md)).

The composer and sent messages skip fold chrome.