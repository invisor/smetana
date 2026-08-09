/* Why the run dialog's live-check toggle is blocked, said in one sentence — or
   '' when nothing blocks it.

   One value rather than two, deliberately: the label the tooltip shows and the
   "is it blocked" flag are the same fact, and keeping them apart is how a
   toggle ends up disabled under an empty tooltip, or live under a sentence
   explaining why it is not.

   Pure and outside the component for the reason branchChoice.js is: a `.vue`
   file is the one thing no test in this repository can reach, and the whole of
   this rule is a decision about what somebody reads on screen.

   Scope is `[live_check].mode = "browser"` and nothing else. A `command` check
   needs no browser, so blocking it over a missing Playwright would be a plain
   untruth; `none` is already handled by `liveCheckAvailable` and its own note
   under the switch, which is a different reason with different words and stays
   exactly as it was. */

/* The two halves of "Playwright is available" are separate facts on the wire,
   because a configured server with no browsers downloaded drives nothing and
   the person has different work to do in each case — add the server, or run
   `npx playwright install`. Naming which half is missing is the point of the
   tooltip. */
const NO_MCP = "Playwright's MCP server is not in the agent's configuration"
const NO_BROWSERS = "Playwright's browsers are not downloaded"
const NO_EXTENSION = 'the Claude in Chrome extension was not found in a Chrome profile'

/* The project is named by its folder, the way every other project name on
   screen is: an absolute path in a tooltip sentence is a wall, and the last
   segment is what somebody recognises. Trailing separators are trimmed first so
   a path stored with one does not name the empty string. */
export function projectName(path) {
  const text = String(path ?? '')
  const trimmed = text.replace(/[/\\]+$/, '')
  const last = trimmed.split(/[/\\]/).pop()
  return last || trimmed
}

export function liveCheckBlock(mode, tools) {
  if (mode !== 'browser') return ''
  /* The answer arrives after the dialog does — `openRun` shows it and then goes
     to disk, the same order the branch list arrives in. Until it lands nothing
     is known, and blocking on "not known yet" would flick the toggle off for
     everybody on every open. The watcher in RunModal is what turns it off when
     a blocking answer does land. */
  if (!tools) return ''

  const playwright = Boolean(tools.playwright_mcp) && Boolean(tools.playwright_browsers)
  if (!playwright && !tools.extension) {
    /* Which Playwright half to name: the server first, since without it the
       browsers on disk are beside the point. */
    const missing = [tools.playwright_mcp ? NO_BROWSERS : NO_MCP, NO_EXTENSION]
    return `Nothing here can drive a browser: ${missing.join(' and ')}.`
  }

  /* Second, and only once there is something to hold: a tool that is not
     installed cannot also be busy, and saying both would be noise. This is the
     app's own runs and nothing more — a browser somebody is driving themselves,
     and the extension's busy-ness in any form, are outside what this process can
     see, and that gap is recorded rather than guessed at (runs/browser.rs). */
  if (tools.busy_project) {
    return `The run in ${projectName(tools.busy_project)} is driving the browser, so this one cannot at the same time.`
  }

  return ''
}
