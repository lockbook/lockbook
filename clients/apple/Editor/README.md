#  Editor Suite

Top level `EditorView` is just a bridge from SwiftUI components to UIKit and AppKit components.
There's some strange behaviors these `UITextView` and `NSTextView` have, like requiring `autoresizingMask` to be set
otherwise they won't work properly. For now this is handled here. Perhaps these things are better off in the next layer.

The `CustomUITextView` and `CustomNSTextView` are, as expected, overriden UIKit/AppKit components. Currently they 
only override functions related to the insertion of text. These components reach deep inside the next layers, describe
what is about to happen, and whether there are any alternate actions the editor should take. This is how most nice 
behaviors associated with lists happen.

Storage, is an implementation of `NSTextStorage`. This is where style information is currently stored. This class doesn't
really work the way you'd expect it to. The common places we've tried to calculate information causes strange behaviors 
related to the cursor and styling. If you get some details wrong you'll get some very strange looking characters, scrolling
going crazy or cursor location jumping around. It's at this stage where we are clever about how we decide what gets 
restyled and what doesn't. When you're using a traditional text editor, if you type text in the middle of a bold region
that text will also be bold. We exploit this property to determine if a restyle is needed. Parsing markdown is cheap,
updating the document is expensive. So if the overall structure of the tree has not changed, a restyle is not needed. This
is a great strategy and works really well, but it's likely implemented at the wrong layer because of the following quirks.
The layer above this one (`XTextView`) does try to manage styles as well. It will have it's own version of what the current
active style is, and things like copying and pasting will duplicate these styles. Because of this we have to keep track of
whether we're the ones editing styles or style information is coming from somewhere else. Getting this wrong at a critical
moment like loading a document causes the document not to show up. Or it could cause our document to forget about the style
associated with the current region. Projects like FSNotes implement their logic at the level above this one. I'm pretty 
sure implementing at that level would still result in the desired autoexpanding of styles. So a future code refactor would
put this information at that layer too. See [this blog post](https://christiantietze.de/posts/2017/11/syntax-highlight-nstextstorage-insertion-point-change/) regarding `NSTextStorage`'s strange behaviors.

`Storage` relies on `Indexer`, `Parser`, `Styler` and `TypeAssist` for everything described above. The parser, everytime the document
is updated will recompute the tree, it will calculate style information using `Styler`. While doing so it will also inform
`TypeAssist` of nodes it may care about (for now, only `List`s).

`Parser` uses `Down` to generate an `AST`, `Down` uses `cmark`, `Down` seems to be unmaintained, and `cmark` has bugs (see
external tag on gh). Parser will populate the various components with info.

`Indexer` handles conversions between the various encodings at play between utf-8 row cols, utf-8 indexes, and utf-16. These
are expensive and we require O(n) memory to try to speed them up. In a pure rust implementation we could parse and return
indexes in the same encoding.

`Styler` is populated with info about each node and it's parent. It's an inheritence like structure, where each node
overrides a common ancestor, only overriding specific properties it cares about. For instance `strong` will only override
the weight property. The keen eye would wonder why we don't just use `addAttribute` instead of managing this ourselves.
And it's because there's a handful of attributes that are mutually exclusive from an API standpoint (all the font properties)
are one property, but are not actually mutually exclusive for the user (bold and font size for instance). Style attribs are
"finalized" when the document has been parsed and each node refers to it's parents for attributes it doesn't care about.

`TypeAssist` receives info about where list nodes are at parse time, and at type time can answer the question: "if a \n or 
\t was inserted at location X how should you behave".

I believe everything `Indexer`, `Parser`, `Styler` and `TypeAssist` can happen once in rust and be reused on all the
platforms. It'll likely be faster and higher quality. We'll look into that once we have further knolwedge about other
platforms.

Furthermore I am curious about building such a component in a webview + wasm. 
