import { describe, expect, it } from 'vitest'
import {
  COPIED_MS,
  DELETE_SESSION_DESCRIPTION,
  DELETE_SESSION_TITLE,
  FORK_LABEL,
  RESUME_LABEL,
  SESSION_MENU_W,
  copyNoun,
  copyPayload,
  deleteSessionFacts,
  isCopyKind,
  menuButtonIcon,
  menuButtonLabel,
  resumeAvailability,
  resumeCommand,
  resumeMenuLabel,
  resumeReasonLine,
  resumeReasonLines,
  sessionMenuItems
} from '../../../src/components/agent/sessionMenu.js'

const session = (over = {}) => ({
  id: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60',
  path: '/Users/you/.claude/projects/-Users-you-dev-smetana/9f1c0a2e.jsonl',
  cwd: '/Users/you/dev/smetana',
  cwdExists: true,
  title: 'Why does the scope bar count dirty files it cannot see',
  size: 148_392,
  ...over
})

/* The project as it is configured out of the box. Written once because every
   test about the two launching verbs has to say which agent the project is set
   to, and the answer decides the whole rule. */
const claude = { agent: 'claude' }
const forking = { agent: 'claude', fork: true }

const kinds = (items) => items.filter((item) => !item.type).map((item) => item.kind)

describe('what a session row offers', () => {
  /* The verbs and their order are the acceptance criteria of this task, and the
     consuming side of the pair is `onSessionAction` in a `.vue` file no runner
     here can read — so this is the only mechanical check either half gets. All
     nine Orca has, and the two that start an agent lead them the way they lead
     Orca's own menu. */
  it('offers the nine verbs, in Orca\'s order', () => {
    expect(kinds(sessionMenuItems())).toEqual([
      'resume',
      'fork',
      'copy-resume',
      'open-log',
      'reveal-log',
      'open-cwd',
      'copy-id',
      'copy-path',
      'delete'
    ])
  })

  /* Five groups: the two that start an agent, the one that hands a command
     over, the three that open something somewhere else, the two that copy, and
     the one that destroys. */
  it('groups them with a separator between each group', () => {
    const shape = sessionMenuItems().map((item) => item.type ?? 'row')
    expect(shape).toEqual([
      'row',
      'row',
      'separator',
      'row',
      'separator',
      'row',
      'row',
      'row',
      'separator',
      'row',
      'row',
      'separator',
      'row'
    ])
  })

  /* Delete is the only destructive row, and `danger` is what `ContextMenu`
     reads to reach for `--status-failed-fg`. Nothing here names a colour, which
     is the point: a hex in this file would be the rule this system has about
     status colours broken in the one place a test can see it. */
  it('marks only the delete as destructive', () => {
    const danger = sessionMenuItems().filter((item) => item.tone === 'danger')
    expect(danger.map((item) => item.kind)).toEqual(['delete'])
    expect(danger[0].tone).toBe('danger')
  })

  it('gives every row a glyph, since a menu row is never colour alone', () => {
    for (const item of sessionMenuItems().filter((item) => !item.type)) {
      expect(item.icon).toBeTruthy()
    }
  })

  /* The reveal names the platform's own word for the thing it opens, borrowed
     from the file tree's menu rather than written again — the two menus say the
     same word on the same machine. */
  it('names the file manager the platform actually has', () => {
    const label = (ua) =>
      sessionMenuItems({ userAgent: ua }).find((item) => item.kind === 'reveal-log').label
    expect(label('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)')).toBe('Reveal log in Finder')
    expect(label('Mozilla/5.0 (Windows NT 10.0; Win64; x64)')).toBe('Reveal log in Explorer')
    expect(label('Mozilla/5.0 (X11; Linux x86_64)')).toBe('Reveal log in file manager')
  })

  /* A delete in flight greys the whole menu: a bd write does the same on a
     card, and for the same reason — a live menu during that second invites a
     second choice racing the first. */
  it('freezes every row while something is in flight', () => {
    const items = sessionMenuItems({ busy: true }).filter((item) => !item.type)
    expect(items.every((item) => item.disabled)).toBe(true)
    expect(sessionMenuItems().every((item) => !item.disabled)).toBe(true)
  })

  /* The ceiling is measured against the longest label this file can produce,
     which is one of the two greyed launching rows carrying its own reason —
     every other row is a verb of two or three words. `ContextMenu` clips at the
     ceiling with an ellipsis and a menu row has no tooltip, so a label past it
     is gone with no way back.

     A test cannot measure a font, so what it holds is the *arithmetic* the
     ceiling was chosen by: 70px of chrome and 6.4px a character, which is what
     the resume's sentence measured at in the webview (324px for 51 characters
     at `--text-sm`) rounded up. What it catches is a reason reworded longer
     than the panel it has to fit in, which is the way this number goes wrong. */
  it('keeps a ceiling wide enough for its longest label', () => {
    const longest = sessionMenuItems({
      resume: resumeAvailability({}, {}),
      fork: resumeAvailability({}, { fork: true })
    })
      .filter((item) => !item.type)
      .map((item) => item.label)
      .reduce((a, b) => (b.length > a.length ? b : a))
    expect(70 + longest.length * 6.4).toBeLessThan(SESSION_MENU_W)
  })
})

