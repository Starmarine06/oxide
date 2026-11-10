---
name: Oxide Offline Player
description: The album booklet opened at the lyric sheet, printed in each album's own colour.
colors:
  ground: "#111216"
  stock: "#16171c"
  raised: "#1d1e24"
  rule: "#2a2b31"
  rule-strong: "#3b3c43"
  ink: "#ebe5da"
  ink-2: "#b7b0a5"
  ink-3: "#8f897f"
  paper-white: "#ffffff"
  album: "#6b5a48"
  album-ink: "#9f917e"
  signal-offline: "#c9a24a"
typography:
  display:
    fontFamily: "Schibsted Grotesk, Noto Sans Malayalam, Noto Sans Tamil, Noto Sans Devanagari, Segoe UI Variable Text, Nirmala UI, Segoe UI, system-ui, sans-serif"
    fontSize: "40px"
    fontWeight: 800
    lineHeight: 1.05
    letterSpacing: "-0.025em"
  lyric:
    fontFamily: "Schibsted Grotesk, Noto Sans Malayalam, Noto Sans Tamil, Noto Sans Devanagari, Segoe UI Variable Text, Nirmala UI, Segoe UI, system-ui, sans-serif"
    fontSize: "26px"
    fontWeight: 650
    lineHeight: 1.25
    letterSpacing: "-0.01em"
  headline:
    fontFamily: "Schibsted Grotesk, Noto Sans Malayalam, Noto Sans Tamil, Noto Sans Devanagari, Segoe UI Variable Text, Nirmala UI, Segoe UI, system-ui, sans-serif"
    fontSize: "26px"
    fontWeight: 800
    lineHeight: 1.25
    letterSpacing: "-0.015em"
  title:
    fontFamily: "Schibsted Grotesk, Noto Sans Malayalam, Noto Sans Tamil, Noto Sans Devanagari, Segoe UI Variable Text, Nirmala UI, Segoe UI, system-ui, sans-serif"
    fontSize: "18px"
    fontWeight: 700
    lineHeight: 1.45
    letterSpacing: "-0.01em"
  body:
    fontFamily: "Schibsted Grotesk, Noto Sans Malayalam, Noto Sans Tamil, Noto Sans Devanagari, Segoe UI Variable Text, Nirmala UI, Segoe UI, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.45
  body-small:
    fontFamily: "Schibsted Grotesk, Noto Sans Malayalam, Noto Sans Tamil, Noto Sans Devanagari, Segoe UI Variable Text, Nirmala UI, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: "Schibsted Grotesk, Noto Sans Malayalam, Noto Sans Tamil, Noto Sans Devanagari, Segoe UI Variable Text, Nirmala UI, Segoe UI, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 600
    lineHeight: 1.45
    letterSpacing: "0.09em"
rounded:
  hairline: "2px"
  sm: "3px"
spacing:
  2xs: "4px"
  xs: "8px"
  sm: "12px"
  md: "16px"
  lg: "20px"
  xl: "28px"
  2xl: "40px"
  page: "48px"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ground}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: "0 16px"
    height: "38px"
  button-primary-hover:
    backgroundColor: "{colors.paper-white}"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    padding: "0 16px"
    height: "38px"
  icon-button:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.sm}"
    size: "36px"
  icon-button-pressed:
    textColor: "{colors.album-ink}"
  play-button:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ground}"
    rounded: "{rounded.sm}"
    size: "44px"
  play-button-hover:
    backgroundColor: "{colors.paper-white}"
  search-input:
    backgroundColor: "{colors.ground}"
    textColor: "{colors.ink}"
    typography: "{typography.body-small}"
    rounded: "{rounded.sm}"
    padding: "0 10px 0 32px"
    height: "34px"
  nav-item:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.sm}"
    padding: "8px 10px"
  nav-item-active:
    backgroundColor: "color-mix(in oklab, #ebe5da 8%, transparent)"
    textColor: "{colors.ink}"
  song-row:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    padding: "0 12px"
    height: "56px"
  song-row-playing:
    backgroundColor: "color-mix(in oklab, #6b5a48 24%, transparent)"
    textColor: "{colors.album-ink}"
  transport-bar:
    backgroundColor: "{colors.stock}"
    textColor: "{colors.ink}"
    padding: "0 20px"
    height: "76px"
