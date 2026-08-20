/* Loads every staged card the way the Design System pane will and reports
   whether it actually drew. There is no component test runner in this repository
   (see CLAUDE.md), so this is the only thing standing between a broken card and
   a design system that renders wrong in every design built from it.

   The load-bearing assertion is the computed background of each pane. `.half`
   asks for `var(--canvas)`; if `styles.css` failed to resolve through the card's
   relative path, or a token file went missing from the @import closure, that
   property falls back to transparent and the check fails. Comparing the light
   pane against the light value and the dark pane against the dark one also
   proves `data-theme` still switches the whole palette from a static document. */
import { chromium } from 'playwright-core'
import { createServer } from 'node:http'
import { readFileSync, readdirSync, mkdirSync, existsSync } from 'node:fs'
import { join, extname } from 'node:path'

const EXE = process.env.CHROMIUM_PATH
const STAGE = process.env.STAGE_DIR || join(process.cwd(), '.design-sync/upload')
const SHOTS = join(process.cwd(), '.design-sync/.cache/shots')

const CANVAS_LIGHT = 'rgb(234, 238, 239)' // --canvas, :root
const CANVAS_DARK = 'rgb(16, 21, 26)' //     --canvas, [data-theme="dark"]

const TYPES = { '.html': 'text/html', '.css': 'text/css', '.woff2': 'font/woff2', '.png': 'image/png' }
const server = createServer((req, res) => {
  const path = join(STAGE, decodeURIComponent(req.url.split('?')[0]))
  if (!existsSync(path)) {
    res.writeHead(404).end('nope')
    return
  }
  res.writeHead(200, { 'content-type': TYPES[extname(path)] || 'application/octet-stream' })
  res.end(readFileSync(path))
})
await new Promise((r) => server.listen(0, r))
const port = server.address().port

mkdirSync(SHOTS, { recursive: true })
const browser = await chromium.launch({ executablePath: EXE })
const page = await browser.newPage({ viewport: { width: 900, height: 700 } })

const cards = readdirSync(join(STAGE, 'gallery')).filter((f) => f.endsWith('.card.html')).sort()
const results = []

for (const file of cards) {
  const slug = file.replace('.card.html', '')
  const errors = []
  const missing = []
  page.removeAllListeners('pageerror')
  page.removeAllListeners('console')
  page.removeAllListeners('response')
  /* Two console errors are the page behaving correctly and are dropped by name
     rather than by loosening the check. The first is the generic companion line
     to a failed request — real ones are counted through `missing` below, with
     their URL, which is the form worth failing on. The second is `ReportView`'s
     `sandbox=""`: every restriction on at once is the point of that frame, so
     the refusal to run a script is the guarantee holding, not a fault. */
  const expected = (text) =>
    text.startsWith('Failed to load resource') || text.includes("frame is sandboxed and the 'allow-scripts'")
  page.on('pageerror', (e) => errors.push(String(e).slice(0, 160)))
  page.on('console', (m) => m.type() === 'error' && !expected(m.text()) && errors.push(m.text().slice(0, 160)))
  // The browser asks every origin for a favicon this server has no reason to
  // hold; that 404 says nothing about the card.
  page.on(
    'response',
    (r) => r.status() >= 400 && !r.url().endsWith('/favicon.ico') && missing.push(`${r.status()} ${r.url().slice(-70)}`)
  )

  /* The @dsCard marker is read off disk rather than out of the DOM: it sits
     before the doctype, so the parser hangs it off `document` and not off
     `documentElement`, and a card is registered by the pane from the file's
     first line either way. */
  const firstLine = readFileSync(join(STAGE, 'gallery', file), 'utf8').split('\n')[0]

  await page.goto(`http://localhost:${port}/gallery/${file}`, { waitUntil: 'networkidle' })
  await page.waitForTimeout(400)

  const probe = await page.evaluate(() => {
    const panes = [...document.querySelectorAll('.half, .full')]
    return {
      panes: panes.length,
      paneBg: panes.map((p) => getComputedStyle(p).backgroundColor),
      elements: document.querySelectorAll('*').length,
      textLen: (document.body.innerText || '').trim().length,
      height: document.body.scrollHeight,
      /* A card whose content sits inside `ReportView`'s frames cannot be counted
         by walking the DOM: `sandbox=""` gives each frame an opaque origin, so
         `contentDocument` is unreachable by design. The document it carries is
         still right there in the attribute, so that is what gets measured. */
      framedChars: [...document.querySelectorAll('iframe[srcdoc]')].reduce(
        (n, f) => n + f.getAttribute('srcdoc').length,
        0
      )
    }
  })

  await page.screenshot({ path: join(SHOTS, `${slug}.png`), fullPage: false })

  const bgOk = probe.paneBg.every((bg, i) =>
    probe.panes === 1 ? bg === CANVAS_LIGHT : bg === (i === 0 ? CANVAS_LIGHT : CANVAS_DARK)
  )
  const dsCard = firstLine.includes('@dsCard')
  const hasContent = probe.elements > 30 || probe.framedChars > 500
  const pass = dsCard && bgOk && hasContent && probe.height > 80 && errors.length === 0 && missing.length === 0

  results.push({
    slug,
    pass,
    panes: probe.panes,
    tokensResolved: bgOk,
    dsCard,
    elements: probe.elements,
    framedChars: probe.framedChars,
    text: probe.textLen,
    h: probe.height,
    missing: [...new Set(missing)].slice(0, 3),
    errors: errors.slice(0, 2)
  })
}

await browser.close()
server.close()

const bad = results.filter((r) => !r.pass)
console.log(JSON.stringify({ total: results.length, failed: bad.length, results }, null, 1))
process.exit(bad.length ? 1 : 0)