describe('bringing a session back as a live agent', () => {
  /* The ordinary case: this project's agent can be told to resume, the
     directory the session ran in is still there, so the row is live. */
  it('offers the resume for a session whose directory is still on disk', () => {
    const answer = resumeAvailability(session(), claude)
    expect(answer).toEqual({ available: true, reason: null })
    const row = sessionMenuItems({ resume: answer }).find((item) => item.kind === 'resume')
    expect(row.label).toBe(RESUME_LABEL)
    expect(row.disabled).toBe(false)
  })

  /* A worktree is removed once its task is merged and the transcript stays
     behind, so this is an ordinary row rather than an exotic one. `--resume`
     resolves an id against the directory it is run in, and starting the agent
     at the project root instead would be an agent reading a tree its own
     transcript never mentions — so the verb is refused and says why. */
  it('refuses a session whose working directory is gone, and says so on the row', () => {
    const answer = resumeAvailability(session({ cwdExists: false }), claude)
    expect(answer.available).toBe(false)
    const row = sessionMenuItems({ resume: answer }).find((item) => item.kind === 'resume')
    expect(row.disabled).toBe(true)
    expect(row.label).toBe(`${RESUME_LABEL} — the working directory is gone`)
  })

  it('refuses a transcript that recorded no working directory at all', () => {
    const answer = resumeAvailability(session({ cwd: '' }), claude)
    expect(answer.available).toBe(false)
    expect(resumeMenuLabel(answer.reason)).toBe(`${RESUME_LABEL} — no working directory recorded`)
  })

  /* `--resume <id>` is Claude Code's grammar and this app does not guess
     anybody else's: `Profile::resume_args` in `agents/codex.rs` keeps its
     default `None`, and this is the front-end half of that pair. */
  it('refuses every session when the project is set to an agent that cannot resume', () => {
    for (const agent of ['codex', '', 'something-new']) {
      const answer = resumeAvailability(session(), { agent })
      expect(answer.available).toBe(false)
      expect(answer.reason).toBe('this agent cannot resume by id')
    }
  })

  /* A record that does not carry the field is one this front end cannot answer
     for — an older worker, a hand-written fixture — and the softer way to be
     wrong is to offer the verb: the spawn's own guard refuses a directory that
     is not there and says so in words, while greying the row would take a
     working session away with nothing to explain it. */
  it('offers the resume for a record that never mentioned whether the directory is there', () => {
    const { cwdExists, ...older } = session()
    expect(cwdExists).toBe(true)
    expect(resumeAvailability(older, claude).available).toBe(true)
  })

  /* One wording for one refusal, set two ways: a lowercase fragment joined onto
     the menu row with a dash, and the same fragment as a sentence under the
     card's button. Two tables would have been two accounts of one thing. */
  it('sets the same reason as a sentence for the opened card', () => {
    expect(resumeReasonLine('the working directory is gone')).toBe(
      'The working directory is gone.'
    )
    expect(resumeReasonLine(null)).toBe('')
  })

  it('says nothing but the verb when there is nothing to explain', () => {
    expect(resumeMenuLabel(null)).toBe(RESUME_LABEL)
  })

  /* A delete in flight greys the resume with everything else, and the label
     stays the plain verb: the row is not refused, it is waiting. */
  it('freezes the resume while a delete runs, without claiming a reason', () => {
    const row = sessionMenuItems({ busy: true }).find((item) => item.kind === 'resume')
    expect(row.disabled).toBe(true)
    expect(row.label).toBe(RESUME_LABEL)
  })
})

