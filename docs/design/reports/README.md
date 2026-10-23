# Handoff: smetana report family — code review report + run report

## Overview
Two self-contained HTML documents produced by Smetana and opened in an app tab (sandboxed iframe) or directly in a browser, often side by side:
- **Code review report** — the result of an automated review of one or more ref pairs.
- **Run report** — what an unattended agent run did: closed / parked tasks, batches, why it stopped.

They are one system: the same `<style>` (byte-identical), the same header, summary strip, cards, section heads, footer, type and palette. The task is to write **two renderers**: input JSON (`*/schema.json`) → one HTML string.

## About the design files
`reference/*.html` are **hi-fi design references** filled with test data. They already meet every output constraint, so their markup and `reports.css` are meant to be used **verbatim** as templates. What's left to build: the template (any engine — `askama`/`minijinja` in Rust, TS template literals, …), escaping, field derivation, and tests for the listed states. Each reference is annotated with `REPEAT` / `FIELDS` comments.

## Fidelity
**High-fidelity.** Colours, type, spacing and copy are final. Tokens come from the Smetana design system and are inlined, because the documents cannot load external files.

## Hard output constraints (both documents)
1. One file: `<!doctype html>`, explicit `<html lang="en">`, the whole of `reports.css` inside a single `<style>` in `<head>`. **The same CSS in both documents** — don't fork it; add classes to the shared file.
2. No `<script>`, no `<img>`, no external CSS, fonts or icons. Glyphs are Unicode characters (see *Glyphs*) or inline SVG.
3. System font stacks only: `--font-sans` for prose, `--font-mono` for identifiers (ids, paths, refs, durations, numbers, code). `"IBM Plex Mono"` sits first in the stack and is used only if installed locally — no `@font-face`.
4. Themes: the full token set in `:root` (light) → `@media (prefers-color-scheme: dark){:root:not([data-theme="light"])}` → `:root[data-theme="dark"]` → `:root[data-theme="light"]`. Each block sets `color-scheme`. The host may set `data-theme` on `<html>`.
5. No gradients, glass, blur or emoji. Sentence case in source text; uppercase only through CSS on 10px mono labels (`.eyebrow`, `.cell-label`, `dt`, `.flag`).
6. Saturated colour **only** for status (severity, verdict range, parked, batch outcome, stop reason). Never colour alone: always glyph + word.
7. No horizontal page scroll. Paths, refs and inline `<code>` wrap (`overflow-wrap:anywhere`); multi-line code (`figure.code`) scrolls inside itself.
8. HTML-escape every substituted field. Agent free text is escaped first; then backtick spans become `<code>…</code>`. Nothing else is parsed (no Markdown).

## Shared building blocks

| block | markup | notes |
|---|---|---|
| Page | `main.page` | `max-width:880px`, centred, padding `32px 20px 48px` (`20px 12px 32px` ≤640px), vertical flex, `gap:24px` |
| Header | `header.head` > `.eyebrow` + `h1` (+ `.meta` in run report) | eyebrow `smetana · code review` / `smetana · run report`; h1 sans 22/600; `.meta` mono 12, secondary, wraps anywhere |
| Summary strip | `.strip` > `.cell` × 4 | grid `repeat(auto-fit,minmax(140px,1fr))`, gap 8. `.cell-label` (mono 10/500 caps, optional glyph) + `.cell-n` (mono 22/500). Use `<nav>` + `<a class="cell" href="#section">` when a cell has a target, otherwise `<div>` |
| Cell modifiers | `.tone-{failed,needs-you,done,neutral}` · `.is-loud` · `.is-zero` · `.is-none` | tone → border + number/glyph colour; `.is-loud` adds the tone background (**max one loud cell per document**); `.is-zero` = count 0, muted, no tone; `.is-none` = no data, number is `—` |
| Section | `section.group` > `.group-head` (glyph?, `h2`, `.count`, `.hint`?) | h2 sans 15/600, 1px `--border` underline; children flex column, gap 12 |
| Card | `.card` | `--surface-raised`, 1px `--border`, radius 4, `--shadow-raised` (none in dark) |
| Label/value list | `dl.f-body` | grid `96px | 1fr` (`120px` in batches), `dt` mono 10 caps muted; one column ≤640px |
| One-line status | `p.stop.tone-*` / `p.outcome.tone-*` > `.g` + text | flex row; **wrap text in one `<span>` if it contains inline markup** |
| Inline code | `:not(pre)>code` | mono .94em, `--surface-sunken` background, radius 2, wraps anywhere |
| Footer | `footer.foot` > `dl` | grid `150px | 1fr`, 12px; one column ≤640px |