---

# Design System: Oxide Offline Player

## Overview

**Creative North Star: "The Lyric Sheet"**

The offline player is a CD booklet opened at the lyric sheet. The words of the playing song are the page; title, artist and album sit above them as credits in small tracked capitals; the library is the numbered track listing printed alongside. The stock is near-black and cool, the printing is warm bone, and every song re-inks the sheet in its own cover colour, so the page changes colour with the record while the type and rules never move.

Density is calm and editorial: one family, a short fixed size scale, 1px hairline rules, corners no softer than 3px. Depth comes from the album-colour wash fading into the ground, not from panels or glass. The reader's pencil (a 1px rule in the album colour drawn under the sung line over its duration) is the signature, and the album colour is reserved for marking place: the sung line, the progress line, the playing row, pressed toggles and focus.

**Scope.** This system covers only the bundled offline player (`dist/offline.html`). The rest of Oxide's UI is YouTube Music's own page plus small injected panels that deliberately match YouTube Music (Roboto); those are not part of this system and should not borrow from it, nor it from them.

**Key Characteristics:**
- Ink-black ground, bone type, one live accent taken from the playing cover.
- Schibsted Grotesk only, in six fixed steps (12/13/15/18/26/40px), with chosen Noto faces for Malayalam, Tamil and Devanagari.
- 1px hairline rules for every division; 2-3px corners; no glass, no gradient text.
- Lyrics as the page: large, left-aligned, opacity encodes past / current / upcoming.
- Credits and table headers in small tracked capitals.

## Colors

A cool near-black stock printed in warm bone, re-inked per song by the cover's average colour.

### Primary
- **Album Colour** (`album`, default shown): not a fixed hue. It is set at runtime to the playing song's cover average (or a seeded hsl fallback from artist + title); the frontmatter value is only the idle default. A playlist page uses the average of its songs' colours as its wash. It tints page washes (26% to 8% over the ground on library views, 30% to 14% on the lyric sheet), the playing row (24%), the cover placeholder and text selection.
- **Album Ink** (`album-ink`): the album colour mixed 60/40 with warm paper (`color-mix(in oklab, var(--album) 60%, #f4ead8)`), so it stays legible on dark ground whatever the cover. Used for the reader's pencil, the progress fill, the volume fill, the playing row's title and number, pressed toggles, focus rings and the caret. The frontmatter hex is the mix of the default album colour.

### Neutral
- **Ink-Black Ground** (`ground`): the page and the inside of inputs and selects.
- **Booklet Stock** (`stock`): the sidebar and transport bar, one step lifted from the ground.
- **Raised Stock** (`raised`): empty thumbnails and cover mosaics before art loads.
- **Hairline** (`rule`): every 1px divider and control border; scrollbar thumbs.
- **Hairline, Pressed** (`rule-strong`): hover state of bordered controls and scrollbar thumbs.
- **Bone** (`ink`): primary text, the current lyric line, and the fill of primary buttons.
- **Faded Bone** (`ink-2`): secondary text: artists, albums, ledes, inactive nav, idle icon buttons.
- **Pencil Grey** (`ink-3`): tertiary text: track numbers, durations, counts, table headers, placeholders, the source line.
- **Paper White** (`paper-white`): hover state of bone-filled buttons only.

### Status
- **Offline Amber** (`signal-offline`): the 7px dot beside "Offline" in the sidebar. Its only use; it is a state signal, not an accent.

### Named Rules
**The One Ink Rule.** The only chromatic colour on screen comes from the playing record. Never introduce a fixed brand accent; anything that needs to mark place uses album ink.

**The Tinted Opacity Rule.** Hover and selected fills are bone or album mixed into transparency (bone 5% hover, 7% on icon buttons, 8% selected; album 24% playing), never new grey hexes.

## Typography