describe('carrying a session on in a new one', () => {
  /* Orca calls this row `Continue in New Session…` and the ellipsis is a
     promise to ask the person something. Ours asks nothing — it starts an agent
     exactly as the row above it does — so the label carries none, and this is
     the one mechanical check of a rule that is otherwise a sentence in a
     comment. Sentence case with it, as every label in this system is. */
  it('is called Continue in a new session, with no ellipsis on it', () => {
    expect(FORK_LABEL).toBe('Continue in a new session')
    expect(FORK_LABEL).not.toMatch(/[.…]/)
    const row = sessionMenuItems().find((item) => item.kind === 'fork')
    expect(row.label).toBe(FORK_LABEL)
    expect(row.disabled).toBe(false)
  })

  /* The row sits directly under the resume and draws its own glyph: what the
     verb does to a transcript is branch it, and the resume's `play` on both
     would have made two rows that differ only in their words. */
  it('stands beside the resume in the group that starts an agent', () => {
    const [first, second] = sessionMenuItems()
    expect([first.kind, second.kind]).toEqual(['resume', 'fork'])
    expect(second.icon).toBe('git-fork')
    expect(second.icon).not.toBe(first.icon)
  })

  /* A worktree removed once its task merged takes both verbs with it, and in
     the same words: `--resume` resolves an id against the directory it is run
     in either way, so a fork has no more of a place to run than a resume. */
  it('is refused with the resume when the working directory is gone', () => {
    const answer = resumeAvailability(session({ cwdExists: false }), forking)
    expect(answer.available).toBe(false)
    expect(answer.reason).toBe(resumeAvailability(session({ cwdExists: false }), claude).reason)
    const row = sessionMenuItems({ fork: answer }).find((item) => item.kind === 'fork')
    expect(row.disabled).toBe(true)
    expect(row.label).toBe(`${FORK_LABEL} — the working directory is gone`)
  })

  it('is refused with the resume for a transcript that recorded no directory', () => {
    const answer = resumeAvailability(session({ cwd: '' }), forking)
    expect(answer.available).toBe(false)
    expect(answer.reason).toBe('no working directory recorded')
  })

  /* The capability is the profile's own answer — `Profile::fork_args` in Rust,
     and not `--fork-session` appended to somebody else's `--resume` — so the
     refusal is worded about forking rather than about resuming. A harness that
     reopens a transcript and cannot branch one is the shape this keeps room
     for. */
  it('says it is the forking the agent cannot do, not the resuming', () => {
    for (const agent of ['codex', '', 'something-new']) {
      const answer = resumeAvailability(session(), { agent, fork: true })
      expect(answer.available).toBe(false)
      expect(answer.reason).toBe('this agent cannot fork')
    }
  })

  /* The opened card's account of two greyed buttons. One refusal in two
     identical words is one line — the alternative reads as two faults — and two
     genuinely different answers are two. */
  it('says one shared reason once and two different ones twice', () => {
    expect(resumeReasonLines('the working directory is gone', 'the working directory is gone')).toEqual([
      'The working directory is gone.'
    ])
    expect(
      resumeReasonLines('this agent cannot resume by id', 'this agent cannot fork')
    ).toEqual(['This agent cannot resume by id.', 'This agent cannot fork.'])
    expect(resumeReasonLines(null, null)).toEqual([])
  })

  /* A delete in flight greys this one with everything else too, and the label
     stays the plain verb: the row is not refused, it is waiting. */
  it('freezes while a delete runs, without claiming a reason', () => {
    const row = sessionMenuItems({ busy: true }).find((item) => item.kind === 'fork')
    expect(row.disabled).toBe(true)
    expect(row.label).toBe(FORK_LABEL)
  })
})

