# IME
The target is **gorgeous, native, and standard** on each platform — Notes/Bear on iOS, the usual notes apps on Android, desktop chords on a hardware keyboard. In practice that means following subtle, poorly documented patterns so the system can draw the caret, loupe, selection handles, and menus for us. As this is an expensive contract to maintain, we aim for one implementation across the text stack — note, composer, single-line, and anything else editable.

Do what feels native on that device for focus:
* On **iPad**, a sidebar stays up and you might switch notes and keep typing, so staying first responder makes sense.
* On **iPhone** there is hardly anything else on screen while the keyboard is up besides back and dismiss-keyboard.

## Features
On **iOS**: autocorrect, swipe-to-type, dictation, and emoji from the system picker. On **macOS**: dictation is desired. On **iPad**: first class hardware keyboard support with desktop-like shortcuts while maintaining drag-scroll and other touch platform behaviors. On **Android**: similar classes of system typing (autocorrect, swipe, dictation).