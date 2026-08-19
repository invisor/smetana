import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

/* `CLAUDE.md` read as text, which is the only way a test in this project can
   reach it: it is a document rather than a module, and nothing imports it.
   `tests/styles/tokens.test.js` is the precedent for that shape — a file read
   off disk and one rule pinned about it. The one thing done differently here is
   the path: that test resolves from `process.cwd()`, and this one resolves from
   its own location, because the document sits at the repository root rather
   than under `src/` and the test has to hold whichever directory vitest was
   started from.

   What is pinned is the size, and the reason is in the document itself, under
   "Where the rest of this document went": `CLAUDE.md` is loaded into the
   context window of **every** session whatever that session is about, so its
   length is a tax on work that has nothing to do with it. It once reached
   168 000 characters, and rewriting it shorter bought three days before it grew
   back. The fix that held was moving prose about one subsystem into
   `.claude/rules/`, which loads only when a file under its `paths:` frontmatter
   is opened; this test is the mechanical half of that rule, and the paragraph
   naming it is what an agent reads instead of checking. Without something here,
   the next time the file grows back nobody finds out until a person notices —
   which is exactly the method that paragraph declares no good.

   Deliberately a check on this file alone. `.claude/rules/*.md` are not
   measured and must not be: a rule is only paid for by the session that opens
   the code it covers, and that is the whole of why the prose moved there. */
const CLAUDE_MD = resolve(dirname(fileURLToPath(import.meta.url)), '../../CLAUDE.md')

/* Characters, not bytes: what costs is what a tokeniser sees, and the file is
   prose in which the two differ only by a handful of em-dashes.

   36 000 is today's file plus room to grow honestly. The document measured
   28 401 characters when this was written, so the budget leaves about 7 500 —
   a quarter again, or roughly two more sections the size of the one that
   explains where the rest of this document went. That is enough for a genuinely
   new project-wide rule and not enough for a subsystem's prose, which is the
   line this number is drawn to sit on. It is a budget, not a measurement: when
   the file shrinks, leave it alone, and when it stops fitting, the question to
   answer is which section is about one subsystem — not which number to raise. */
const BUDGET = 36000

describe('CLAUDE.md stays inside its budget', () => {
  it('is short enough to be worth loading into every session', () => {
    const size = readFileSync(CLAUDE_MD, 'utf8').length

    expect(
      size,
      [
        `CLAUDE.md is ${size} characters, over its ${BUDGET}-character budget.`,
        '',
        'This file is loaded into the context window of every session, whatever',
        'that session is about, so every character is paid for by work that has',
        'nothing to do with it. It once reached 168 000 characters that way.',
        '',
        'The fix is not to compress this file. Prose about one subsystem belongs',
        'in a rule file under .claude/rules/, with `paths:` frontmatter scoping',
        'it to the code it is about, so it loads only when a file it covers is',
        'read; add a row to the table in "Where the rest of this document went".',
        'What stays here is what is true in every session regardless of subject.',
        '',
        'Raising this number is a decision to spend more of every session on',
        'this document, and it needs a reason written next to it in this file.'
      ].join('\n')
    ).toBeLessThanOrEqual(BUDGET)
  })
})