describe('the resume command', () => {
  /* The working directory is in it because `claude --resume` resolves an id
     against the folder it is run in: the same id elsewhere is a session Claude
     Code has never heard of. */
  it('goes to the working directory and resumes by id', () => {
    expect(resumeCommand(session())).toBe(
      "cd '/Users/you/dev/smetana' && claude --resume '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60'"
    )
  })

  it('quotes a path with a space in it so the shell keeps it whole', () => {
    expect(resumeCommand(session({ cwd: '/Users/you/My Projects/smetana' }))).toContain(
      "cd '/Users/you/My Projects/smetana'"
    )
  })

  /* The one character single quotes cannot hold. Closed, escaped and reopened,
     which is what every shell manual writes — without it the line is torn in
     half at the apostrophe and the rest of it becomes a second argument. */
  it('survives an apostrophe in the path', () => {
    const command = resumeCommand(session({ cwd: "/Users/you/kate's work" }))
    expect(command).toContain("cd '/Users/you/kate'\\''s work'")
  })

  it('resumes without a cd when the session recorded no directory', () => {
    expect(resumeCommand(session({ cwd: null }))).toBe(
      "claude --resume '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60'"
    )
  })

  /* Nothing rather than a command that would resume nothing: the caller reads
     an empty string as "there is nothing to copy" and says so on the button
     instead of emptying somebody's clipboard. */
  it('answers with nothing at all for a record with no id', () => {
    expect(resumeCommand({ cwd: '/p' })).toBe('')
    expect(resumeCommand(null)).toBe('')
  })
})

describe('the three verbs that copy', () => {
  it('knows which kinds copy and which do not', () => {
    expect(isCopyKind('copy-id')).toBe(true)
    expect(isCopyKind('copy-path')).toBe(true)
    expect(isCopyKind('copy-resume')).toBe(true)
    expect(isCopyKind('open-log')).toBe(false)
    expect(isCopyKind('delete')).toBe(false)
  })

  it('puts the session id, the log path and the command on the clipboard', () => {
    expect(copyPayload('copy-id', session())).toBe('9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60')
    expect(copyPayload('copy-path', session())).toBe(
      '/Users/you/.claude/projects/-Users-you-dev-smetana/9f1c0a2e.jsonl'
    )
    expect(copyPayload('copy-resume', session())).toBe(resumeCommand(session()))
  })

  it('names each of them for the sentence that confirms it', () => {
    expect(copyNoun('copy-id')).toBe('session id')
    expect(copyNoun('copy-path')).toBe('log path')
    expect(copyNoun('copy-resume')).toBe('resume command')
  })

  /* The table is null-prototype, so a name every object inherits is answered
     with the fall-back rather than with a function or with a prototype. */
  it('holds for the names an object inherits, which it does not have', () => {
    for (const name of ['constructor', '__proto__', 'toString', 'valueOf']) {
      expect(isCopyKind(name)).toBe(false)
      expect(copyPayload(name, session())).toBe('')
      expect(copyNoun(name)).toBe('')
    }
  })
})

