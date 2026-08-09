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

/* The project holding the browser is named by its folder, the way every other
   project name on screen is — an absolute path in a tooltip sentence is a wall.
   `basename` is the tree's one copy of that rule (src/paths.js); a fourth was
   written here first and disagreed with it about a root path, which reached the
   screen as an empty gap in this very sentence. */
import { basename } from '../../paths.js'

/* The two halves of "Playwright is available" are separate facts on the wire,
   because a configured server with no browsers downloaded drives nothing and
   the person has different work to do in each case — add the server, or run
   `npx playwright install`. Naming which half is missing is the point of the
   tooltip. */
const NO_MCP = "Playwright's MCP server is not in the agent's configuration"
const NO_BROWSERS = "Playwright's browsers are not downloaded"
const NO_EXTENSION = 'the Claude in Chrome extension was not found in a Chrome profile'

export function liveCheckBlock(mode, tools) {
  if (mode !== 'browser') return ''
  /* The answer arrives after the dialog does — `openRun` shows it and then goes
     to disk, the same order the branch list arrives in. Until it lands nothing
     is known, and blocking on "not known yet" would flick the toggle off for
     everybody on every open. The watcher in RunModal is what turns it off when
     a blocking answer does land. */
  if (!tools) return ''

  const playwright = Boolean(tools.playwright_mcp) && Boolean(tools.playwright_browsers)
  const extension = Boolean(tools.extension)

  if (!playwright && !extension) {
    /* Which Playwright half to name: the server first, since without it the
       browsers on disk are beside the point. */
    const missing = [tools.playwright_mcp ? NO_BROWSERS : NO_MCP, NO_EXTENSION]
    return `Nothing here can drive a browser: ${missing.join(' and ')}.`
  }

  /* Busy-ness is a **Playwright** fact and only ever that, so it may block only
     where Playwright is the tool that would be used — which means the extension
     is not there. The app can see its own runs, and a Playwright run in another
     project genuinely holds the one persistent profile; it can see nothing at
     all about the extension, because a Chrome window holding it is not
     something this process can observe (runs/browser.rs records that gap).

     So a machine whose only tool is Claude in Chrome never blocks here. Doing
     otherwise disabled the toggle over a tool nobody had shown to be held — the
     app would have been guessing, and about the one half it has already said it
     cannot know. A browser somebody is driving themselves is invisible the same
     way, and is not guessed at either. */
  if (!extension && tools.busy_project) {
    return `The run in ${basename(tools.busy_project)} is driving the browser, so this one cannot at the same time.`
  }

  return ''
}