### Tones
| tone | token set | used for |
|---|---|---|
| `tone-failed` | `--status-failed-*` | blocking, verdict <70, crashed / vanished batch, crashed stop |
| `tone-needs-you` | `--status-needs-you-*` | serious, verdict 70–89, parked, handed-back batch |
| `tone-done` | `--status-done-*` | verdict 90–100, closed, clean exit, queue-empty stop |
| `tone-neutral` | `--status-neutral-*` / surfaces | minor, removed-by-person, stopped-by-person |

Text colour classes for glyphs: `.c-blocking` `.c-serious` `.c-minor` `.c-ok`.

### Glyphs
`■` U+25A0 failed/blocking · `▲` U+25B2 needs-you/serious/parked · `○` U+25CB neutral/minor · `●` U+25CF done/ok · `←` U+2190 · `−` U+2212 (minus in stats) · `—` U+2014 (no data) · `·` U+00B7 (separator).

### Formatting
- Durations (run report): `null` → `—`; `< 60s` → `<1m`; `< 1h` → `{m}m`; otherwise `{h}h {m}m` (`1h 0m` stays as is). Minutes are floored.
- Dates: `YYYY-MM-DD HH:mm`, local time.
- Separators between telemetry values: ` · `.

---

## Document 1 — Code review report
Schema `code-review/schema.json` · fixtures `code-review/examples/{typical,no-findings}.json` · references `reference/Code review report*.html`.

`<title>`: `Code review · {repos joined ", "} · {score}%`.

