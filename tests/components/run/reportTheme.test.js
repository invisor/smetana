import { describe, expect, it } from 'vitest'
import { themed } from '../../../src/components/run/reportTheme.js'

/* A document shaped like the one `report.rs` writes, cut down to the root tag:
   what this rule reads is that tag, and the body is here only so the assertions
   can show it came back untouched. */
const doc = (root = '<html lang="en">') =>
  `<!doctype html>${root}<head><title>Task report</title></head><body><h1>Task report</h1></body></html>`

describe('themed', () => {
  it('names the dark theme on the document root', () => {
    expect(themed(doc(), 'dark')).toContain('<html lang="en" data-theme="dark">')
  })

  it('names the light theme the same way', () => {
    expect(themed(doc(), 'light')).toContain('<html lang="en" data-theme="light">')
  })

  it('adds the attribute rather than replacing the tag it found', () => {
    // A document that declares its language must not lose it on the way into
    // the frame.
    const out = themed(doc(), 'dark')
    expect(out).toContain('lang="en"')
    expect(out).toContain('<body><h1>Task report</h1></body>')
  })

  it('marks a root tag that carries nothing else', () => {
    expect(themed(doc('<html>'), 'light')).toContain('<html data-theme="light">')
  })

  it('replaces a theme the document already named, so the app tab follows the app', () => {
    // Inside this app's tab the app is the one showing the document. An
    // attribute found in a file could only come from a hand edit or a future
    // writer, and honouring it would leave one tab light in a dark window.
    expect(themed(doc('<html lang="en" data-theme="light">'), 'dark')).toContain(
      '<html lang="en" data-theme="dark">'
    )
    expect(themed(doc("<html data-theme='dark'>"), 'light')).toContain(
      '<html data-theme="light">'
    )
  })

  it('applies to its own output without accumulating anything', () => {
    // The rule is idempotent, and the second theme wins: a theme change
    // recomputes the frame's string, and nothing about that may depend on how
    // many times the rule has run over it.
    const once = themed(doc(), 'dark')
    expect(themed(once, 'dark')).toBe(once)
    expect(themed(once, 'light')).toBe(themed(doc(), 'light'))
    expect((themed(once, 'light').match(/data-theme/g) ?? []).length).toBe(1)
  })

  it('hands back a document with no root tag exactly as it arrived', () => {
    // A buffer still loading, one that failed to read, or a file mangled since
    // it was written. There is nowhere to put the attribute, and inventing a
    // root would be rewriting somebody's document.
    expect(themed('', 'dark')).toBe('')
    expect(themed('not a document at all', 'dark')).toBe('not a document at all')
    expect(themed('<!doctype html><body>no root here</body>', 'light')).toBe(
      '<!doctype html><body>no root here</body>'
    )
  })

  it('leaves the document alone for a theme it cannot honour', () => {
    // Every one of these is the same fact: nobody said dark or light. The
    // document then falls back on `prefers-color-scheme`, which is exactly where
    // it stands when somebody opens the file in a browser.
    for (const theme of ['system', '', null, undefined, 'DARK', 7]) {
      expect(themed(doc(), theme)).toBe(doc())
    }
  })

  it('marks the first root tag and no later one', () => {
    // `report.rs` escapes every `<` it writes, so a second one can only be in a
    // document somebody has edited — and the first is still the real root.
    const twice = '<html lang="en"><body>&lt;html&gt; and <html lang="ru"></body></html>'
    const out = themed(twice, 'dark')
    expect(out).toContain('<html lang="en" data-theme="dark">')
    expect(out).toContain('<html lang="ru">')
  })

  it('answers with the empty string for html that is not a string at all', () => {
    // `ReportView` defaults its prop to `''`, but the buffer behind it is a
    // store's and may be absent; a rule that threw here would blank the tab.
    expect(themed(null, 'dark')).toBe('')
    expect(themed(undefined, 'dark')).toBe('')
  })
})
