# Appearance and performance
The editor should look like the rest of the app: same palette, dark/light, especially on Lockbook Desktop, which shares the Rust UI. On macOS, iOS, and Android the surrounding chrome is a different design system, so some discrepancy is inevitable. The aesthetic is minimal, not flavorful, so it usually still looks at home on whatever client.

## Type
Faces are chosen per platform (SF on Apple, Noto as the bundled fallback). Respect OS text size, especially on mobile, where users with poorer vision set a large size to read and want the app to follow automatically.

We bundle some fonts as a fallback. For CJK, use fonts that shipped with the system — people already have the ones they want. Phosphor for icons. Missing italic+mono+bold faces are a non-essential gap ([inlines](inlines.md)).

## Color
App palette, including syntax colors that must still work in dark/light. Colored type needs a stark background to hit accessibility; an off-color (tinted) background is fine for black and white, but colors on off-tone backgrounds don't meet APCA accessibility targets.

## Fast enough
Hardware should not do unnecessary work. Fast enough is as fast as it can feasibly be — after all, what amount of waste is the correct amount?

No hanging the UI thread. Loading belongs on background threads. In developer builds, a frame that takes too long crashes so we notice. This is subject to a threshold that we tune down as we improve.
