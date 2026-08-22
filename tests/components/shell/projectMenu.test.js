import { describe, expect, it } from 'vitest'
import { projectMenuItems } from '../../../src/components/shell/projectMenu.js'

const base = { active: true, configured: true, configBroken: false, canAddAgent: true }
const kinds = (items) => items.filter((i) => !i.type).map((i) => i.kind)
const caption = (items) => items.find((i) => i.type === 'label')?.label
const find = (items, kind) => items.find((i) => i.kind === kind)

describe('projectMenuItems', () => {
  it('offers the row three actions, with removal last and behind a separator', () => {
    const items = projectMenuItems(base)
    expect(kinds(items)).toEqual(['setup', 'add-agent', 'remove'])
    expect(items.at(-2)).toEqual({ type: 'separator' })
    // The caption's reach: removal is live below the separator, so it is not
    // part of the group the caption refuses.
    expect(kinds(projectMenuItems({ ...base, active: false }))).toEqual([
      'setup',
      'add-agent',
      'remove'
    ])
    expect(find(items, 'remove').tone).toBe('danger')
  })

  it('reads "Set up again" when a configuration is there', () => {
    expect(find(projectMenuItems(base), 'setup')).toMatchObject({
      label: 'Set up again',
      existing: true,
      disabled: false
    })
  })

  it('reads "Set up" when the project has no configuration yet', () => {
    const items = projectMenuItems({ ...base, configured: false })
    expect(find(items, 'setup')).toMatchObject({ label: 'Set up', existing: false })
  })

  it('offers the setup over a configuration that cannot be parsed', () => {
    // The row draws no gear for a damaged file and the run dialog no longer
    // carries a button, so this item is the whole route back to the setup.
    const items = projectMenuItems({ ...base, configured: false, configBroken: true })
    expect(find(items, 'setup')).toMatchObject({
      label: 'Set up again',
      existing: true,
      disabled: false
    })
  })

  it('greys the two project-scoped verbs elsewhere, and says why once above them', () => {
    // One fact refuses both, so it is a caption and not a suffix per label: the
    // suffixed version ran past the panel's ceiling and clipped mid-word.
    const items = projectMenuItems({ ...base, active: false })
    expect(items[0]).toEqual({ type: 'label', label: 'Switch to this project first' })
    expect(find(items, 'setup')).toMatchObject({ label: 'Set up', disabled: true })
    expect(find(items, 'add-agent')).toMatchObject({ label: 'New agent', disabled: true })
  })

  it('captions nothing on the project the window is pointed at', () => {
    expect(caption(projectMenuItems(base))).toBeUndefined()
  })

  it('claims nothing about the configuration of a project it was not measured for', () => {
    // `configured` and `configBroken` are measured for the active project only,
    // so on any other row they are not this row's to draw.
    const items = projectMenuItems({ ...base, active: false, configured: true })
    expect(find(items, 'setup')).toMatchObject({ label: 'Set up' })
    expect(find(items, 'setup').existing).toBe(false)
  })

  it('keeps removal live on a project the window is not pointed at', () => {
    const items = projectMenuItems({ ...base, active: false })
    expect(find(items, 'remove').disabled).toBeFalsy()
  })

  it('offers a new agent whichever side panel is open', () => {
    // The row's plus appears only over the agents panel; the menu keeps the
    // active half of that rule and drops the panel half, because starting a
    // session switches to that panel anyway.
    const items = projectMenuItems({ ...base, canAddAgent: false })
    expect(find(items, 'add-agent').disabled).toBe(false)
  })
})
