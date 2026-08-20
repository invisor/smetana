/* Builds static HTML+CSS preview cards for claude.ai/design from the live
   gallery (`?view=gallery`), one card per gallery section.

   Why this exists rather than the bundled /design-sync converter: that converter
   is React-only end to end — it vendors React+ReactDOM, mounts every preview
   through `ReactDOM.createRoot(...).render(React.createElement(...))` and
   discovers components from PascalCase `.d.ts` exports. This library is Vue 3
   SFCs with no TypeScript and no library build, so the converter stops at
   `[ZERO_MATCH]`. See NOTES.md.

   Why capturing rendered markup is faithful here and would not be in most
   repositories: components in this system carry no scoped CSS and no classes —
   every visual value is an inline `:style` of `var(--token)` references. A
   section's `outerHTML` is therefore self-contained and renders identically
   anywhere `styles.css` is loaded. The two exceptions render their own DOM with
   their own generated classes, CodeMirror (`cm-*`, `ͼ*`) and xterm (`xterm-*`),
   so the stylesheets those libraries inject are captured and inlined into the
   three cards that need them.

   Both themes are captured from their own render rather than by reusing the
   light markup under `data-theme="dark"`. Most of the tree would survive that,
   but two things resolve colour in JS and would come out wrong: the file-type
   icons, whose palette is substituted per theme into a `data:` URL
   (`src/catppuccinIcon.js`), and the terminal, which is handed resolved colour
   strings (`components/terminal/theme.js`). */
import { chromium } from 'playwright-core'
import { createServer } from 'node:http'
import { mkdirSync, writeFileSync, rmSync, cpSync, readFileSync, existsSync } from 'node:fs'
import { join, extname } from 'node:path'

const EXE = process.env.CHROMIUM_PATH
const BASE = process.env.GALLERY_URL || 'http://localhost:5173'
/* The staging tree mirrors the remote project exactly — `gallery/<slug>.card.html`
   beside a root `styles.css` and `tokens/` — so a card verified here is verified
   against the same relative paths it will resolve through once uploaded. */
const STAGE = process.env.STAGE_DIR || join(process.cwd(), '.design-sync/upload')
const OUT = join(STAGE, 'gallery')

if (!EXE) {
  console.error('set CHROMIUM_PATH to a chromium binary playwright-core can drive')
  process.exit(1)
}

/* Section title -> file slug, card name and the group label the Design System
   pane sorts by. Order follows the gallery itself. */
const SECTIONS = [
  ['Buttons', 'buttons', 'Buttons', 'Button and IconButton, every variant, size and state'],
  ['Form controls', 'form-controls', 'Form controls', 'Input, Textarea, Select, Dropdown, Checkbox, Switch, Tooltip'],
  ['Status', 'status', 'Status', 'StatusBadge, StatusDot, dependency marks, assignees — reserved and generated'],
  ['Kanban', 'kanban', 'Kanban', 'Board, columns, task cards, type badges'],
  ['Scope bar', 'scope-bar', 'Scope bar', 'ScopeIndicator and its counters'],
  ['Shell', 'shell', 'Shell', 'AppShell, Panel, TabBar, Resizer, SectionHeader'],
  ['Editor', 'editor', 'Editor', 'FileEditor over CodeMirror'],
  ['Diff', 'diff', 'Diff', 'DiffView, unified and side by side'],
  ['Terminal', 'terminal', 'Terminal', 'TerminalView over xterm.js'],
  ['Agents', 'agents', 'Agents', 'AgentList, ChatMessage, ToolCall, CodeBlock'],
  ['Git', 'git', 'Git', 'GitPanel: status, changes, branches, commit box'],
  ['Projects', 'projects', 'Projects', 'ProjectList, RepoList, setup and branch modals'],
  ['Dropdown', 'dropdown', 'Dropdown', 'Dropdown and the branch picker built on it'],
  ['Run bar', 'run-bar', 'Run bar', 'RunBar and the run modal'],
  ['Run report', 'run-report', 'Run report', 'ReportView'],
  ['Agent output', 'agent-output', 'Agent output', 'LogView, LogLine, LogToolbar, ansi text'],
  ['Settings window', 'settings', 'Settings', 'Every settings tab and SettingsRow'],
  ['Overlays and states', 'overlays', 'Overlays', 'Modal, PointerMenu, ContextMenu, EmptyState, Skeleton, notifications']
]

/* Sections the gallery deliberately renders many repetitions of, to show a
   behaviour rather than a look. A card wants two or three representative states,
   so these keep only the first N direct children after the heading. Everything
   else is captured whole. */
const TRIM = { Git: 1, Kanban: 2, Projects: 3, Settings: 2, Overlays: 4, 'Run report': 1 }

/* The dev server serves the About tab's app icon — the system's one raster — from
   a source path that means nothing once the card is uploaded. The file ships
   beside the cards instead. */
