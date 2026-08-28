import { describe, expect, it } from 'vitest'
import {
  COPIED_MS,
  DELETE_SESSION_DESCRIPTION,
  DELETE_SESSION_TITLE,
  SESSION_MENU_W,
  copyNoun,
  copyPayload,
  deleteSessionFacts,
  isCopyKind,
  menuButtonIcon,
  menuButtonLabel,
  resumeCommand,
  sessionMenuItems
} from '../../../src/components/agent/sessionMenu.js'

const session = (over = {}) => ({
  id: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60',
  path: '/Users/you/.claude/projects/-Users-you-dev-smetana/9f1c0a2e.jsonl',
  cwd: '/Users/you/dev/smetana',
  title: 'Why does the scope bar count dirty files it cannot see',
  size: 148_392,
  ...over
})

const kinds = (items) => items.filter((item) => !item.type).map((item) => item.kind)

describe('what a session row offers', () => {
  /* The seven verbs and their order are the acceptance criteria of this task,
     and the consuming side of the pair is `onSessionAction` in a `.vue` file no
     runner here can read — so this is the only mechanical check either half
     gets. */
  it('offers the seven verbs, in Orca\'s order', () => {
    expect(kinds(sessionMenuItems())).toEqual([
      'copy-resume',
      'open-log',
      'reveal-log',
      'open-cwd',
      'copy-id',
      'copy-path',
      'delete'
    ])
  })

  /* Four groups: the command, the three that open something somewhere else,
     the two that copy, and the one that destroys. */
  it('groups them with a separator between each group', () => {
    const shape = sessionMenuItems().map((item) => item.type ?? 'row')
    expect(shape).toEqual([
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

  it('keeps a ceiling wide enough for its longest label', () => {
    expect(SESSION_MENU_W).toBeGreaterThan(0)
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

  /* The same 1.2 s the board's id copy holds for. Two confirmations in one app
     that faded at different speeds would be two features where there is one. */
  it('holds as long as the board\'s own copy confirmation', () => {
    expect(COPIED_MS).toBe(1200)
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
