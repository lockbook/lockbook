# IME (how the system types into us)

This is in a sorry state. The editor feels bad on iOS even in basic ways. We have a lot of open issues and are still learning how these system text APIs actually work. The product direction is simple: **gorgeous, native, standard** on each platform — Notes/Bear on iOS, the usual notes apps on Android, desktop chords on a hardware keyboard. In practice that means following careful, poorly documented patterns so the *system* can draw the caret, loupe, handles, and edit menu for us. If we lie to those APIs, it just feels janky.

Get it solid for current users first. Then grow.

We do not need this note to teach the protocols. We need it to say what must *feel* true. Mechanism lives in the hosts.

## One contract

Ideally **one keyboard contract for the whole text stack** — note, composer, single-line, and anything else editable. Non-editable jobs (show, read-only as “not a field”) do not summon a keyboard. Common sense; not a flag table.

Focus while the editor is on screen is **not a hard requirement** either way; it has not been thought through that hard. Today the editor tends to stay first responder: tap other chrome and you can still type. That makes more sense on **iPad**, where a sidebar stays up and you might switch notes and keep typing. On **iPhone** there is hardly anything else on screen while the keyboard is up besides back and dismiss-keyboard, so “stay focused” vs “tap away” barely shows up. Do what feels native on that device; don’t freeze today’s default.

Hiding the software keyboard while reordering a list is a **nice-to-have**. We do not do it, and we are already over budget on this complexity. Not a requirement.

## What must work (current users)

On **iOS**: autocorrect, swipe-to-type, dictation, and emoji from the system picker. These are not optional.

On **Apple with a hardware keyboard** (iPad, and a phone with a keyboard): first-class. Shortcuts should feel **desktop-like** (the full chord set). Touch still applies: a drag across the screen **scrolls**, it does not start a drag-select.

macOS emoji input may already work; not sure. **macOS dictation** is desired and unimplemented / unprioritized.

Android: the same *class* of system typing (Gboard-class autocorrect, swipe, dictation) wherever we support those input modes. The bar holds everywhere we take that work on — we do not keep a lower secret bar for “the other OS.”

Desktop IME beyond that (pinyin on a laptop, etc.) waits on CJK.

## Uncommitted text, later CJK

We do not have CJK yet. Those fonts are too large to bundle; when we do this we will use **system fonts** ([appearance-perf](appearance-perf.md)). We want this: those markets are large, and some (China in particular) are politically conducive to the app. It is a matter of time. **Not until the current app is solid.**

Whenever we do composition (CJK, and any other marked/uncommitted text), we are open to whatever design makes sense. Likely: **never save uncommitted content**; **do show it**; the editor-side of that rendering lives in the editor.

Same-frame replacements from a keyboard (autocorrect deleting a word and inserting another) are already [document-model](document-model.md). Valid UTF-8 in the file is non-negotiable. Hosts that speak another encoding get a translation layer.

## Native chrome

Caret, loupe, handles, edit menu: the system will do them if we play along. That is the product. It is not free. [Selection](selection.md) is the feel; this note is “the OS is allowed to own that chrome.”

## Not this note

- Tokenizer internals, `UITextInput` / `InputConnection` method lists, marked-text stubs in the FFI.
- A promise that current iOS typing is fine. It isn’t.
- Keyboard-hiding during reorder.
- CJK as a current milestone.