const APP_ICON = { from: 'src/assets/app-icon.png', devSrc: '/src/assets/app-icon.png', as: 'app-icon.png' }

const CARD_CSS = `html,body{margin:0;background:var(--border-subtle);font-family:var(--font-sans)}
.pair{display:grid;grid-template-columns:1fr 1fr;gap:1px;background:var(--border);min-height:100vh}
.half{color:var(--text-primary);background:var(--canvas);padding:10px 12px;min-width:0;overflow:hidden}
.full{color:var(--text-primary);background:var(--canvas);padding:10px 12px;min-height:100vh;min-width:0;overflow:hidden}
.lbl{font:500 9px/1 var(--font-mono);letter-spacing:.07em;text-transform:uppercase;color:var(--text-muted);margin-bottom:8px}
.stack{display:flex;flex-direction:column;gap:10px;min-width:0}
.stack>div{min-width:0}
`

const page = await (async () => {
  const browser = await chromium.launch({ executablePath: EXE })
  return { browser, page: await browser.newPage({ viewport: { width: 1400, height: 1200 } }) }
})()

/* One pass per theme. Returns, per section title, the markup of everything after
   the section heading, plus the stylesheets the two DOM-owning libraries injected
   into this document. */
async function capture(theme) {
  await page.page.goto(`${BASE}/?view=gallery&theme=${theme}`, { waitUntil: 'networkidle' })
  // CodeMirror and xterm mount asynchronously and paint a frame later. A fixed
  // wait caught the terminal mid-mount and captured empty rows, so wait on the
  // thing itself and keep the timeout only as a floor under a gallery that has
  // no terminal in it at all.
  await page.page
    .waitForFunction(
      () => {
        const rows = document.querySelector('.xterm-rows')
        return !rows || (rows.textContent || '').trim().length > 0
      },
      null,
      { timeout: 20000 }
    )
    .catch(() => {})
  await page.page.waitForTimeout(1500)
  return page.page.evaluate((trim) => {
    const out = { sections: {}, cm: '', xtermBase: '' }
    for (const style of document.querySelectorAll('style')) {
      const css = style.textContent || ''
      // The app's own stylesheet is served as a <style> by the dev server; it is
      // linked by the card instead, so it must not be inlined.
      if (css.includes('--canvas:') || css.includes('IBM+Plex+Mono')) continue
      if (css.includes('.cm-') || css.includes('ͼ')) out.cm += css + '\n'
      /* xterm's styling arrives in two kinds and only one of them belongs in the
         card's head. The blocks naming a generated `xterm-dom-renderer-owner-N`
         are that instance's palette, and the captured markup already carries
         them inside its own container. What is left is the library's base
         stylesheet — the rules that position the helpers and hide the character
         measurement element — which lives in the document head and has to be
         carried, or a row of measurement glyphs prints across the top of the
         card. */
      else if (css.includes('.xterm') && !css.includes('xterm-dom-renderer-owner')) out.xtermBase += css + '\n'
    }
    for (const section of document.querySelectorAll('section')) {
      const title = section.firstElementChild?.textContent?.trim() || ''
      let kids = [...section.children].slice(1)
      const limit = Object.entries(trim).find(([k]) => title.startsWith(k))
      if (limit) kids = kids.slice(0, limit[1])
      const box = section.getBoundingClientRect()
      out.sections[title] = {
        html: kids.map((k) => k.outerHTML).join('\n'),
        width: Math.round(box.width),
        height: Math.round(kids.reduce((n, k) => n + k.getBoundingClientRect().height + 12, 20))
      }
    }
    return out
  }, TRIM)
}

const light = await capture('light')
const dark = await capture('dark')
await page.browser.close()

const attr = (s) => String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;')

rmSync(STAGE, { recursive: true, force: true })
mkdirSync(OUT, { recursive: true })
writeFileSync(join(OUT, '_card.css'), CARD_CSS)

/* The stylesheet and the tokens go up verbatim: they are the same files the app
   itself loads, and `styles.css` is an @import list whose relative paths already
   match this layout. */
cpSync(join(process.cwd(), 'src/styles/styles.css'), join(STAGE, 'styles.css'))
cpSync(join(process.cwd(), 'src/styles/tokens'), join(STAGE, 'tokens'), { recursive: true })
cpSync(join(process.cwd(), APP_ICON.from), join(OUT, APP_ICON.as))

