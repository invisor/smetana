import { describe, expect, it } from 'vitest'
import { workOf } from '../../../src/components/agent/sessionWork.js'

/* The mirror of `Intent::work` in `src-tauri/src/agents/mod.rs`, field for
   field: the placeholder row a start draws for a second and the session's own
   row a moment later must say the same thing. */
describe('what a start will call its work', () => {
  it('keeps a filing draft minus its images and its stages', () => {
    expect(
      workOf({
        kind: 'newTask',
        brainstorm: 'off',
        draft: { text: 'Ring once', issue_type: 'bug', priority: 1, parent: 'x-1', images: ['/a.png'] }
      })
    ).toEqual({ kind: 'newTask', text: 'Ring once', issueType: 'bug', priority: 1, parent: 'x-1' })
  })

  it('names the issue for the three intents about one', () => {
    for (const kind of ['editTask', 'resolveTask', 'fixTask']) {
      expect(workOf({ kind, id: 'x-1', title: 'T' })).toEqual({ kind, id: 'x-1' })
    }
  })

  it('keeps the repository and the incoming branch of a conflict', () => {
    expect(workOf({ kind: 'resolveConflict', repo: '/p/app', op: 'merge', theirs: 'feat', files: ['a'] }))
      .toEqual({ kind: 'resolveConflict', repo: '/p/app', theirs: 'feat' })
  })

  it('keeps only the title of a resume and only the report of a review', () => {
    expect(workOf({ kind: 'resumeSession', id: 'u', cwd: '/p', title: 'T', fork: true }))
      .toEqual({ kind: 'resumeSession', title: 'T' })
    expect(workOf({ kind: 'reviewBranch', pairs: [], report: '/p/r.html' }))
      .toEqual({ kind: 'reviewBranch', report: '/p/r.html' })
  })

  it('reduces everything else to its kind, and nothing to bare', () => {
    expect(workOf({ kind: 'setup' })).toEqual({ kind: 'setup' })
    expect(workOf({ kind: 'repairTracker', dir: '/p' })).toEqual({ kind: 'repairTracker' })
    expect(workOf(undefined)).toEqual({ kind: 'bare' })
  })
})
