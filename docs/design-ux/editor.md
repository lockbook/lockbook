# Editor
When a user has a document selected, that file is open for editing in an editor. The name of the document is displayed above the document and is editable (editing renames the document).

An icon near the document's name indicates one of the following statuses:
* the document is presented exactly as it appears on disk
* all changes to the document have been merged into the version on disk
* there are unsaved changes to the document

At most 5s after the user completes an edit on this document, the document is saved. At most 60s after the user completes an edit on any document, a sync occurs and any open documents are reloaded from disk.

On platforms that present an editor side-by-side with navigation, the navigation is collapsible.

## Markdown
At most 1s after the user completes an edit, the document is syntax-highlighted and the active style is set in accordance with the syntax highlighting at the cursor's position. The syntax highlighting makes links clickable to open the link with the user's default browser.

The editor non-intrusively spell-checks the user's document.

The editor supports undo and redo, including:
* text added or removed
* text pasted or cut

## Drawing
There is a toolbar at the left of the editor which features:
* pen color controls (8 colors from drawing theme or, if drawing has no theme, 8 colors from app theme)
* pen size control (thickness range is from 1 pixel to 20 pixels at full pen pressure)
* a stroke eraser
* an erase-all
* undo and redo
    * strokes added and removed
    * modifications made by platform-specific features (e.g. iPadOS's "duplicate")
* pan & zoom
    * cannot pan to where none of the canvas is visible
    * cannot zoom out more than 5x (20% zoom)
    * cannot zoom in more than 20x
* a touch input toggle (off by default)

Drawings are rendered on a fixed-size canvas. By default, the width of the canvas is exactly the screen's width. The height of the canvas is 2x the width. The non-canvas area is rendered as a solid neutral color distinct from the canvas background color. The canvas background color is determined by the system's light/dark theme setting and drawing theme's white/black colors. In dark mode, the white and black colors in the drawing are swapped (including in the toolbar).