**Display Font:** Schibsted Grotesk (variable 400-900, self-hosted, latin + latin-ext + italic)
**Script Faces:** Noto Sans Malayalam, Noto Sans Tamil, Noto Sans Devanagari (self-hosted, unicode-range scoped, 500-800)
**Fallback:** Segoe UI Variable Text, Nirmala UI, Segoe UI, system-ui

**Character:** A single compact grotesk carries everything from 40px credits to 12px caps; heavy weights (800) for titles, a middle weight (650) for lyrics so long Indic lines stay open.

### Hierarchy
- **Display** (800, 40px, 1.05, -0.025em, balanced wrap): page titles and the song title in the credits.
- **Lyric** (650, 26px, 1.25, -0.01em): synced lyric lines, max 24em measure, 18px apart. Upcoming at 50% bone, sung at 28%, current at full bone, hover 78%.
- **Headline** (800, 26px, -0.015em): empty-state headings.
- **Title** (700, 18px, -0.01em): section heads ("Recently downloaded", "Playlists"). Unsynced plain lyrics also sit at 18px / 1.6 in faded bone, 46ch.
- **Body** (400, 15px, 1.45): base text; song and playlist names at 600-700.
- **Body Small** (400, 13px): artists, albums, durations, counts, inputs.
- **Label** (600, 12px, 0.09em, uppercase): credits (artist, album, "Track n of m"), table headers, the Playlists label, the lyrics source line, keycaps.

### Named Rules
**The Six Steps Rule.** Sizes come from the scale (12/13/15/18/26/40px) and nothing else; the only exception is the 64px initial on a blank cover tile.

**The Chosen Script Rule.** Every script the library actually holds gets a chosen, bundled face, never a system fallback. Adding a language means adding its Noto face with a unicode-range, not widening the fallback stack.

**The Numbers Line Up Rule.** Track numbers, durations, counts and clocks use tabular figures.

## Layout

A fixed app grid inside Oxide's 32px title bar: sidebar column (248px) beside a scrolling main column, over a 76px transport bar spanning both. Main views pad at 40px top and 48px sides (`page`), capped at 1240px; the lyric sheet pads 40px 56px and caps at 1080px. The credits block sets a 152px cover beside the title with a 28px gap, closed by a hairline, then the lyrics scroll beneath with a soft mask at top (40px) and bottom (80px) and 45vh of tail room so the current line can sit high.

Song tables use a four-column grid (number 36px, title with 40px thumb, album, time 64px), 16px column gap, 56px rows. Home shows playlists as an auto-fill shelf of 176px cards (28px row / 22px column gaps).

Responsive: at 980px and below the sidebar narrows to 208px, page padding drops to 28px, the album column, volume and clocks drop, and the credits cover shrinks to 104px. At 680px and below the sidebar becomes a top strip holding the offline line, search and a three-way nav; playlists move out of the sidebar; page padding is 20px. Below 420px shuffle and lyrics toggles drop from the transport.

Spacing steps observed in reuse: 4, 8, 12, 16, 20, 28, 40, 48px.

## Elevation & Depth

Flat by construction. Planes are separated by tone (ground, stock, raised) and 1px hairlines, never by floating surfaces. The one depth cue is the album wash: a vertical gradient from the album colour into the ground behind the page. Album art alone casts a soft ambient shadow, as a printed cover sits on the page.

### Shadow Vocabulary
- **Cover** (`box-shadow: 0 18px 40px -18px rgba(0,0,0,.7), 0 2px 6px rgba(0,0,0,.35)`): the now-playing cover.
- **Playlist header art** (`box-shadow: 0 20px 44px -22px rgba(0,0,0,.85)`): the mosaic on a playlist page.
- **Shelf card art** (`box-shadow: 0 16px 34px -20px rgba(0,0,0,.8)`): playlist mosaics on Home, which lift 3px on hover.

### Named Rules
**The Art Casts, Chrome Doesn't Rule.** Only cover art carries a shadow. Buttons, rows, inputs, bars and panels stay flat.

## Shapes

Square-cut print. Corners are 3px on every interactive surface (buttons, inputs, rows, nav items, covers, mosaics, focus rings) and 2px on small thumbnails and the volume thumb; nothing is pill-rounded. Dividers are always 1px. Playlist art is a 2x2 mosaic of its first four distinct covers, like a booklet's back panel; with fewer than four it shows one. A song with no saved cover gets a coloured tile with its initial.

