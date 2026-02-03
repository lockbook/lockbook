# **Markdown Syntax**
Markdown is a language for easily formatting your documents. This document can help you get started.

## Styled Text
To style text, wrap your text in the corresponding characters.
| Style         | Syntax              | Example           |
|---------------|---------------------|-------------------|
| emphasis      | `*emphasis*`        | *emphasis*        |
| strong        | `**strong**`        | **strong**        |
| strikethrough | `~~strikethrough~~` | ~~strikethrough~~ |
| underline     | `__underline__`     | __underline__     |
| code          | ``code``            | `code`            |
| spoiler       | `||spoiler||`       | ||spoiler||       |
| superscript   | `^superscript^`     | ^superscript^     |
| subscript     | `~subscript~`       | ~subscript~       |

## Links
To make text into a link, wrap it with `[` `]`, add a link destination to the end , and wrap the destination with `(` `)`. The link destination can be a web URL or a relative path to another Lockbook file.
```md
[Lockbook's website](https://lockbook.net)
```
> [Lockbook's website](https://lockbook.net)

## Images
To embed an image, add a `!` to the beginning of the link syntax.
```md
![create your luck](https://upload.wikimedia.org/wikipedia/commons/4/47/PNG_transparency_demonstration_1.png)
```
> ![create your luck](https://upload.wikimedia.org/wikipedia/commons/4/47/PNG_transparency_demonstration_1.png)

## Headings
To create a heading, add up to six `#`'s plus a space before your text. More `#`'s create a smaller heading.
```md
# Heading 1
## Heading 2
### Heading 3
#### Heading 4
##### Heading 5
###### Heading 6
```
> # Heading 1
> ## Heading 2
> ### Heading 3
> #### Heading 4
> ##### Heading 5
> ###### Heading 6

## Lists
Create a list item by adding `- `, `+ `, or `* ` for a bulleted list, `1. ` for a numbered list, or `- [ ] `, `+ [ ] `, or `* [ ] ` for a task list at the start of the line. The added characters are called the *list marker*.
```md
* bulleted list item
- bulleted list item
+ bulleted list item

1. numbered list item
1. numbered list item
1. numbered list item

- [ ] task list item
- [x] task list item
```
>* bulleted list item
>- bulleted list item
>+ bulleted list item
>
>1. numbered list item
>1. numbered list item
>1. numbered list item
>
>- [ ] task list item
>- [x] task list item

List items can be nested. To nest an inner item in an outer one, the inner item's line must start with at least one space for each character in the outer item's list marker: usually 2 for bulleted lists, 3 for numbered lists, or 2 for tasks lists (the trailing `[ ] ` is excluded).
```md
* This is a bulleted list
    * An inner item needs at least 2 spaces
1. This is a numbered list
    1. An inner item needs at least 3 spaces
* [ ] This is a task list
    * [ ] An inner item needs at least 2 spaces
```
> * This is a bulleted list
>   * An inner item needs at least 2 spaces
> 1. This is a numbered list
>    1. An inner item needs at least 3 spaces
> * [ ] This is a task list
>   * [ ] An inner item needs at least 2 spaces

List items can contain formatted content. For non-text content, each line must start with the same number of spaces as an inner list item would.
```md
* This item contains text,
    > a quote
    ### and a heading.
* This item contains two lines of text.
The second line doesn't need spaces.
```
> * This item contains text,
>   > a quote
>   ### and a heading.
> * This item contains two lines of text.
> The second line doesn't need spaces.

## Quotes
To create a block quote, add `> ` to each line.
```md
> This is a quote
```
> This is a quote

Like list items, block quotes can contain formatted content.
```md
> This quote contains some text,
> ```rust
> // some code
> fn main() { println!("Hello, world!"); }
> ```
> ### and a heading.

> This quote contains two lines of text.
The second line doesn't need added characters.
```
> This quote contains some text,
> ```rust
> // some code
> fn main() { println!("Hello, world!"); }
> ```
> ### and a heading.

> This quote contains two lines of text.
The second line doesn't need added characters.

## Alerts
To create an alert, add one of 5 tags to the first line of a quote: `[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, or `[!CAUTION]`. An alternate title can be added after the tag.
```md
> [!NOTE]
> This is a note.

> [!TIP]
> This is a tip.

> [!IMPORTANT]
> This is important.

> [!WARNING]
> This is a warning.

> [!CAUTION] Caution!!!!!
> This is a caution.
```
> [!NOTE]
> This is a note.

> [!TIP]
> This is a tip.

> [!IMPORTANT]
> This is important.

> [!WARNING]
> This is a warning.

> [!CAUTION] Caution!!!!!
> This is a caution.

## Tables
A table is written with `|`'s between columns and a row after the header row whose cell's contents are `-`'s.
```md
| Style         | Syntax              | Example           |
|---------------|---------------------|-------------------|
| emphasis      | `*emphasis*`        | *emphasis*        |
| strong        | `**strong**`        | **strong**        |
```
> | Style         | Syntax              | Example           |
> |---------------|---------------------|-------------------|
> | emphasis      | `*emphasis*`        | *emphasis*        |
> | strong        | `**strong**`        | **strong**        |

## Code
A code block is wrapped by two lines containing three backticks. A language can be added after the opening backticks.
```md
    ```rust
    // some code
    fn main() { println!("Hello, world!"); }
    ```
```
> ```rust
> // some code
> fn main() { println!("Hello, world!"); }
> ```

You can also create a code block by indenting each line with four spaces. Indented code blocks cannot have a language.
```md
    // some code
    fn main() { println!("Hello, world!"); }
```
>     // some code
>     fn main() { println!("Hello, world!"); }

## Thematic Breaks
A thematic break is written with `***`, `---`, or `___` and shows a horizontal line across the page.
```md
***
---
___
```
> ***
> ---
> ___
"