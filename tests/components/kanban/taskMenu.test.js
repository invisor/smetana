import { describe, expect, it } from 'vitest'
import {
  MENU_W,
  STATUSES,
  statusOptions,
  taskMenuItems
} from '../../../src/components/kanban/taskMenu.js'

const base = { bdStatus: 'open', runnable: true, runBlockedReason: '', busy: false }
const kinds = (items) => items.filter((i) => i.type !== 'separator').map((i) => i.kind)
const find = (items, kind) => items.find((i) => i.kind === kind)

describe('MENU_W', () => {
  it('is one number, and a number rather than a length', () => {
    /* Three callers hand it to `MenuButton`'s `width`, which is a Number prop
       it does arithmetic against: a string of px would be handed straight to
       the placement maths and clip every long row silently. The measurement
       behind the value is in the module's own comment. */
    expect(typeof MENU_W).toBe('number')
    expect(MENU_W).toBe(424)
  })
})

describe('statusOptions', () => {
  it('offers the three a person is given, and no more', () => {
    expect(statusOptions('open')).toEqual(STATUSES)
  })

  it('appends the status the issue actually holds when it is outside the three', () => {
    // With no matching option the list would claim the issue is Ready.
    // in_progress is an agent's, but the menu still has to tell the truth.
    expect(statusOptions('in_progress')).toEqual([
      ...STATUSES,
      { value: 'in_progress', label: 'In progress' }
    ])
  })

  it('appends nothing when there is no status to append', () => {
    // TaskCard defaults bdStatus to '', so a caller that forgets it reaches
    // here. Appending would draw a fourth row with an empty label, checked and
    // refused — a blank line in a menu with nothing to say what it is.
    expect(statusOptions('')).toEqual(STATUSES)
    expect(statusOptions(undefined)).toEqual(STATUSES)
  })
})

