import { describe, expect, it } from 'vitest'
import { liveCheckBlock, projectName } from '../../../src/components/run/browserTools.js'

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

  it('blocks with its own words when the tool is there and another run holds it', () => {
    const reason = liveCheckBlock('browser', { ...PLAYWRIGHT, busy_project: '/Users/someone/other-app' })
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
})

describe('naming the project holding the browser', () => {
  it('is the folder, not the path somebody would have to read sideways', () => {
    expect(projectName('/Users/someone/Projects/smetana')).toBe('smetana')
    expect(projectName('/Users/someone/Projects/smetana/')).toBe('smetana')
    expect(projectName('C:\\Users\\someone\\smetana')).toBe('smetana')
  })

  it('a path with no folder in it is left as it is rather than emptied', () => {
    expect(projectName('smetana')).toBe('smetana')
    expect(projectName('/')).toBe('')
    expect(projectName(null)).toBe('')
  })
})