const report = []
for (const [match, slug, name, subtitle] of SECTIONS) {
  const key = Object.keys(light.sections).find((t) => t.startsWith(match))
  if (!key) {
    report.push({ slug, status: 'MISSING' })
    continue
  }
  const l = light.sections[key]
  const d = dark.sections[key]

  /* CodeMirror writes its stylesheet into the document head, so the card has to
     carry it or the editor and the diff arrive unstyled. One copy serves both
     panes: `editor/theme.js` is written in `var(--token)` references like
     everything else, so it re-resolves per pane on its own.

     xterm is split: only its base stylesheet goes here, since each pane's
     palette rides inside the captured markup. See the classification in
     `capture`, and `darkHtml` below for why the two palettes need separating. */
  const injected = slug === 'editor' || slug === 'diff' ? light.cm : slug === 'terminal' ? light.xtermBase : ''

  /* xterm is handed resolved colour strings rather than tokens, so each pane's
     palette is baked into a stylesheet of its own — and both name the same
     generated `xterm-dom-renderer-owner-N` selectors, so whichever landed later
     in the document repainted the other. Renumbering the dark pane's owner makes
     the two independent. Without this the light pane drew the dark theme's ink
     on its own light ground and was very nearly unreadable. */
  const darkHtml = d.html.replace(/xterm-dom-renderer-owner-(\d+)/g, 'xterm-dom-renderer-owner-9$1')

  /* Both themes side by side is the house style, but a heavy section doubled
     would make a card nobody can load. Past the budget it draws light only and
     says so. */
  const paired = l.html.length + darkHtml.length < 260_000
  const body = paired
    ? `<div class="pair"><div class="half"><div class="lbl">light</div><div class="stack">${l.html}</div></div>` +
      `<div class="half" data-theme="dark"><div class="lbl">dark</div><div class="stack">${darkHtml}</div></div></div>`
    : `<div class="full"><div class="lbl">${attr(name.toLowerCase())} — light theme</div><div class="stack">${l.html}</div></div>`

  const width = paired ? 900 : 700
  // Provisional; the measuring pass below replaces it with what the card is
  // actually this wide. The gallery measurement cannot be reused — a section
  // laid out across 1400px reflows to a very different height in a 450px pane.
  const height = 400
  const html =
    (`<!-- @dsCard group="App gallery" viewport="${width}x${height}" name="${attr(name)}" subtitle="${attr(subtitle)}" -->\n` +
    `<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><title>${attr(name)}</title>\n` +
    `<link rel="stylesheet" href="../styles.css"><link rel="stylesheet" href="_card.css">\n` +
    (injected ? `<style>\n${injected}</style>\n` : '') +
    `</head>\n<body>${body}</body></html>\n`
    ).split(APP_ICON.devSrc).join(APP_ICON.as)

  writeFileSync(join(OUT, `${slug}.card.html`), html)
  report.push({ slug, kb: Math.round(html.length / 1024), paired, injectedKB: Math.round(injected.length / 1024) })
}

/* Measuring pass. Each card is served from the staging tree at its own declared
   width and asked how tall it came out, and the `viewport` in its @dsCard line
   is rewritten to match. Serving rather than opening from disk so `../styles.css`
   resolves through the same relative path the uploaded card will use. */
const server = createServer((req, res) => {
  const path = join(STAGE, decodeURIComponent(req.url.split('?')[0]))
  if (!existsSync(path)) {
    res.writeHead(404).end()
    return
  }
  const type = { '.html': 'text/html', '.css': 'text/css', '.png': 'image/png' }[extname(path)]
  res.writeHead(200, { 'content-type': type || 'application/octet-stream' })
  res.end(readFileSync(path))
})
await new Promise((r) => server.listen(0, r))

const measurer = await chromium.launch({ executablePath: EXE })
for (const row of report) {
  if (!row.kb) continue
  const file = join(OUT, `${row.slug}.card.html`)
  const source = readFileSync(file, 'utf8')
  const width = Number(source.match(/viewport="(\d+)x/)[1])
  const tab = await measurer.newPage({ viewport: { width, height: 400 } })
  await tab.goto(`http://localhost:${server.address().port}/gallery/${row.slug}.card.html`, {
    waitUntil: 'networkidle'
  })
  await tab.waitForTimeout(300)
  const measured = await tab.evaluate(() => {
    /* `.pair` and `.full` are `min-height:100vh` so a card always fills its box.
       That floor is exactly what a measurement must not see — left on, every
       short card reports the height of the window it was measured in. */
    const root = document.querySelector('.pair, .full')
    const boxes = [root, ...root.querySelectorAll('.half, .full')]
    for (const box of boxes) box.style.minHeight = '0'
    const height = Math.ceil(Math.max(...[...root.children].map((c) => c.scrollHeight), root.scrollHeight))
    for (const box of boxes) box.style.minHeight = ''
    return height
  })
  await tab.close()
  // A card is better a little tall than clipped, hence the padding; the ceiling
  // keeps one long section from becoming a card nobody can take in at a glance.
  row.height = Math.min(Math.max(measured + 24, 180), 1100)
  writeFileSync(file, source.replace(/viewport="\d+x\d+"/, `viewport="${width}x${row.height}"`))
}
await measurer.close()
server.close()

console.log(JSON.stringify(report, null, 1))
