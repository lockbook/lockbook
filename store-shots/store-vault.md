# Store screenshot vault

Working document. Nothing here is approved except what has been settled in chat. Items marked **proposed** are mine to be reacted to. Items marked **missing** need Travis's own words or facts, written from scratch, not refined from a draft.

## Settled in chat

- The account is based on Travis's real vault, flattened and fictionalized. Real structure, invented specifics.
- Persona: a travel journal written after the fact, with the rest of an ordinary life around it.
- Light mode, default theme, brand colors.
- Sensitive folders exist by title and are never the open note: journal, health, api-keys.
- Top-level folders: journal, finances, food, health, trips, work, books, crisis-prep, api-keys, plus loose notes ideas and to-watch. Home and people dropped.
- A second reader is in mind: the preparedness-minded privacy person. The sailing note's distress systems and the `crisis-prep` folder speak to them by title. Books stay neutral.
- The trip is Croatia: family, a week of sailing lessons, Dubrovnik, Split, Zadar, and other places.
- The hero note is Dubrovnik with the dome photo at the top.
- Three photos: dome (hero), cat (in the trip folder; the everything shot opens the set reconciliation paper instead, settled 2026-11-02), sea (sailing note).
- Share counterparts are `samantha` and `jordan`, showcase accounts created 2026-11-02 on the real server (`sam` and `samuel` were taken; jordan is the gender-neutral one). It is a group trip, so both have the trip folder. `john` was a made-up name that turned out to be a stranger's real account; the croatia share to it was revoked the same day.
- Keys are `api-keys`.
- No food photography.

## Tree (proposed)

```
api-keys/
books/
crisis-prep/
finances/
food/
health/
ideas.md
journal/
to-watch.md
trips/
  croatia/             shared with samantha and jordan
    croatia.md         overview, links to each city
    dubrovnik.md       hero
    split.md
    zadar.md
    sailing.md         sea photo
    packing-list.md    checkboxes
    dome.jpg           the hero photo (renamed from dubrovnik.jpg 2026-11-02 so a search for dubrovnik matches prose, not the image path)
    cat.jpg
    sea.jpg
work/
  lockbook/
    onboarding.md
    release-process.md
    whats-the-difference.pdf   Eppstein, Goodrich, Uyeda, Varghese, SIGCOMM 2011
```

Settled: `work/lockbook` stays. A Lockbook developer is on the cover of the Store page on purpose. `api-keys` is a folder.

Contents of the set-dressing folders (proposed, titles only, never opened):

```
api-keys/
  aws.md
  cloudflare.md
  openai.md
books/
  fooled-by-randomness.md
  outliers.md
  staff-engineer.md
  the-art-of-gathering.md
  the-red-queen.md
crisis-prep/
  crisis-prep.md
  case-studies.md
finances/
  tax-returns/
  w2s/
  budget.md
health/
  insurance-card.pdf
  vaccinations.md
journal/
  2026-10-31.md
  2026-10-30.md
  2026-10-27.md
  2026-10-25.md
```

Books are five from the real list, chosen to be neutral on a store page. The ones left out are the political and religious titles, which are yours to put back if you want the cover to be that honest. Health titles are invented, nothing genuine. Finances are the real folder names. Search for `dubrovnik` now hits `croatia.md`, `dubrovnik.md`, and a journal entry.

## Which shot draws on what

All seven captions settled 2026-11-02. Finals in `store-shots/final/`, numbered in strip order; `strip.png` is the contact sheet.

