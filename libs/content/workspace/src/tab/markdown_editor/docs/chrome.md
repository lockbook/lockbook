# Chrome
Toolbar, find, completions, and menus around the document. These may or may not be available in different contexts — the chat composer, for example, may hide the toolbar.

## Toolbar
Users want an option to hide it. Customize which buttons is a mobile-only experiment with some positive feedback: it teaches what a button means without trying it on the document and it creates focus so people don't scroll a mile of icons.

The heading button cycles levels. Quotes still need an entry. Which buttons exist is allowed to grow.

In a table the block styles give way to the table set: insert and delete for rows and columns, the column's alignment, and on touch the cell-to-cell keys a touch keyboard lacks ([blocks](blocks.md)). Moving a row or column is a drag, as it is for list items ([reorder](reorder.md)). On desktop the row and the column under the pointer also each show a handle on the table's border: drag it to reorder, click it for the same actions as a menu, with move among them; a second click or a key puts the menu away.

## Find
Enter a search term, regex optional, count & navigate matches, replace one, replace all. On iOS a native search button opens this.

## Completions
Emoji on `:`. Files/wiki/images on `[` / `[[` / `![` — they pick the link type by how you start ([links-images](links-images.md)). May insert the emoji character instead of the shortcode ([dialect](dialect.md)).

You should be able to ignore them while typing with no side effect and also submit easily. They shouldn't show unless you probably intend to invoke them or at least write the kind of syntax they cover. Those conflict when Return submits instead of inserting a newline (acceptably).

Completions are differentiators in the composer on mobile.

## Menus
iOS / Android: native system menus (same bar as [ime](ime.md) / [selection](selection.md)). macOS: native eventually; the Lockbook desktop design system is a workable placeholder. Link Open / Edit / Copy is a natural home ([rich-inlines](rich-inlines.md)), as are copy-as-plain / copy-as-markdown ([edits](edits.md)).
