# Appearance and performance

The editor should look like the **rest of the app**: same palette, dark/light. Especially on Lockbook Desktop, which shares the Rust UI. On macOS, iOS, and Android the surrounding chrome is a **different design system**, so some discrepancy is inevitable. The aesthetic is **minimal, not flavorful**, so it usually still looks at home on whatever client.

Huge notes, the readable column, and stay-put are [layout](layout.md). CJK input is [ime](ime.md). Syntax highlighting quality is [identity](identity.md) / [blocks](blocks.md). This note is type, color, motion, cost, and UI-thread discipline.

## Type and icons

Faces are **chosen per platform** to look and feel great there (SF on Apple, Noto as the bundled fallback, etc.). Spacing and size were averaged from examples that looked good on that platform — not a sacred grid.

Users have asked for **different fonts** casually. Not important right now.

**Respect OS text size**, especially on **iOS**, where older users set a large size to read and want the app to follow automatically.

We bundle some fonts as a **fallback**. For **CJK**, use **fonts that shipped with the system** — don’t distribute those families; people already have the ones they want (and not the ones they don’t).

**Phosphor** is replacing nerd fonts; that migration is underway.

Missing italic+mono+bold faces are an engine gap ([inlines](inlines.md)), not a “don’t nest” rule.

## Color and the composer

App palette, including syntax colors that must still work in dark/light.

Design-system wrinkle: **colored type needs a stark background** to hit accessibility. An off-color (tinted) background is fine for black and white, but colors often don’t show. Align composer and editor with that, rather than putting colored markdown on a washed chip.

## Motion

Feel **normal**. Scroll in points, caret behaves, no syntax-reveal jumping ([reveal](reveal.md)). “Reduced motion” is not a concept we’re using.

## Fast enough

Hardware should not do **unnecessary work**. Fast enough is **as fast as it can feasibly be**. Almost nobody is complaining (one person on a very old laptop).

We do **not** want a 100K cutoff or a thumb that drifts under you ([layout](layout.md)).

**Parse every frame** is a problem. Cache keys have bitten us (an optimization that helped an M3 with fat memory **hurt** someone else’s machine). That’s architectural debt. Don’t freeze cache keys, the syntax denylist, or the debug FPS overlay — details.

**No hanging the UI thread.** Loading belongs on background threads. In developer builds, a frame that takes too long **crashes** so we notice. Sometimes we hit that and do or don’t have time to fix it in the moment. The discipline stands.

## Jobs

**Note, composer, show:** same palette and type language. Composer/editor alignment on colored-type-on-background is the one known design gap. Show (chat) is still Lockbook type, not a third face.

## Not this note

- `MdLayout` magic numbers, TextMate theme file, nerd-font codepoints.
- 60 fps as a number. Smooth and no wasted work is the bar.
