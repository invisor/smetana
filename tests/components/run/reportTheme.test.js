import { describe, expect, it } from 'vitest'
import { documentFor, guarded, themed } from '../../../src/components/run/reportTheme.js'

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

/* The content policy, which is the other half of what shuts this frame.

   `sandbox=""` was measured against a local server logging every request and let
   a stylesheet, an image and a font through — the attribute has never had
   anything to say about subresources, and `csp` in `tauri.conf.json` is `null`,
   so a child frame inherits nothing either. These tests pin the string that
   closes it and, more importantly, pin where it lands: a policy the parser meets
   after the first `<link>` binds nothing. */
const CSP = 'http-equiv="Content-Security-Policy"'
const POLICY = "default-src 'none'; style-src 'unsafe-inline'; img-src data:"
const policy = () => `<meta ${CSP} content="${POLICY}">`

/* How many policies the parser would actually act on: the metas that are not
   inside a comment. Counting occurrences of the string would count the inert
   one too, which is precisely the mistake the rule itself used to make. */
const live = (html) => (html.replace(/<!--[\s\S]*?-->/g, '').match(/http-equiv="Content-Security-Policy"/g) || []).length

describe('guarded', () => {
  it('states the policy that refuses everything the document did not bring', () => {
    expect(guarded(doc())).toContain(`content="${POLICY}"`)
  })

  it('puts it immediately after the root tag, in front of the whole document', () => {
    // The whole of the correctness: a policy binds what follows it, so a meta
    // after the first stylesheet link is a meta that does nothing. The parser
    // opens an implicit head for it and ignores the explicit <head> that comes
    // next, which is what makes going in this early safe.
    expect(guarded(doc())).toContain(`<html lang="en"><meta ${CSP}`)
  })

  it('is not fooled by a <head> in the body of a document that opens none', () => {
    // Why the root tag is tried before the head and not after it. <head> here
    // is content, so a meta placed at it would have the body as its parent —
    // and a meta outside the head is ignored outright, with no console message
    // to say the frame is now unguarded. Measured against a probe server: this
    // document fetched its stylesheet under the old order.
    const stray = '<!doctype html><html lang="en"><body><p>x</p><head><p>y</p></body></html>'
    expect(guarded(stray)).toContain(`<html lang="en"><meta ${CSP}`)
    expect(guarded(stray)).not.toContain(`<head><meta ${CSP}`)
  })

  it('falls back to the head for a document that omits the root tag', () => {
    // <html> is an optional tag, so this is a valid document with no root to
    // sit behind — which is the whole reason the head branch is still here.
    const rootless = '<!doctype html><head><title>x</title></head><body><p>hi</p></body>'
    expect(guarded(rootless)).toContain(`<head><meta ${CSP}`)
  })

  it('puts it after the doctype and never in front of one', () => {
    // A doctype with anything before it is not a doctype, and the document
    // would fall into quirks mode over a security header.
    const rootless = '<!doctype html><p>a fragment somebody saved</p>'
    expect(guarded(rootless).startsWith('<!doctype html><meta ')).toBe(true)
  })

  it('covers a fragment that has no doctype, root or head at all', () => {
    expect(guarded('<p>hi</p>')).toBe(`<meta ${CSP} content="${POLICY}"><p>hi</p>`)
  })

  it('is not skipped for a document carrying the policy literal in a comment', () => {
    // The defect this replaced: the rule used to return early for any document
    // whose text *contained* the meta, which is a substring search and not a
    // check that a live policy sits in the head. A meta inside a comment is
    // inert, so such a file was handed a bare sandbox — an off switch operated
    // by whoever wrote the file. Measured: it fetched a stylesheet, an image
    // and a font.
    const commented = `<!doctype html><html lang="en"><head><!-- ${policy()} --></head><body>x</body></html>`
    const out = guarded(commented)
    expect(out).toContain(`<html lang="en"><meta ${CSP}`)
    expect(live(out)).toBe(1)
  })

  it('leaves the frame under the same policy when applied to its own output', () => {
    // Idempotence is about what the frame is bounded by, not about the bytes.
    // A second identical policy intersects with the first to exactly itself, so
    // stacking is harmless — which is what makes the unconditional insert above
    // free rather than a trade.
    const twice = guarded(guarded(doc()))
    expect(twice).toContain(`content="${POLICY}"`)
    expect(live(twice)).toBe(2)
    // And nothing else about the document has changed under the stacking.
    expect(twice.replaceAll(policy(), '')).toBe(doc())
  })

  it('leaves a policy the document brought itself, since two are an intersection', () => {
    // A second policy can only make the frame stricter, so there is no ordering
    // by which a file could talk its way out of ours.
    const own = '<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="default-src \'self\'"><title>x</title></head></html>'
    const out = guarded(own)
    expect(out).toContain(`content="default-src 'self'"`)
    expect(out).toContain(`content="${POLICY}"`)
  })

  it('states the policy even for an empty buffer, since the rule has no exceptions', () => {
    // A buffer still loading is the empty string, and it gets the policy like
    // everything else. Nothing here needs it — an empty document fetches
    // nothing — but an exception is a branch, and the one branch this rule used
    // to have was the way out of it.
    expect(guarded('')).toBe(policy())
  })

  it('answers an empty string for anything that is not one', () => {
    expect(guarded(null)).toBe('')
    expect(guarded(undefined)).toBe('')
  })
})

describe('documentFor', () => {
  it('is the theme and the policy at once, which is what ReportView calls', () => {
    const out = documentFor(doc(), 'dark')
    expect(out).toContain('<html lang="en" data-theme="dark">')
    expect(out).toContain(`content="${POLICY}"`)
  })

  it('states the policy even for a theme the stamp declines to honour', () => {
    // `themed` returns the document untouched for anything but the two painted
    // themes. The policy is not the theme's to depend on: a frame that reached
    // the network whenever a caller passed a bad string would be the defect
    // back again, through a different door.
    expect(documentFor(doc(), 'system')).toContain(`content="${POLICY}"`)
    expect(documentFor(doc(), undefined)).toContain(`content="${POLICY}"`)
  })

  it('states it for a document with no root tag, which the theme stamp skips', () => {
    expect(documentFor('<p>hi</p>', 'dark')).toContain(`content="${POLICY}"`)
  })
})