1. **Header.** h1 = repo names joined with `", "`. `ul.refs.card` — REPEAT per pair, `li.ref` (grid `1fr auto`, padding `8px 16px`): `.ref-repo` (mono 12/600) · `.ref-pair` = `{base}<span class="arrow">←</span>{head}` · `.ref-stat` = `{files} files · <span class="add">+{added}</span> <span class="del">−{removed}</span>` ("1 file" when singular).
2. **Verdict** `section.verdict.card` — grid `auto | 1fr`, gap 24. Left: eyebrow "Readiness", `.score-num` `{score}<small>%</small>` (mono 600 ≈64px), `.score-label` glyph + range label. Right: `.bar` with three bands at 70% / 20% / 10% (`b-low`/`b-mid`/`b-high`, current one `.is-current`), `.marker` `style="left:{score}%"` (at ≥98: `left:calc(100% - 2px)`, span `translateX(-90%)`), `.ticks` 0/70/90/100, `ul.legend` in fixed order high → mid → low, current row `.is-current` + "Current".

   | range | score | glyph | colour | label |
   |---|---|---|---|---|
   | high | 90–100 | ● | `c-ok` | Ready as is |
   | mid | 70–89 | ▲ | `c-serious` | Serious issues, nothing blocking |
   | low | <70 | ■ | `c-blocking` | Blocking issue — returned for rework |

   The score is an input; log a warning (don't fail) if it contradicts the findings (blocking > 0 with score ≥ 70).
3. **Summary strip:** `■ Blocking` (`tone-failed is-loud` when > 0) · `▲ Serious` (`tone-needs-you`) · `○ Minor` (`tone-neutral`) · `Findings` (total, no tone). A count of 0 → `<div class="cell is-zero">`, no tone, no link. All zero → `<div class="strip">`, not `<nav>`.
4. **Groups** `section.group.group-{level}#{level}` — order blocking → serious → minor, non-empty only. Hints: "Must be fixed before merge" / "Fix before merge or file a follow-up" / "Optional". `.group-minor` has tighter cards.
5. **Finding** `article.finding.card#{id}` — id = `B|S|M` + index within the level. `.f-title` (`.f-id.c-{level}` "{glyph} {id}" + h3) · `.f-loc` `<span class="repo">{repo}/</span>{path}:<span class="line">{line}</span>` · `dl.f-body`: Defect / Fails when / Fix (+ optional `figure.code`). In "Fails when" backticks → `<span class="io">` instead of `<code>`.
6. **Code quote** `figure.code` > `figcaption` + `<pre>` of `span.l` per line (**no whitespace between lines**), `span.n` line number, `.hl` for highlighted lines. Optional syntax classes `.k .s .nu .cm .fn .ty`; plain escaped text is fine. ≤ 12 lines.
7. **Zero findings:** groups are replaced by `section.clean.card` — "● No findings", the sentence "Every changed file was read against the project rules. Nothing blocking, serious or minor." (drop "against the project rules" if no pair has `hazards`), and `ul.files` from `reviewedFiles` (prefix `{repo}/` when there are several repos).
8. **Footer:** Project rules (`[merge].hazards` + `yes`/`no` per repo, `· {rulesSource}` if known) · Reviewer · Date · Refs (REPEAT `{repo} · {base} ← {head}`).

## Document 2 — Run report
Schema `run-report/schema.json` · fixtures `run-report/examples/{typical,crashed}.json` · references `reference/Run report*.html`.

`<title>`: `Run report · {title} · smetana`.

1. **Header:** eyebrow `smetana · run report`, h1 `{title}`, `.meta` = `{project} · {scope} · finished {finishedAt}`.
2. **Summary strip** (`<nav>` if any cell links):
   - `● Closed` — `tone-done` when N > 0 → `#closed`; N = 0 → `.is-zero`.
   - `▲ Parked · needs you` — N > 0 → `tone-needs-you is-loud` → `#parked`; N = 0 → label `Parked`, `.is-zero`.
   - `Batches` → `#batches`.
   - `Total` — formatted `totalSec`.
   - `tasks: null` → Closed and Parked become `.is-none` with `—` and `aria-label="no data"`, no tone, no link.
3. **Stop reason** `p.stop` (15px): `queue-empty` → ● `tone-done` · `stopped-by-person` → ○ `tone-neutral` · `crashed` → ■ `tone-failed` · `other` → ○ `tone-neutral`. The text is used as is.
4. **Summary** section — only if `summary` is non-empty. h2 "Summary", `.prose` (max 72ch) with one `<p>` per paragraph.
5. **Closed** section `#closed` (● glyph, count) — only if there are tasks. REPEAT `article.task.card#{id}`:
   ```html
   <div class="task-top">
     <div class="task-meta"><span class="task-id">{id}</span></div>
     <span class="dur">{duration}</span>            <!-- .is-none + aria-label="duration unknown" when null -->
     <h3>{title}</h3>
   </div>
   <p class="task-body">{done}</p>
   ```
6. **Parked** section `#parked` (▲ glyph, count, hint "Waiting on you") — same card + `.is-parked` (needs-you border, doubled with a 1px ring) and `<span class="flag tone-needs-you"><span aria-hidden="true">▲</span>Waiting on you</span>` inside `.task-meta`. Order: Closed, then Parked.
7. **Batches** section `#batches` — REPEAT `article.batch.card`:
   - `.batch-top`: `.batch-label` = `batch {n}` + (` · attempt {attempt}` when > 1) + ` <span class="agent">({agent})</span>`; `.dur` on the right.
   - Notes: `p.task-body`, or `p.no-account` "This batch left no account of itself." when `notes` is null.
   - Optional `dl.f-body`: "Left unfinished" → `dd.ids` with one `<code>` per id; "Lock released" → `<code>{task}</code> · by {by}`.
   - Outcome `p.outcome` (top hairline):

     | kind | glyph · tone | text |
     |---|---|---|
     | `clean-exit` | ● `tone-done` | The run saw its session exit cleanly. |
     | `exit-code` | ■ `tone-failed` | The session crashed with exit code `<code>{code}</code>`. |
     | `vanished` | ■ `tone-failed` | The session process disappeared without an exit code. |
     | `removed-by-person` | ○ `tone-neutral` | A person removed the session. |
     | `handed-back` | ▲ `tone-needs-you` | The lead handed the work back to a person. |
8. **Footer:** Total (mono, formatted) · Journal (mono path, wraps anywhere).

## States to test
Code review: typical (1/3/5, two repos) · no findings (100%) · 4+ repos · one empty level in a non-empty report · long path + long code line.
Run report: typical (5 closed, 1 parked, 3 batches, one with attempt 2) · crashed on the first batch (`summary: null`, `tasks: null`) · long `done` text with paths and backticks (`smetana-6tev`) · parked = 0 · every outcome kind.
Both:
- at a 360px iframe width, `scrollWidth === clientWidth`;
- render with `data-theme="dark"`, with `data-theme="light"`, and with no attribute under OS dark mode;
- the `<style>` of the two renderers must match `reports.css` byte for byte (snapshot test).

## Interactions
Anchor links only (strip cells → sections, `#id` on findings/tasks). Links use `--text-link`, underlined on hover. `a.cell:hover` → `--surface-hover` (loud cells: the border takes the tone fg colour). Focus-visible = 2px `--focus-ring`, offset 1px. No animation. Print: cards don't break across pages; code wraps.

## Design tokens (inlined in `reports.css`)
| token | light | dark |
|---|---|---|
| `--canvas` | `#eaeeef` | `#10151a` |
| `--surface-sunken` | `#e1e6e7` | `#0c1116` |
| `--surface` | `#f4f7f7` | `#161b21` |
| `--surface-raised` | `#ffffff` | `#1b2229` |
| `--surface-hover` | `#e4e9ea` | `#202932` |
| `--border-subtle` / `--border` / `--border-strong` | `#dde3e3` / `#c9d1d2` / `#a9b4b6` | `#232b33` / `#2e3841` / `#3d4954` |
| `--text-primary` / `-secondary` / `-muted` | `#16201f` / `#4a565a` / `#6b777c` | `#e3e8ed` / `#a8b3bd` / `#7c8b97` |
| `--text-link` / hover | `#1f5d8f` / `#123f63` | `#8fb6e8` / `#b3cef2` |
| `--focus-ring` | `#1c6fd0` | `#5fa8ff` |
| `--selection-bg` | `#c6dcf0` | `#2b4560` |
| `--shadow-raised` | `0 1px 2px rgba(20,28,28,.06), 0 1px 1px rgba(20,28,28,.04)` | `none` |
| `--status-failed-{fg,bg,border}` | `#a33224` `#fbe8e5` `#edbfb8` | `#ee8878` `#2c1815` `#653029` |
| `--status-needs-you-*` | `#8a5405` `#fbf0da` `#e8ce94` | `#f2b03d` `#2b2010` `#6a4e1b` |
| `--status-done-*` | `#3f6b54` `#e6eee9` `#c0d3c8` | `#7fa792` `#16211c` `#2c4136` |
| `--status-neutral-*` (local) | `#4a565a` `#e1e6e7` `#c9d1d2` | `#a8b3bd` `#0c1116` `#2e3841` |
| code: `--editor-bg`, `--editor-line-number`, `--editor-hl`, `--editor-hl-gutter` | `#f7faf9` `#8b979b` `#fbe4e0` `#c94a34` | `#0e1317` `#5b6771` `#25150f` `#c94a34` |
| syntax `--syn-keyword/string/number/comment/function/type` | `#8a3a96` `#48762f` `#a3560f` `#6b777c` `#2a5da8` `#13707a` | `#c58bd6` `#8fbf7a` `#e0a45c` `#7c8b97` `#9bc0f0` `#6bc5ce` |
| `--diff-added-fg` / `--diff-removed-fg` | `#2f5c1c` / `#8f2a1c` | `#a8d693` / `#ff9384` |

- **Type:** 10 / 11 / 12 / 13 / 15 / 18 / 22 / 28 px (`--text-2xs`…`--text-3xl`); body 13/1.5 sans; caps tracking `.07em`.
- **Space:** 2 / 4 / 6 / 8 / 12 / 16 / 20 / 24 / 32 / 48 (`--space-1…10`).
- **Radius:** 2 / 4 / 6.

Everything except `--status-neutral-*` mirrors `tokens/color-surfaces.css`, `color-status.css` and `color-editor.css` in the DS. When those change, regenerate the token blocks at the top of `reports.css`.

## Assets
None.

## Files
- `reports.css` — the shared stylesheet, inlined verbatim into both documents.
- `code-review/schema.json`, `code-review/examples/typical.json`, `code-review/examples/no-findings.json`
- `run-report/schema.json`, `run-report/examples/typical.json`, `run-report/examples/crashed.json`
- `reference/Code review report.html`, `reference/Code review report - no findings.html`
- `reference/Run report.html`, `reference/Run report - crashed.html`
