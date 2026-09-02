import { describe, expect, it } from 'vitest'
import {
  CONFIG_FILE,
  DEFAULTS_FALLBACK,
  NO_BRANCH,
  branchOptions,
  configNotice,
  draftFrom,
  isDirty,
  offersDefaults,
  validateDraft
} from '../../../src/components/run/projectDefaults.js'

describe('draftFrom', () => {
  it('falls back to the same values the Rust side defaults to', () => {
    // `Defaults::default()` in `src-tauri/src/runs/config.rs`: no branch, 2, 3,
    // 5. Two copies of one fact, so this is the test that keeps them one.
    expect(draftFrom(null)).toEqual({
      target_branch: null,
      min_priority: 2,
      max_parallel_tasks: 3,
      review_passes: 5
    })
    expect(draftFrom({ project: { repos: ['.'] } })).toEqual(DEFAULTS_FALLBACK)
  })

  it('reads the four keys a file does carry', () => {
    const config = {
      project: { repos: ['.'] },
      defaults: {
        target_branch: 'staging',
        min_priority: 1,
        max_parallel_tasks: 4,
        review_passes: 3
      }
    }
    expect(draftFrom(config)).toEqual({
      target_branch: 'staging',
      min_priority: 1,
      max_parallel_tasks: 4,
      review_passes: 3
    })
  })

  it('fills in only the keys a half-written section is missing', () => {
    const config = { project: { repos: ['.'] }, defaults: { max_parallel_tasks: 8 } }
    expect(draftFrom(config)).toEqual({ ...DEFAULTS_FALLBACK, max_parallel_tasks: 8 })
  })

  it('hands back a draft of its own rather than the frozen fall-back', () => {
    // A form writes into what it is given, and the fall-back is one object for
    // the life of the module.
    const draft = draftFrom(null)
    expect(draft).not.toBe(DEFAULTS_FALLBACK)
    expect(Object.isFrozen(draft)).toBe(false)
  })
})

describe('branchOptions', () => {
  it('offers no default first, as an empty value', () => {
    const options = branchOptions([{ name: 'main' }, { name: 'develop' }], null)
    expect(options[0]).toEqual({ value: '', label: NO_BRANCH })
    expect(options[0].label).toBe('No default — use the current branch')
    expect(options.map((option) => option.value)).toEqual(['', 'main', 'develop'])
  })

  it('keeps a chosen branch the list no longer has, so opening the dialog changes nothing', () => {
    const options = branchOptions([{ name: 'main' }], 'a-branch-somebody-deleted')
    expect(options.map((option) => option.value)).toContain('a-branch-somebody-deleted')
  })

  it('does not repeat a chosen branch the list already has', () => {
    const options = branchOptions([{ name: 'main' }], 'main')
    expect(options.filter((option) => option.value === 'main')).toHaveLength(1)
  })

  it('still offers the one option when there is no list at all', () => {
    // The dialog can open before `target_branches` has answered.
    expect(branchOptions(undefined, null)).toEqual([{ value: '', label: NO_BRANCH }])
  })
})

describe('validateDraft', () => {
  it('accepts the fallback', () => {
    expect(validateDraft(DEFAULTS_FALLBACK)).toEqual({})
  })

  it('accepts both ends of every range', () => {
    expect(
      validateDraft({ target_branch: null, min_priority: 0, max_parallel_tasks: 1, review_passes: 1 })
    ).toEqual({})
    expect(
      validateDraft({
        target_branch: null,
        min_priority: 4,
        max_parallel_tasks: 16,
        review_passes: 10
      })
    ).toEqual({})
  })

  it('names the field and its range when a number is out of bounds', () => {
    const bad = { ...DEFAULTS_FALLBACK, max_parallel_tasks: 0, review_passes: 11, min_priority: 5 }
    const errors = validateDraft(bad)
    expect(errors.max_parallel_tasks).toBe('Between 1 and 16.')
    expect(errors.review_passes).toBe('Between 1 and 10.')
    expect(errors.min_priority).toBe('Between 0 and 4.')
  })

  it('rejects a number that is not whole', () => {
    expect(validateDraft({ ...DEFAULTS_FALLBACK, review_passes: 2.5 }).review_passes).toBe(
      'Between 1 and 10.'
    )
  })

  it('rejects what a number field hands back when it is empty', () => {
    // An `<input type="number">` emits '' rather than a number, and the other
    // side of the wire takes a `u8`.
    expect(validateDraft({ ...DEFAULTS_FALLBACK, max_parallel_tasks: '' }).max_parallel_tasks).toBe(
      'Between 1 and 16.'
    )
  })

  it('says nothing about an absent target branch, which is a legitimate value', () => {
    expect(validateDraft({ ...DEFAULTS_FALLBACK, target_branch: null })).toEqual({})
    expect(validateDraft({ ...DEFAULTS_FALLBACK, target_branch: '' })).toEqual({})
  })
})