describe('taskMenuItems', () => {
  it('offers the five actions, with delete last and behind a separator', () => {
    const items = taskMenuItems(base)
    expect(kinds(items)).toEqual(['run', 'ask-agent', 'follow-up', 'move', 'delete'])
    expect(items.at(-2)).toEqual({ type: 'separator' })
    expect(find(items, 'delete').tone).toBe('danger')
  })

  it('greys the run and says why, in the row rather than in a tooltip', () => {
    const items = taskMenuItems({ ...base, runBlockedReason: 'a run is already going on this task' })
    expect(find(items, 'run')).toMatchObject({
      disabled: true,
      label: 'Run this — a run is already going on this task'
    })
  })

  it('greys the run on a card that cannot be run at all, with no dangling dash', () => {
    // Done and blocked cards: there is nothing to run, and no scope is busy.
    expect(find(taskMenuItems({ ...base, runnable: false }), 'run')).toMatchObject({
      disabled: true,
      label: 'Run this'
    })
  })

  it('still offers the other four on a card that cannot be run', () => {
    const items = taskMenuItems({ ...base, runnable: false })
    for (const kind of ['ask-agent', 'follow-up', 'move', 'delete']) {
      expect(find(items, kind).disabled).toBeFalsy()
    }
  })

  it('offers a follow-up between editing the task and moving it', () => {
    // Between the two rows that are also about what an agent does with a task's
    // text, and above the move — which is about the board rather than the work.
    const items = taskMenuItems(base)
    expect(kinds(items)).toEqual(['run', 'ask-agent', 'follow-up', 'move', 'delete'])
    expect(find(items, 'follow-up')).toMatchObject({
      label: 'Follow-up task',
      icon: 'git-branch-plus',
      disabled: false
    })
  })

  it('offers the follow-up on a card whose task is done', () => {
    // The case the row was asked for: a task is closed and clarifications
    // arrive. Nothing about it is conditional on the status.
    const items = taskMenuItems({ ...base, bdStatus: 'closed', runnable: false })
    expect(find(items, 'follow-up').disabled).toBe(false)
  })

  it('greys the follow-up while a write is in flight', () => {
    const items = taskMenuItems({ ...base, busy: true })
    expect(find(items, 'follow-up').disabled).toBe(true)
  })

  it('marks the status the issue holds and refuses to write it again', () => {
    const children = find(taskMenuItems({ ...base, bdStatus: 'pinned' }), 'move').children
    expect(children.map((c) => c.value)).toEqual(['open', 'pinned', 'closed'])
    expect(children[1]).toMatchObject({ kind: 'status', icon: 'check', disabled: true })
    expect(children[0].icon).toBeUndefined()
  })

  it('marks nothing when the issue holds a status outside the three, and offers it', () => {
    const children = find(taskMenuItems({ ...base, bdStatus: 'in_progress' }), 'move').children
    expect(children.map((c) => c.value)).toEqual(['open', 'pinned', 'closed', 'in_progress'])
    expect(children.at(-1)).toMatchObject({ disabled: true, icon: 'check' })
  })

  it('marks Ready on a card the board computed as blocked', () => {
    // Blocked is a column, not a status: bd holds such an issue at `open`, and
    // the menu has to check Ready rather than nothing.
    const children = find(taskMenuItems({ ...base, bdStatus: 'open' }), 'move').children
    expect(children[0]).toMatchObject({ value: 'open', disabled: true, icon: 'check' })
  })

  it('checks nothing at all on a card that was given no status', () => {
    const children = find(taskMenuItems({ ...base, bdStatus: '' }), 'move').children
    expect(children.map((c) => c.value)).toEqual(['open', 'pinned', 'closed'])
    expect(children.every((c) => c.icon === undefined && !c.disabled)).toBe(true)
  })

  it('greys every row while a write is in flight', () => {
    const items = taskMenuItems({ ...base, busy: true })
    for (const kind of ['run', 'ask-agent', 'follow-up', 'move', 'delete']) {
      expect(find(items, kind).disabled).toBe(true)
    }
    expect(find(items, 'move').children.every((c) => c.disabled)).toBe(true)
  })

  it('offers answering the questions on a parked card, above the play', () => {
    // Above, because it is the thing to do with a parked task and the play is
    // the thing not to: the caller greys the run for exactly this card, so the
    // live row has to be the one a person reaches first.
    const items = taskMenuItems({ ...base, bdStatus: 'parked', runnable: false })
    expect(kinds(items)).toEqual(['resolve', 'run', 'ask-agent', 'follow-up', 'move', 'delete'])
    expect(find(items, 'resolve').disabled).toBeFalsy()
  })

  it('offers it on no other card at all', () => {
    // Absent rather than greyed: a fifth row that is dead on all but a handful
    // of cards is a row a person learns to read past.
    for (const bdStatus of ['open', 'in_progress', 'closed', 'pinned', 'deferred', '']) {
      expect(find(taskMenuItems({ ...base, bdStatus }), 'resolve')).toBeUndefined()
    }
  })

  it('greys it too while a write is in flight', () => {
    const items = taskMenuItems({ ...base, bdStatus: 'parked', busy: true })
    expect(find(items, 'resolve').disabled).toBe(true)
  })

  it('offers parked back as the status it holds, checked and refused', () => {
    // The submenu is where the warning is triggered from, so Ready has to be a
    // live option on a parked card — and parked itself an appended fourth.
    const children = find(taskMenuItems({ ...base, bdStatus: 'parked' }), 'move').children
    expect(children.map((c) => c.value)).toEqual(['open', 'pinned', 'closed', 'parked'])
    expect(children[0]).toMatchObject({ value: 'open', disabled: false })
    expect(children.at(-1)).toMatchObject({ label: 'Parked', disabled: true, icon: 'check' })
  })

  it('drops the run and the edit on a done card, and offers the fix instead', () => {
    /* Neither removed row is a loss. The run is refused on a closed issue
       anyway (`runnableTask` in DesktopApp), so it was a permanently greyed
       row; the edit is about the issue's own text, which is not what somebody
       wants from work that is finished and wrong. */
    const items = taskMenuItems({ ...base, bdStatus: 'closed' })
    expect(kinds(items)).toEqual(['fix', 'follow-up', 'move', 'delete'])
    expect(find(items, 'fix')).toMatchObject({
      label: 'Fix this',
      icon: 'wrench',
      disabled: false
    })
    expect(items.at(-2)).toEqual({ type: 'separator' })
  })

  it("reads the normalised status, not bd's own word for it", () => {
    /* A project whose done column is spelled some other way still normalises
       to `done`, and the menu it gets has to be the same one. */
    expect(kinds(taskMenuItems({ ...base, bdStatus: 'Done' }))).toEqual([
      'fix',
      'follow-up',
      'move',
      'delete'
    ])
  })

  it('offers the fix on a done card only', () => {
    // A row dead on all but a handful of cards is one a person reads past,
    // which is the trade the `resolve` row above it already makes.
    for (const bdStatus of ['open', 'in_progress', 'parked', 'blocked', '']) {
      expect(kinds(taskMenuItems({ ...base, bdStatus }))).not.toContain('fix')
    }
  })

  it('greys the fix while a write is in flight, like every other row', () => {
    const items = taskMenuItems({ ...base, bdStatus: 'closed', busy: true })
    expect(find(items, 'fix').disabled).toBe(true)
  })
})