## Components

### Buttons
- **Shape:** squared (3px), 38px tall, 16px sides, 700 weight, 18px icon with 8px gap.
- **Primary:** bone fill, ground text; hover to paper white. One per action group (Play, Shuffle everything, Back to the song).
- **Secondary:** transparent with a hairline border, bone text; hover border to pencil grey.
- **Text link:** bone 600 with a pencil-grey underline at 3px offset; underline goes bone on hover ("Go online", "All 14 songs").
- **Focus:** 2px album-ink outline, 2px offset, everywhere.

### Transport
- **Bar:** booklet stock, hairline top, 76px, three columns (now-playing mini, centred controls with clocks, volume right).
- **Icon buttons:** 36px squares, faded bone, 20px SVG icons; hover to bone over a 7% bone fill; pressed toggles (shuffle, lyrics) turn album ink.
- **Play:** 44px bone-filled square with ground icon; hover paper white.
- **Progress:** a 1px hairline across the top edge with an album-ink fill, thickening to 4px on hover. Volume is a 3px track with an album-ink fill and a 12px square bone thumb.

### Track listing (song rows)
- 56px rows, 3px corners, 12px side padding; hover 5% bone.
- Number in pencil grey, title 600 bone over artist in faded bone 13px, album faded bone, time pencil grey right-aligned.
- Playing row: 24% album fill, title and number in album ink, number replaced by three animated 3px bars.
- Header row in label caps, pencil grey, closed by a hairline.
- Missing lyrics are flagged inline with a small uppercase "No lyrics" tag in pencil grey.

### Inputs / Fields
- **Search:** 34px, ground fill, hairline border, 3px, 13px text, 15px stroke search icon inset 10px; hover border to the pressed hairline.
- **Sort select:** 32px, same treatment, faded bone text.

### Navigation
- Sidebar nav items: 600 weight faded bone, 18px icon, 8px 10px padding, 3px; hover bone over 5% bone, current page bone over 8% bone; counts trail in pencil grey.
- Playlists list beneath a label-caps heading: 36px mosaic, name 600, song count in pencil grey.

### The Lyric Sheet (signature)
- Credits: cover, display title, then artist (bone), album (faded bone) and "From playlist · Track n of m" (pencil grey), all in label caps; closed by a 14% bone hairline.
- Lyric lines in the Lyric style; clicking a line seeks to it; scroll follows smoothly (instant under reduced motion).
- **The reader's pencil:** a 1px album-ink rule under the current line, scaled from left to right over the line's duration. Instrumental gaps show as widely spaced dots at 18px.
- Below the lyrics, a label-caps source line in pencil grey ("Lyrics · source", "Approximate timing", "not time-synced").

### Keycaps
- 12px 600 bone, hairline border with a 2px bottom edge, 3px corners; used inline in help text.

### Motion
- One easing for the world: `cubic-bezier(.16, 1, .3, 1)` (lyric colour changes .35s, card lift .35s, playing bars 1s). Control colour changes are .15s. All decorative motion stops under `prefers-reduced-motion`.

## Do's and Don'ts

### Do:
- **Do** take every chromatic accent from the playing song's cover through `album` and `album-ink`.
- **Do** separate planes with ground / stock / raised tone and 1px hairlines.
- **Do** set credits, table headers and source lines in 12px 600 uppercase at 0.09em.
- **Do** keep type on the six steps (12/13/15/18/26/40px) and figures tabular wherever numbers align.
- **Do** bundle every font and image; this page must render with no network.
- **Do** give every script in the library a chosen bundled face.

### Don't:
- **Don't** add a fixed brand accent colour or borrow YouTube Music's red, Roboto or layout.
- **Don't** round corners past 3px or use pill shapes.
- **Don't** use glass, blur panels or gradient text; the only gradients are the album washes behind the page and the volume fill.
- **Don't** put shadows on chrome; only cover art casts one.
- **Don't** load web fonts, CDNs or remote images.
