import { describe, expect, it } from 'vitest'
import { projectMenuItems } from '../../../src/components/shell/projectMenu.js'

const base = { active: true, configured: true, configBroken: false, canAddAgent: true }
const kinds = (items) => items.filter((i) => !i.type).map((i) => i.kind)
const caption = (items) => items.find((i) => i.type === 'label')?.label
const find = (items, kind) => items.find((i) => i.kind === kind)

describe('projectMenuItems', () => {
  it('offers the row four actions, with removal last and behind a separator', () => {
    const items = projectMenuItems(base)
    expect(kinds(items)).toEqual(['setup', 'settings', 'add-agent', 'remove'])
    expect(items.at(-2)).toEqual({ type: 'separator' })
    // The caption's reach: removal is live below the separator, so it is not
    // part of the group the caption refuses.
    expect(kinds(projectMenuItems({ ...base, active: false }))).toEqual([
      'setup',
      'settings',
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

describe('the project settings item', () => {
  const settings = (items) => items.find((item) => item.kind === 'settings')
  const captions = (items) => items.filter((item) => item.type === 'label').map((i) => i.label)

  it('is live for an active project whose configuration loads', () => {
    const items = projectMenuItems({ ...base, configured: true, configBroken: false })
    expect(settings(items).disabled).toBe(false)
    expect(captions(items)).toEqual([])
  })

  it('is dead on a row that is not the active project, under the existing caption', () => {
    // One caption, not two: `ELSEWHERE` already greys the whole group, and a
    // second sentence under it would claim to know something about a project
    // nobody measured.
    const items = projectMenuItems({ ...base, active: false })
    expect(settings(items).disabled).toBe(true)
    expect(captions(items)).toEqual(['Switch to this project first'])
  })

  it('is live on an active project that has no configuration yet', () => {
    // The window is not only about `.smetana/project.toml` any more: it carries
    // this machine's caveman level for the project, which is kept in
    // `settings.json`. A project with no file still has that to change, so the
    // item opens and the window says why there are no fields under it.
    const items = projectMenuItems({ ...base, configured: false, configBroken: false })
    expect(settings(items).disabled).toBe(false)
    expect(captions(items)).toEqual([])
    // And the route to a file is still the live item above it.
    expect(find(items, 'setup').disabled).toBe(false)
  })

  it('is live on an active project whose configuration will not parse', () => {
    const items = projectMenuItems({ ...base, configured: false, configBroken: true })
    expect(settings(items).disabled).toBe(false)
    expect(captions(items)).toEqual([])
    // A form cannot help with the file itself — there are no parsed values to
    // draw — and running the setup over the damaged file is what can, which is
    // why that item reads "Set up again" and stays live beside this one.
    expect(find(items, 'setup').label).toBe('Set up again')
    expect(find(items, 'setup').disabled).toBe(false)
  })

  it('captions nothing over an active row, whatever the project config is in', () => {
    // The two refusals this item used to carry are gone with the greying: a
    // caption here would now refuse a window that has something to offer.
    for (const config of [
      { configured: true, configBroken: false },
      { configured: false, configBroken: false },
      { configured: false, configBroken: true }
    ]) {
      expect(captions(projectMenuItems({ ...base, ...config }))).toEqual([])
    }
  })

  it("does not share the setup item's glyph", () => {
    const items = projectMenuItems(base)
    expect(settings(items).icon).not.toBe(find(items, 'setup').icon)
  })
})