| Shot | Open file | What else is visible |
|---|---|---|
| 1 Hero | `trips/croatia/dubrovnik.md` | tree with `trips/croatia` expanded, people icon on it. Second pass 2026-11-02: title above the photo, no blank line between them, one blank line before the prose; sidebar 320. Caption: The best place for your **notes**. |
| 2 Drawing | `euler.svg` (Euler napkin, renamed from e.svg Travis drew 2026-11-02, replaces the sailing diagram) | sidebar 320, 1440×712 window so its bottom edge is the slide's bottom edge and the drawing toolbar shows; FIT from the zoom-stops popover then the 120% stop, a small wheel pan to even the margins, and enough frames for the sync status to settle; strokes recolored in the file: blue to brand #207FDF, red to black (settled 2026-11-02) (the pen palette's own red and blue are 218,21,21 and 62,130,230, same on canvas-diagrams). Caption: **Sketch** on an infinite canvas. |
| 3 Everything | `work/lockbook/whats-the-difference.pdf` | second pass 2026-11-02: sidebar off, four zoom-out clicks take the page from 192% (page width) to 152% (Travis asked for 150; the viewer steps by 10 from 192), then a 3740-point scroll to the top of page 4; tree scrolled so the croatia folder (notes, photos, drawing) and work/lockbook (notes and the paper) both show; 900-point window; viewer scrolled 3250 points to the encode and subtract figures on page 4. Caption: Your home for **everything**. |
| 4 Links | `trips/croatia/croatia.md` | second pass 2026-11-02: sidebar off, sidecar open (outline with three headings, four backlinks), 1440×1100 window, caret in the first paragraph, pointer just inside `[[packing-list]]` at the start of the course section's last line (470, 475): the card opens left over the gutter (the 2026-11-02 `style/tip.rs` rule: the side that hides less of the text column) and previews the checklist whole, chosen over the Dubrovnik card because the carousel has no other to-dos and a card that leads with text reads as a preview of another note. Caption: Your notes, **connected**. |
| 5 Together | `trips/croatia/packing-list.md` | second pass 2026-11-02: a 1100×800 window placed at x 853, the left third line, so the sidebar and the share sheet fill the right two thirds and the editor bleeds off the right; sidebar on the Shared with me pane showing `restaurants` from `jordan`, `ferry-times` from `samantha` and three pending shares from `travis` (`budget-2026`, `lb-maintainers` expanded to docs/meetings/postmortems/onboarding/release-26.8/weekly-sync, `reading-list`; the two notes live in travis's `/test/store-shots/`), share sheet on `trips/croatia` with you (owner), jordan and samantha (can edit) and `travis` typed so the summary line reads "croatia will be shared with travis with edit access"; an iPhone on the left (`store-shots/phone` frames `out/ios-packing.png`, shot on the lb-iphone sim seeded with a copy of jane's account at `~/.lockbook/jane-ios-shot`, 620 px wide at (116, 440) in the left third, bleeding off the bottom) showing the same packing list, so sharing and sync read in one frame. Driver gained `expand-id UUID` for pending shares. Caption: **Shared** only by invitation. |
| 6 Search | content search for `maritime` with the overview note open behind, sidebar off (2026-11-02: `dubrovnik` put raw image markdown in the snippets, `sailing lessons` had one hit, `maritime law` only half-matched the table) | four hits; ArrowDown ×3 walks the flat result list to the sailing note's table snippet, which scrolls the preview to the rendered Terms table and highlights the match there. The table's terms are inline code and the drawing embed is gone from `sailing.md` (both 2026-11-02); the phrase `sailing lessons` was also put into `croatia.md`, `sailing.md` and `journal/2026-10-25.md` along the way; dome backdrop. Caption: Remember less. **Find** more. |
| 7 Yours | account key screen over dubrovnik.md | phrase masked after four words. Caption: You hold the **keys**. |

Candidates for 8 to 10: a table note in finances, a Mermaid diagram in work, a phone-plus-desktop composition for every device.

## The hero note: `dubrovnik.md`

Shape (settled 2026-11-02): title, the dome photo directly under it, a blank line, two paragraphs in Travis's words with *usually* in italics, the alleyways sentence in bold, and 😵‍💫 closing the note (the ZWJ sequence renders as one glyph). One job: be a note you'd want to have written.

Draft from Travis's words, not yet approved:

```markdown
# Dubrovnik

![](dubrovnik.jpg)

Too often I'm disappointed by a historic city, finding that it just feels old. Not here. The old town is a maze of cozy alleyways that make you want to be lost in them, and behind every door is a storefront or some adventure waiting to be had. There was hardly any bad food in the whole city.

We walked the walls and learned about the siege, first from the guide and then at the hilltop museum, which if I understand correctly was host to the final stand of the country's defenders. The terms from a week of sailing lessons were still swirling around in my head, but I tried to put the sea out of mind and turn my attention to the land.

## Worth doing again

- Getting lost in the alleys on purpose
- The second-story pub my brother found
- The wall tour, then the museum on the hill
- Dinner anywhere
```

Open: "Not here" versus a fuller clause. Whether "my brother" stays.

## The overview note: `croatia.md`

Purpose: the links shot. Travis's words, settled 2026-11-02, with the coast section placed second and a drafted closing line on the course section (`The [[packing-list]] says I remembered the ND filters and forgot the deck shoes, which is about right.`) as the hovered link, his to rewrite:

```markdown
# Croatia
## Sailing course
The first few days of the trip, we took sailing lessons. I expected to spend more of it... *sailing*? Instead, we spent most of the time in the classroom or studying, which was unexpectedly still super cool. We learned about different maneuvers, the anatomy of an engine, and emergency signaling. I now know more about Croatian maritime law than anyone I've met. More on that in [[sailing]].

We also made a few friends from all over Europe, including a retired football player, a chain-smoking German, and a successful Polish tech entrepreneur. There's nothing like a cold radler at the marina pub after a morning in the classroom and afternoon on the water — and all the better in good company.

The [[packing-list]] says I remembered the ND filters and forgot the deck shoes, which is about right.

## The coast
Then the cities, north to south. [[zadar]] first, then [[split]], and we saved the old town for last. Of the three, it was [[dubrovnik]] that left the romantic impression.

## Photography
My 70-200 put in the work on the sailboat, grabbing candids from across the deck with ease. I got some dramatic pics of my brother leaning up against the mast against a sky of picturesque cumulus clouds. I'm a sucker for some good clouds.

I also took the new drone for a test run: a timelapse drifting over an inlet towards the bay. On one side, the inlet has boats tethered all along a road that has the coast on one side and dense village houses and businesses on the other. It was a successful maiden voyage, and since the drone color grades well automatically, it's already up on my socials.
```

