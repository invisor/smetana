import { describe, expect, it } from 'vitest'
import {
  AGENT_MENU_W,
  CLOSE_LABEL,
  PIN_LABEL,
  UNPIN_LABEL,
  agentMenuItems,
  agentMenuLabel
} from '../../../src/components/agent/agentMenu.js'

const kinds = (items) => items.filter((item) => !item.type).map((item) => item.kind)
const labelOf = (items, kind) => items.find((item) => item.kind === kind).label
const disabled = (items, kind) => items.find((item) => item.kind === kind).disabled

/* An ordinary live row: pinnable, closable. */
const row = (over = {}) => ({ pinned: false, conversation: 'conv-a', starting: false, ...over })

describe('what an agent row offers', () => {
  /* The verbs and their order are the acceptance criteria of this task, and the
     consuming side of the pair is a `.vue` file no runner here can read — so
     this is the only mechanical check either half gets. */
  it('offers the two verbs, pinning first and closing last', () => {
    expect(kinds(agentMenuItems(row()))).toEqual(['pin', 'close'])
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
    const longest = agentMenuItems(row({ conversation: null }))
      .map((item) => item.label.length)
      .reduce((a, b) => Math.max(a, b))
    expect(longest * 6.4 + 70).toBeLessThan(AGENT_MENU_W)
  })
})
