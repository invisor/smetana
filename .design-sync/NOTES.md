# Syncing this repository to claude.ai/design

The project is **Smetana Design System**, `5da5ca35-7bdf-4a9b-b237-6a6516330bc6`, pinned in
`config.json`. That file holds the pin and nothing else, deliberately — see the next section for
why the rest of the converter's schema does not apply here.

## The bundled `/design-sync` converter cannot run against this repository

It is React-only end to end, and the mismatch is structural rather than a matter of configuration:

- `lib/emit.mjs` vendors React and ReactDOM into `_vendor/` and mounts every preview through
  `ReactDOM.createRoot(...).render(React.createElement(window.<GLOBAL>.<Name>, ...))`.
- Components are discovered from PascalCase exports in a shipped `.d.ts` tree, and the emitted
  contract is `React.ComponentType<<Name>Props>`.
- Its dependency set is `esbuild ts-morph @types/react`.

This library is Vue 3 SFCs: no `.d.ts` anywhere, no `.tsx`/`.jsx`, no `tsconfig.json`, React not
installed, and `package.json` is `private: true` with no `main`/`module`/`exports` and no library
build. The converter stops at `[ZERO_MATCH]` on the first pass, and forcing past it through
`componentSrcMap` does not help: esbuild cannot bundle `.vue` without the Vue plugin, and a bundle
that did would put Vue component objects where the design agent writes React JSX.

**Do not spend another session rediscovering this.** The direction is also the reverse of what the
skill assumes: the 28 React components already in the project are the original, and the Vue tree in
`src/` is the port that grew past them — 78 components against those 28.

## What is uploaded instead, and by what

`make-cards.mjs` builds one static HTML+CSS card per gallery section from the live gallery, and
`verify-cards.mjs` loads every card the way the pane will and fails on anything that did not draw.
Both need `playwright-core` and a chromium binary; neither is a repository dependency:

```sh
npm --prefix /tmp/pw i playwright-core          # anywhere outside the repo
ln -sfn /tmp/pw/node_modules/playwright-core .design-sync/node_modules/playwright-core
npm run dev                                     # the generator reads the live gallery
CHROMIUM_PATH=~/Library/Caches/ms-playwright/chromium-1169/chrome-mac/Chromium.app/Contents/MacOS/Chromium \
  node .design-sync/make-cards.mjs && node .design-sync/verify-cards.mjs
```

Capturing rendered markup is faithful **because of this system's styling rule**: components carry no
scoped CSS and no classes, so a section's `outerHTML` is self-contained inline `var(--token)`
references and renders identically anywhere `styles.css` is loaded. A probe of the whole gallery
found exactly one class of ours in the tree (`sm-scroll-hidden`); everything else with a class name
belongs to CodeMirror or xterm.

Uploaded: `gallery/` (18 cards, `_card.css`, `app-icon.png`), `tokens/` (12 files) and `styles.css`.
Nothing was deleted — the React components, `guidelines/`, `templates/` and `ui_kits/` are
untouched, by decision, since the React 28 are still the only thing in the project the design agent
can actually build with. HTML cards are reference, not runnable parts.

## Gotchas the next run should not have to rediscover

- **xterm needs its two kinds of CSS separated.** Its palette is a stylesheet injected into its own
  container, so the captured markup already carries it; its base stylesheet lives in the document
  head and must be carried into the card, or a row of character-measurement glyphs prints across the
  top. Both panes' palette blocks name the same generated `xterm-dom-renderer-owner-N` selectors, so
  the dark pane's owner is renumbered — without that the later block repaints the earlier one and
  the light pane drew dark ink on its own light ground.
- **The terminal has to be waited for, not slept on.** A fixed 3s wait captured empty rows; the
  generator waits for `.xterm-rows` to have text.
- **CodeMirror is the easy one.** It injects only into the head, and `editor/theme.js` is written in
  token references, so a single copy re-resolves correctly in both panes.
- **Card heights must be measured after the fact.** A section laid out across the gallery's 1400px
  reflows to a very different height in a 450px pane, and `.pair`'s own `min-height:100vh` hides the
  real number unless it is zeroed for the measurement.
- **The About tab's app icon** is served by the dev server from `/src/assets/app-icon.png`, which
  means nothing once uploaded; it ships as `gallery/app-icon.png`.
- **Two console errors are the page behaving correctly** and are dropped by name in the verifier: the
  generic companion line to a failed request (real ones are caught by URL instead), and
  `ReportView`'s `sandbox=""` refusing to run a script, which is that frame's whole point.
- **Trimming is card design, not misrepresentation.** The gallery renders a dozen repetitions of a
  panel to show a behaviour; a card wants two or three states. `TRIM` in the generator holds the
  per-section counts. Git was 886 KB untrimmed.

## Re-sync risks

- The card set is captured from `Gallery.vue`. A section renamed there silently drops out of
  `SECTIONS` in the generator and its card goes stale in the project rather than failing — the
  generator reports `MISSING` for a section it cannot find, so read that output.
- Token names were diffed against the project before overwriting and **nothing was removed** — the
  only changes were additions (`--ui-scale`, `--z-popover`, `.sm-scroll-hidden`, an italic axis in
  `fonts.css`) plus the new `color-type.css`. A future sync that removes or renames a token would
  break the 28 React cards, which link the same `styles.css`. Diff before uploading tokens again.
- The chromium path above is a version-pinned cache directory and will rot; it is an environment
  detail, not a repository one.
