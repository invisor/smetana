import { describe, expect, it } from 'vitest'
import { liveCheckBlock } from '../../../src/components/run/browserTools.js'

const NOTHING = { playwright_mcp: false, playwright_browsers: false, extension: false, busy_project: null }
const PLAYWRIGHT = { ...NOTHING, playwright_mcp: true, playwright_browsers: true }
const EXTENSION = { ...NOTHING, extension: true }

describe('why the run dialog blocks its live-check toggle', () => {
  it('leaves the toggle alone whatever the machine looks like, unless the check opens a browser', () => {
    // A declared command needs no browser, and `none` is a different reason
    // with its own note under the switch.
    expect(liveCheckBlock('command', NOTHING)).toBe('')
    expect(liveCheckBlock('none', NOTHING)).toBe('')
    expect(liveCheckBlock(undefined, NOTHING)).toBe('')
  })

  it('says nothing while the answer is still on its way', () => {
    // The dialog opens first and the answer follows it. Blocking on "not known
    // yet" would flick the toggle off on every open, for everybody.
    expect(liveCheckBlock('browser', null)).toBe('')
    expect(liveCheckBlock('browser', undefined)).toBe('')
  })

  it('blocks with both tools named when neither is there', () => {
    const reason = liveCheckBlock('browser', NOTHING)
    expect(reason).toContain("Playwright's MCP server")
    expect(reason).toContain('Claude in Chrome')
  })

  it('names the downloaded browsers rather than the server when only that half is missing', () => {
    // Different work to do: adding a server entry against running
    // `npx playwright install`, and the tooltip is the only thing that says so.
    const reason = liveCheckBlock('browser', { ...NOTHING, playwright_mcp: true })
    expect(reason).toContain("Playwright's browsers are not downloaded")
    expect(reason).not.toContain('MCP server')
  })

  it('leaves the toggle live when either tool is there', () => {
    expect(liveCheckBlock('browser', PLAYWRIGHT)).toBe('')
    expect(liveCheckBlock('browser', EXTENSION)).toBe('')
    // The extension alone is enough even with Playwright configured and empty.
    expect(liveCheckBlock('browser', { ...EXTENSION, playwright_mcp: true })).toBe('')
  })

  it('blocks with its own words when Playwright is the tool and another run holds it', () => {
    const reason = liveCheckBlock('browser', { ...PLAYWRIGHT, busy_project: '/Users/someone/other-app' })
    // Named by its folder, not by a path somebody would read sideways.
    expect(reason).toContain('other-app')
    expect(reason).toContain('driving the browser')
    // Not the "nothing found" sentence: the tool is there.
    expect(reason).not.toContain('MCP server')
  })

  it('does not call a tool that is missing busy as well', () => {
    // A tool nobody has cannot also be held by somebody, and saying both would
    // be two reasons for one blocked toggle.
    const reason = liveCheckBlock('browser', { ...NOTHING, busy_project: '/Users/someone/other-app' })
    expect(reason).toContain("Playwright's MCP server")
    expect(reason).not.toContain('other-app')
  })

  it('never blocks on busy-ness where the extension is the tool that would be used', () => {
    // Busy-ness is a Playwright fact. The app can see its own Playwright runs
    // and can see nothing whatever about a Chrome window holding the extension,
    // so blocking here would disable the toggle over a tool nobody has shown to
    // be held — guessing about exactly the half it has said it cannot know.
    expect(liveCheckBlock('browser', { ...EXTENSION, busy_project: '/Users/someone/other-app' })).toBe('')
    // Both tools present: the extension is still a way through, so still ''.
    expect(
      liveCheckBlock('browser', { ...PLAYWRIGHT, extension: true, busy_project: '/Users/someone/other-app' })
    ).toBe('')
  })
})
