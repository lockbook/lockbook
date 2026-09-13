# Dialect
Lockbook's editor speaks **GitHub Flavored Markdown** plus some extensions. It includes popular features (e.g. highlights) in chosen forms (e.g. `__` over `++` for underlines), aiming for broad compatibility in practice, though it will not perfectly match any single other markdown editor. It also includes less common features (e.g. Bear-style fold tags, Obsidian-style `|WxH` image dimensions) where it improves the product, aiming for at least one other client that's compatible.

## One dialect
We're not in the business of making a build-your-own markdown flavor so each user can have what they're used to. Instead, we prioritize **a single dialect so people's notes stay compatible with each other** and with the tools we chose. This simplifies the user experience and promotes network effects.

## Soft breaks are hard
The spec leaves soft-break rendering to the implementer, so we render **soft breaks as hard breaks, everywhere** — the note, the composer, previews. That is what an interactive editor needs, even if it's not the norm for rendering. Obsidian's editor does the same, but not in their render previews, which creates confusion when switching between these modes. We push that division out to the export boundary so notes within Lockbook appear consistently throughout.

## Interactive editing
Some CommonMark behavior is faithful and still feels wrong *while you type*. Setext headings are off because starting a hyphen-bullet under a paragraph briefly turned that paragraph into a heading and users found it very confusing. A lone hyphen on the next line is a one-item empty list — unless there is no blank line after the paragraph, in which case it is just a hyphen. Work-in-progress syntax should not surprise people mid-keystroke as able (subject to parser constraints).

## Parser
Lockbook's editor uses comrak, a pure Rust GFM markdown parser. It's oriented towards renderers in ways that negatively impact us:
1. **Performance**: We use parse-reported source positions to slice our document and negotiate how to draw the document. We don't need the parse's copies of slices of our document. Additionally, comrak's ownership model does not facilitate preserving a parse across frames (though experiments indicate workarounds exist). The combined effect is many unnecessary allocations per frame and is significant in the overall performance story.
1. **Correctness**: We maintain a collection of comrak source position corrections and workarounds. While comrak has been investing in source position reporting, its first priority seems to be HTML rendering.
1. **Detail**: Comrak reports a single range for each markdown node. This is insufficient for the careful attention demanded by nested container blocks, where the `>`'s of a block quote can be interleaved with the lines of an inner code block. We need to work in terms of the ranges that apply directly to a given node which, with comrak, requires us to build a complex layer to extend comrak's one-range-per-node source positions.

All existing markdown constructs must survive if Lockbook chooses to invest in its own parser.

## Extensions
* **Basics: highlights, spoilers, tasks, tables, alerts, underline, sub/superscript, strikethrough, link, image, autolink, wiki links, emoji shortcodes**. Wiki links follow Obsidian’s title-after-pipe and images follow Obsidian's width and height controls.
* **Advanced: math, front matter, HTML**. In the target state, we render inline math, provide an Obsidian-like frontmatter experience, and support GitHub's HTML vocabulary.
* **Not prioritized: footnotes**.
* **Excluded: description lists, greentext, multiline block quotes**. These create compatibility issues, engineering complexity, or editing annoyances.