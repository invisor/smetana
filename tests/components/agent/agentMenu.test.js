import { describe, expect, it } from 'vitest'
import {
  AGENT_MENU_W,
  CLEAR_LABEL,
  CLOSE_LABEL,
  PIN_LABEL,
  UNPIN_LABEL,
  agentMenuItems,
  agentMenuLabel
} from '../../../src/components/agent/agentMenu.js'

const kinds = (items) => items.filter((item) => !item.type).map((item) => item.kind)
const labelOf = (items, kind) => items.find((item) => item.kind === kind).label
const disabled = (items, kind) => items.find((item) => item.kind === kind).disabled

/* An ordinary live row of a project set to a harness that can clear:
   pinnable, clearable, closable. */
const row = (over = {}) => ({
  pinned: false,
  conversation: 'conv-a',
  starting: false,
  state: 'running',
  agent: 'claude',
  ...over
})

describe('what an agent row offers', () => {
  /* The verbs and their order are the acceptance criteria of this task, and the
     consuming side of the pair is a `.vue` file no runner here can read — so
     this is the only mechanical check either half gets. */
  it('offers the three verbs, pinning first and closing last', () => {
    expect(kinds(agentMenuItems(row()))).toEqual(['pin', 'clear', 'close'])
  })

  /* The label is the act and not the state: a row already pinned offers the way
     back out, which is the whole of what tells somebody the mark is theirs to
     remove. */
  it('names the act rather than the state', () => {
    expect(labelOf(agentMenuItems(row()), 'pin')).toBe(PIN_LABEL)
    expect(labelOf(agentMenuItems(row({ pinned: true })), 'pin')).toBe(UNPIN_LABEL)
  })

  it('closes an ordinary row with no reason to say', () => {
    const items = agentMenuItems(row())
    expect(labelOf(items, 'close')).toBe(CLOSE_LABEL)
    expect(disabled(items, 'close')).toBe(false)
    expect(disabled(items, 'pin')).toBe(false)
  })

  /* The one row that is offered on the strength of the harness rather than of
     the session: a live agent under a profile that has a word for forgetting a
     conversation, which today is Claude Code's `/clear`. */
  it('clears a live row whose harness has a command for it', () => {
    const items = agentMenuItems(row())
    expect(labelOf(items, 'clear')).toBe(CLEAR_LABEL)
    expect(disabled(items, 'clear')).toBe(false)
  })

  /* A state this file has never met is offered rather than refused, which is
     the softer way to be wrong: `toUiState` may grow a word, and the write's
     own guard answers for a row with nothing behind it. */
  it('offers the clear on a state it has never heard of', () => {
    expect(disabled(agentMenuItems(row({ state: 'ready' })), 'clear')).toBe(false)
    expect(disabled(agentMenuItems(row({ state: 'something-new' })), 'clear')).toBe(false)
  })
})