describe('isDirty', () => {
  it('is false for a draft nobody has touched', () => {
    expect(isDirty({ ...DEFAULTS_FALLBACK }, { ...DEFAULTS_FALLBACK })).toBe(false)
  })

  it('is true for one changed field', () => {
    expect(isDirty({ ...DEFAULTS_FALLBACK, review_passes: 6 }, DEFAULTS_FALLBACK)).toBe(true)
    expect(isDirty({ ...DEFAULTS_FALLBACK, target_branch: 'main' }, DEFAULTS_FALLBACK)).toBe(true)
  })

  it('treats an empty branch and no branch as the same thing', () => {
    // `Select` has no way to hand back `null`, so choosing "No default" over a
    // file that never had the key is not a change.
    expect(isDirty({ ...DEFAULTS_FALLBACK, target_branch: '' }, DEFAULTS_FALLBACK)).toBe(false)
    expect(isDirty(DEFAULTS_FALLBACK, { ...DEFAULTS_FALLBACK, target_branch: '' })).toBe(false)
  })

  it('is true for a branch cleared away', () => {
    const stored = { ...DEFAULTS_FALLBACK, target_branch: 'staging' }
    expect(isDirty({ ...stored, target_branch: '' }, stored)).toBe(true)
  })
})

describe('what the window draws instead of the fields', () => {
  it('offers the form for a parsed file and for nothing else', () => {
    expect(offersDefaults('ok')).toBe(true)
    expect(offersDefaults('missing')).toBe(false)
    expect(offersDefaults('broken')).toBe(false)
    // A state this front end has never heard of is an ordinary outcome: no
    // fields, rather than four fall-backs presented as the project's values.
    expect(offersDefaults('rearranged')).toBe(false)
    expect(offersDefaults(undefined)).toBe(false)
  })

  it('says nothing where the fields are there to speak for themselves', () => {
    expect(configNotice('ok')).toBeNull()
  })

  it('names which of the two states a project with no form is in', () => {
    expect(configNotice('missing').lead).toBe('This project has no')
    expect(configNotice('missing').tail).toMatch(/nothing here to fill in/)
    expect(configNotice('broken').lead).toBe("This project's")
    expect(configNotice('broken').tail).toMatch(/will not parse/)
    // The two are genuinely different sentences: "set it up" against "set it up
    // again", which is what the menu item beside this window offers in each.
    expect(configNotice('missing')).not.toEqual(configNotice('broken'))
  })

  it('falls back to a sentence claiming nothing for a state it has not heard of', () => {
    const notice = configNotice('rearranged')
    expect(notice).toBeTruthy()
    expect(notice).not.toEqual(configNotice('missing'))
    expect(notice).not.toEqual(configNotice('broken'))
  })

  it('keeps the path out of both halves, since the window sets it in mono', () => {
    for (const state of ['missing', 'broken', 'rearranged']) {
      const { lead, tail } = configNotice(state)
      expect(lead).not.toContain(CONFIG_FILE)
      expect(tail).not.toContain(CONFIG_FILE)
    }
    expect(CONFIG_FILE).toBe('.smetana/project.toml')
  })
})