describe('what the menu button says about a copy', () => {
  /* The confirmation lands here rather than in a toast because the menu closes
     on the way out and this trigger is what is left of it. Three strings, and
     they are one of this feature's acceptance criteria. */
  it('invites the press before anything has been asked', () => {
    expect(menuButtonLabel('')).toBe('Session actions')
    expect(menuButtonIcon('')).toBe('ellipsis')
  })

  it('confirms a copy and names what was copied', () => {
    expect(menuButtonLabel('copied', 'session id')).toBe('Copied the session id')
    expect(menuButtonIcon('copied')).toBe('check')
  })

  it('says so when the clipboard refused, on the same button', () => {
    expect(menuButtonLabel('failed', 'log path')).toBe('Could not copy the log path')
    expect(menuButtonIcon('failed')).toBe('x')
  })

  it('still says something with no noun to name', () => {
    expect(menuButtonLabel('copied')).toBe('Copied')
    expect(menuButtonLabel('failed')).toBe('Could not copy')
  })

  it('falls back to the invitation for a state it has never heard of', () => {
    expect(menuButtonLabel(undefined)).toBe('Session actions')
    expect(menuButtonLabel('COPIED')).toBe('Session actions')
    expect(menuButtonIcon('pending')).toBe('ellipsis')
  })

  /* Re-exported from `kanban/copyId.js` rather than declared here: the number
     itself is pinned in that module's own test, and what is checked here is
     that a component drawing a session row gets the same one — a second
     declaration is exactly what this line exists to make impossible. */
  it('holds as long as every other copy confirmation in the app', async () => {
    const { COPIED_MS: owned } = await import('../../../src/components/kanban/copyId.js')
    expect(COPIED_MS).toBe(owned)
  })
})

describe('what the delete dialog names', () => {
  /* Three facts, and they are the acceptance criterion: which session, which
     file, how much of it. The heading deliberately holds none of them — a
     36-character UUID is not what a person recognises a conversation by. */
  it('names the id, the path and the size', () => {
    expect(deleteSessionFacts(session())).toEqual([
      { label: 'Session id', value: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60' },
      {
        label: 'Log path',
        value: '/Users/you/.claude/projects/-Users-you-dev-smetana/9f1c0a2e.jsonl'
      },
      { label: 'Size', value: '144.9 KiB' }
    ])
  })

  /* The same units the Storage tab speaks in, because it is the same function:
     one app calling 1024 bytes a kilobyte in one window and a kibibyte in
     another would be two vocabularies for one number. */
  it('measures a large transcript in the units the rest of the app uses', () => {
    const size = (bytes) =>
      deleteSessionFacts(session({ size: bytes })).find((fact) => fact.label === 'Size').value
    expect(size(16_402_771)).toBe('15.6 MiB')
    expect(size(0)).toBe('0 bytes')
  })

  /* A record from a build that did not send a size is answered with a dash
     rather than with a zero: zero is a fact about the disk and would read as
     "there is nothing in it". */
  it('draws a dash for a size nobody told it', () => {
    const facts = deleteSessionFacts(session({ size: undefined }))
    expect(facts.find((fact) => fact.label === 'Size').value).toBe('—')
  })

  /* One copy of the caption, called by the component's heading and by the
     announcement the app window hands to `set_title`. The two cannot drift
     because there is only one of them. */
  it('captions the window once, for both the frame and the body', () => {
    expect(DELETE_SESSION_TITLE).toBe('Delete this session?')
  })

  /* The consequence rather than an apology, and the two clauses that make this
     different from every other delete in the app: not a repository, and not the
     trash. */
  it('says the deletion cannot be undone and why there is nothing to undo it from', () => {
    expect(DELETE_SESSION_DESCRIPTION).toContain('no undo')
    expect(DELETE_SESSION_DESCRIPTION).toContain('trash')
  })
})
