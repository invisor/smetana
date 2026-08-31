import { describe, expect, it } from 'vitest'
import { conflictRecord } from '../../../src/components/git/conflictRecord.js'

describe('conflictRecord', () => {
  it('is nothing when the tree has no conflicted paths', () => {
    const record = conflictRecord({
      repo: '/w/api',
      files: [],
      progress: { op: 'merge', ours: 'main', theirs: 'feature' },
      previous: null
    })
    expect(record).toBeNull()
  })

  it('is nothing when git is part-way through neither operation', () => {
    // A cherry-pick, a revert, a stash pop: unmerged paths with neither of the
    // dialog's two doors true.
    const record = conflictRecord({
      repo: '/w/api',
      files: ['src/one.js'],
      progress: null,
      previous: null
    })
    expect(record).toBeNull()
  })

  it('carries what the probe answered', () => {
    const record = conflictRecord({
      repo: '/w/api',
      files: ['src/one.js'],
      progress: { op: 'merge', ours: 'main', theirs: 'feature' },
      previous: null
    })
    expect(record).toEqual({
      repo: '/w/api',
      op: 'merge',
      ours: 'main',
      theirs: 'feature',
      files: ['src/one.js']
    })
  })

  it('keeps a branch the probe could not name', () => {
    // The press that started the rebase knew what it was onto; the probe never
    // can. A name already held must survive a refresh.
    const record = conflictRecord({
      repo: '/w/api',
      files: ['src/one.js'],
      progress: { op: 'rebase', ours: 'feature', theirs: null },
      previous: { repo: '/w/api', op: 'rebase', ours: 'feature', theirs: 'main', files: [] }
    })
    expect(record.theirs).toBe('main')
  })

  it('prefers the name the probe answered over the one held', () => {
    const record = conflictRecord({
      repo: '/w/api',
      files: ['src/one.js'],
      progress: { op: 'merge', ours: 'main', theirs: 'feature' },
      previous: { repo: '/w/api', op: 'merge', ours: 'main', theirs: 'stale', files: [] }
    })
    expect(record.theirs).toBe('feature')
  })

  it('borrows nothing from another repository', () => {
    const record = conflictRecord({
      repo: '/w/api',
      files: ['src/one.js'],
      progress: { op: 'rebase', ours: 'feature', theirs: null },
      previous: { repo: '/w/web', op: 'rebase', ours: 'feature', theirs: 'main', files: [] }
    })
    expect(record.theirs).toBeNull()
  })

  it('borrows nothing from the other operation', () => {
    const record = conflictRecord({
      repo: '/w/api',
      files: ['src/one.js'],
      progress: { op: 'rebase', ours: 'feature', theirs: null },
      previous: { repo: '/w/api', op: 'merge', ours: 'main', theirs: 'feature', files: [] }
    })
    expect(record.theirs).toBeNull()
    expect(record.ours).toBe('feature')
  })

  it('always reports the paths the tree shows now', () => {
    const record = conflictRecord({
      repo: '/w/api',
      files: ['src/one.js'],
      progress: { op: 'merge', ours: 'main', theirs: 'feature' },
      previous: { repo: '/w/api', op: 'merge', ours: 'main', theirs: 'feature', files: ['gone.js'] }
    })
    expect(record.files).toEqual(['src/one.js'])
  })
})