Backlinks for the panel (drafted, his to rewrite): `zadar.md` "First stop on the coast, after the course. Part of [[croatia]].", `split.md` "The middle stop. Part of [[croatia]].", a closing line "The first week of [[croatia]]." on `sailing.md`, and "For [[croatia]]." on `packing-list.md`; each item leads with an emoji (👕 🩳 🧴 📷 🚁 👟 📓 🔌), `==Sunscreen==` is highlighted so the card shows a highlight, and no trip note has a blank line between a heading and its body (both 2026-11-02). The hero note stays untouched.

## The sailing note: `sailing.md`

Purpose: carries the sea photo and the trip's tables (the SART/EPIRB drawing embed was removed 2026-11-02; the drawing file stays in the folder).

Known: daily classes, mostly classroom, a lot of law. A group from across Europe. The SART versus EPIRB distinction was subtle and interesting. Travis came away preferring the rocket parachute flare: shortest burn, but airborne, so visible over the horizon, day and night.

Draft from Travis's words, not yet approved:

```markdown
# Sailing

![](sea.jpg)

Daily classes, and far more classroom than boat. Most of it was law, so I now know more about Croatian maritime law than most people I've met. The group was from all over Europe: a very cool German man who smoked constantly, and someone I'm fairly sure was a retired footballer.

The difference between a SART and an EPIRB is subtler than it sounds. The beacon tells a satellite where you are so a rescue center can send someone. The transponder makes you show up on the radar of whoever is already nearby, and nearby is who actually pulls you out. I came away convinced the rocket parachute flare is the signal I'd want in a real emergency. It burns the shortest of the three, but it's the only one that gets above the horizon, and the only one that works on someone who isn't already looking for you.

## Terms

| Term | Meaning |
|---|---|
| GMDSS | Global Maritime Distress and Safety System |
| DSC | Digital Selective Calling, VHF channel 70 |
| CH 16 | VHF voice distress channel |
| EPIRB | Emergency position-indicating radio beacon |
| SART | Search and rescue radar transponder |
| MRCC | Maritime Rescue Coordination Center, 195 Rijeka |

## Flares

| | Count | Burns |
|---|---|---|
| Hand flare | 6 | 60 seconds |
| Rocket parachute | 4 | 40 seconds |
| Smoke signal | 2 | 3 minutes |
```

## The packing list: `packing-list.md`

Purpose: the together shot and the checkbox beat. Short.

Known: packed light, clothes for the weather and the vibe, the camera drone.

Draft from Travis's words, not yet approved:

```markdown
# Packing list

- [x] Linen shirts
- [x] Swim trunks
- [x] Sunscreen and a hat
- [x] Camera
- [x] Drone, spare batteries, ND filters
- [ ] Deck shoes
- [ ] Notebook for the course
- [ ] Charger for everything
```

The unchecked items are what samantha or jordan is plausibly adding in the together shot. Open: whether the drone line is too much gear-talk for the one list a visitor sees.

## The other city notes: `split.md`, `zadar.md`

Never opened. Titles only unless the hover card or search shows a first line.

## Food

Settled: no dish from the trip, so no food hit in search. The folder holds recipes Travis meant to learn and never did, titles from the real vault:

```
food/
  chicken-piccata.md
  garlic-butter-salmon.md
  chicken-stir-fry.md
```

Never expanded unless the sidebar needs rows.

## The journal

Never opened. Dated titles, recent. One entry should contain the search term.

**Missing:** nothing, unless you want the dates to mean something.

## The drawing

Three options on the table. New and recommended: a SART versus EPIRB diagram in `trips/croatia/sailing.svg`. A boat, a satellite, a rescue center, a nearby ship with a radar sweep, two arrows. Technical, on the vault's theme rather than the product's, and self-explanatory at thumbnail size. The drawing shot then lives inside the same story as every other shot.

The earlier two. Authentic: a distributed-systems sketch, such as how two devices' edits to one note merge. Explainer: how Lockbook keys work, as you might draw for the docs. My recommendation is the authentic one, because it sits in the work folder honestly and a two-lanes-merge shape reads at thumbnail size.

Settled: Travis drew `sart-vs-epirb.svg` by hand on 2026-11-02. Left: a SART pinging a boat. Right: an EPIRB reaching a satellite and a lighthouse. Labels spell both acronyms with red initials.