describe('what it refuses, and in what words', () => {
  /* "Cannot be closed" is drawn rather than enforced — the pinned row has no
     cross at all — and this is the same sentence in the menu. Greyed and
     worded, never left out: a row that vanishes tells nobody why. */
  it('refuses to close a pinned row, and says to unpin it first', () => {
    const items = agentMenuItems(row({ pinned: true }))
    expect(disabled(items, 'close')).toBe(true)
    expect(labelOf(items, 'close')).toBe(`${CLOSE_LABEL} — unpin it first`)
    expect(disabled(items, 'pin')).toBe(false)
  })

  /* The cross on a starting row is greyed for this already: the id such a row
     carries is this window's own, so there is no session to end. */
  it('refuses to close a row that has not started, and says so', () => {
    const items = agentMenuItems(row({ conversation: null, starting: true }))
    expect(disabled(items, 'close')).toBe(true)
    expect(labelOf(items, 'close')).toBe(`${CLOSE_LABEL} — it has not started yet`)
  })

  /* A pin has to outlive the session it was put on, and a row with no
     conversation id leaves nothing to remember it by after a restart. */
  it('refuses to pin a row with no conversation id, and says why', () => {
    const items = agentMenuItems(row({ conversation: null }))
    expect(disabled(items, 'pin')).toBe(true)
    expect(labelOf(items, 'pin')).toBe(`${PIN_LABEL} — nothing to remember it by`)
  })

  /* An offline row — the app's own record of an agent it was holding when it
     last closed, or a session whose process has gone. There is no input to
     write into either way, so both are the one sentence. */
  it('refuses to clear an offline row, and says it is not running', () => {
    for (const state of ['done', 'failed']) {
      const items = agentMenuItems(row({ state }))
      expect(disabled(items, 'clear')).toBe(true)
      expect(labelOf(items, 'clear')).toBe(`${CLEAR_LABEL} — it is not running`)
    }
  })

  /* The second before a spawn answers. The id such a row carries is this
     window's own rather than the worker's, so there is no session to write to —
     the same absence the cross is greyed for. */
  it('refuses to clear a row that has not started, and says so', () => {
    const items = agentMenuItems(row({ conversation: null, starting: true }))
    expect(disabled(items, 'clear')).toBe(true)
    expect(labelOf(items, 'clear')).toBe(`${CLEAR_LABEL} — it has not started yet`)
  })

  /* The refusal this verb exists to make: an agent waiting for an answer reads
     the next line written into it as that answer, so the clearing command would
     be a pick in somebody else's dialog. Two steps, not a confirmation. */
  it('refuses to clear an agent that is waiting, and says to answer it first', () => {
    const items = agentMenuItems(row({ state: 'needs-you' }))
    expect(disabled(items, 'clear')).toBe(true)
    expect(labelOf(items, 'clear')).toBe(`${CLEAR_LABEL} — answer it first`)
    /* And the other two rows are untouched by it: a waiting agent is still one
       somebody may pin or close. */
    expect(disabled(items, 'pin')).toBe(false)
    expect(disabled(items, 'close')).toBe(false)
  })

  /* A fact about the project rather than about the row, which is why it is
     asked first: every row of a project set to such a harness says the same
     thing. `CLEARS_BY_ID` is the front end's copy of `Profile::clear_command`,
     and it is asked before the row so both sides of the wire word a row refused
     twice over the same way. */
  it('refuses to clear under a harness with no command for it, whatever the row', () => {
    for (const over of [{}, { state: 'needs-you' }, { starting: true }]) {
      const items = agentMenuItems(row({ ...over, agent: 'codex' }))
      expect(disabled(items, 'clear')).toBe(true)
      expect(labelOf(items, 'clear')).toBe(`${CLEAR_LABEL} — this agent cannot do it`)
    }
  })

  /* No agent named at all is the same refusal and not an offer: the list is
     what the app was told to run, and a row promising on a guess is what the
     table exists to prevent. */
  it('refuses to clear when no agent is named', () => {
    expect(disabled(agentMenuItems({ conversation: 'conv-a', state: 'running' }), 'clear')).toBe(
      true
    )
  })
})

describe('the wording', () => {
  it('joins a reason onto a label with a dash, and leaves a plain label alone', () => {
    expect(agentMenuLabel('Close agent', 'unpin it first')).toBe('Close agent — unpin it first')
    expect(agentMenuLabel('Close agent', null)).toBe('Close agent')
  })

  /* The ceiling the refusals are worded against. `ContextMenu` clips a row
     rather than wrapping it and gives it no tooltip, so a label past this width
     is gone with no way back — see the file's own measurement. */
  it('leaves room for the longest label it can produce', () => {
    /* Every combination this file can be asked about, not one row of them: the
       clear row's four refusals are worded against this ceiling and each is a
       different length, so measuring one arrangement would leave three
       unmeasured. */
    const every = [
      row(),
      row({ pinned: true }),
      row({ conversation: null }),
      row({ conversation: null, starting: true }),
      row({ state: 'done' }),
      row({ state: 'failed' }),
      row({ state: 'needs-you' }),
      row({ agent: 'codex' }),
      row({ agent: null })
    ]
    const longest = every
      .flatMap((one) => agentMenuItems(one))
      .map((item) => item.label.length)
      .reduce((a, b) => Math.max(a, b))
    expect(longest * 6.4 + 70).toBeLessThan(AGENT_MENU_W)
  })
})