`sart-vs-epirb-tight.svg` beside it in the vault root is the working copy: labels centered under their scenes with even row spacing, overall 1375 by 882 units, stray stroke removed, arcs in brand blue (#207FDF), acronym initials in brand red (#DF2040), the rest of the label text black, device squares black. Drawn strokes untouched.

Open: Travis's look on the iPad, then which copy becomes `trips/croatia/sailing.svg`.

## Rules on screen (proposed)

- No name anywhere except `samantha` (shares `ferry-times.md` with jane), `jordan` (shares `restaurants.md`), both with the trip folder, and `travis` as a sharer and the typed invitee on the together slide. Both are accounts we hold; the server rejects made-up names.
- No pronouns for anyone.
- Nothing in journal, health, or api-keys is ever the open note.
- Folder names are one lowercase word. `api-keys` is the one hyphen.
- Dates are real and recent.

## Rig

- Driver: `clients/desktop/examples/shots.rs` on this branch, built with `cargo build --release -p lockbook-desktop --features win-chrome --example shots`. Release, so the canvas debug overlay is off and the zoom popover's debug assertion does not fire.
- Accounts, all on the real server, each one's key living only in its folder: jane at `~/.lockbook/jane` (the showcase vault, `LOCKBOOK_PATH=~/.lockbook/jane`, light mode pinned in her settings), samantha at `~/.lockbook/samantha`, jordan at `~/.lockbook/jordan` (both created 2026-11-02 to share with jane), a seeded copy of jane for the iPhone sim at `~/.lockbook/jane-ios-shot`, and Travis's own account at `~/.lockbook/cli` and `~/.lockbook/egui`. `lockbook account export` in a folder reveals that account's key when a device needs it. Delete `~/.lockbook/jane/ws_persistence.json` before each scene or tabs carry over.
- Scenes and compositor: `store-shots/` in the worktree (git-excluded). `store-shots/compose OUT W H #hex SRC x y scale` places a render on a flat canvas. Scratchpad copies were lost to the hourly /tmp sweep on 2026-11-02; nothing lives there now.
- Backdrop (settled 2026-11-02 evening, one for every slide): `store-shots/ambient out/bg/dome-warm.png 2560 1440 assets/dubrovnik.jpg 120 1.9 0.10 0.06 90` blurs the dome photo at saturation 1.9 with only a 0.10 white lift so the terracotta stays warm, the top-weighted lift still solved so the caption band reaches APCA Lc 90; then `store-shots/slate IN OUT 0.25 0.5` fades a 50% gray up from the bottom edge over a quarter of the height so the window stands on a floor. Earlier: ambient soft at lift 0.45 (read beige), sea and cat photos (too cool), a 55% slate (took over). The compositor adds the window shadow (following image alpha, so framed phones keep their corners) and a 10% darkening over the bottom half.
- Captions: `compose … -- "text with a *starred* word" 96 1280 150` sets one headline in Segoe UI Semibold (`CAPTION_FONT=store-shots/fonts/seguisb.ttf`, copied from the Windows VM) with the starred word in accent blue on a 12% tint pill, the way lockbook.net's hero sets its word.
- Key phrase: `store-shots/mask IN OUT 1090 600 2180 740 4 9` finds the words in the plate and covers all but the first four with white pills before composing.
- Scene commands: size, ready, sidebar on|off, sidebar-width W, pane files|recents|shared, expand PATH, expand-id UUID (pending shares), open PATH, share PATH, sidecar, search [path|content], type, key [N], click X Y, move X Y, wheel DX DY, settings, reveal, blur, frames N, dump, shot NAME. `blur` marks the window unfocused so the editor paints no caret (the app's own background-window rule); every scene with a note open ends with it before its shot, and the keys scene clicks into the body first so the title shows no `#`.
- Known: the image viewer fits a photo to the pane, so a wide photo sits with bands above and below; the everything scene clicks the zoom island's plus twice (`click 1402 77` at the 374 sidebar) to take the cat past every edge at 120%.
- Known: `pane files` on the already-active pane collapses the sidebar. A typed shortcut goes to the focused note, so open search with the `search` command, never a key chord.

## Shipped

2026-11-02 evening: the seven finals and the locked copy went live as Partner Center submission 5 for product 9P5BJXV6044Z (Store listings Updated, packages/pricing/properties unchanged), submitted for certification through the browser pane; Travis dropped the PNGs in by hand since the built-in browser cannot attach files and the `lbdev` Store tool's Azure credentials are not on this machine. The same copy sits in the vault at `/test/lb-maintainers/store/microsoft-store-listing.md` beside the stills, and in the repo's `fastlane/metadata/windows/en-US/images/desktopScreenshots/` (staged, not committed) with one caption `.txt` per still.

