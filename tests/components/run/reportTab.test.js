import { describe, expect, it } from 'vitest'
import {
  REPORTS_DIR,
  REVIEWS_DIR,
  isDocumentPath,
  isReportPath,
  reportTabPath,
  reviewReportPath,
  reviewReportTabs
} from '../../../src/components/run/reportTab.js'

describe('isDocumentPath', () => {
  it('accepts an html file wherever it sits', () => {
    // The whole of the widening: the report of a branch review lands wherever
    // the reviewed project's own conventions send it, and a rule keyed on a
    // folder is wrong about every folder it has not heard of.
    expect(isDocumentPath('docs/reviews-pr/review-NXC-239-2026-09-07.html')).toBe(true)
    expect(isDocumentPath('src/index.html')).toBe(true)
    expect(isDocumentPath('page.html')).toBe(true)
  })

  it('accepts the reports and reviews it used to be the only rule about', () => {
    expect(isDocumentPath(`${REPORTS_DIR}2026-08-12-143155.html`)).toBe(true)
    expect(isDocumentPath(`${REVIEWS_DIR}2026-08-31-1345-feature-x.html`)).toBe(true)
  })

  it('accepts .htm, and either extension in any case', () => {
    expect(isDocumentPath('docs/page.htm')).toBe(true)
    expect(isDocumentPath('docs/PAGE.HTML')).toBe(true)
    expect(isDocumentPath('docs/Page.Htm')).toBe(true)
  })

  it('refuses every other extension, the review report\'s own Markdown included', () => {
    expect(isDocumentPath('.smetana/reviews/2026-08-31-1345-feature-x.md')).toBe(false)
    expect(isDocumentPath('src/main.js')).toBe(false)
    expect(isDocumentPath('README.md')).toBe(false)
  })

  it('refuses a name that merely contains the extension', () => {
    expect(isDocumentPath('docs/page.html.bak')).toBe(false)
    expect(isDocumentPath('docs/html')).toBe(false)
    expect(isDocumentPath('docs/.html-notes')).toBe(false)
  })

  it('refuses a diff tab id, which ends in the very path it is about', () => {
    // The rule decides which branch the centre draws, and `project.activeTab` is
    // not always a path: `diffId` builds `\u0000diff:<repo>\u0000<path>`. Read on
    // the extension alone this answers true, the document branch is tried before
    // the diff branch, and a synthetic id has no buffer — so a click on a
    // changed `.html` in the Git panel drew an empty sandbox instead of the
    // diff, with no error and no way back.
    expect(isDocumentPath('\u0000diff:/Users/x/repo\u0000src/index.html')).toBe(false)
    expect(isDocumentPath('\u0000diff:/Users/x/repo\u0000docs/page.htm')).toBe(false)
  })

  it('refuses a terminal tab id by the same clause', () => {
    // This one does not end in a path and would have fallen through anyway. The
    // rule worth writing is "this is not a path", not "not one of the two kinds
    // of id that exist today".
    expect(isDocumentPath('\u0000term:3')).toBe(false)
  })

  it('refuses the zero byte wherever it sits, which is what makes that true', () => {
    expect(isDocumentPath('docs/\u0000/page.html')).toBe(false)
    expect(isDocumentPath('\u0000page.html')).toBe(false)
  })

  it('refuses the pinned tabs and anything that is not a string', () => {
    // Where the value comes from: project.activeTab is null before a project is
    // open, and one of these two the rest of the time.
    expect(isDocumentPath('kanban')).toBe(false)
    expect(isDocumentPath('terminal')).toBe(false)
    expect(isDocumentPath(null)).toBe(false)
    expect(isDocumentPath(undefined)).toBe(false)
    expect(isDocumentPath(42)).toBe(false)
  })
})

describe('isReportPath', () => {
  it('accepts a report the run wrote', () => {
    expect(isReportPath('.smetana/reports/2026-08-12-143155.html')).toBe(true)
  })

  it('accepts the disambiguated name two runs in one second produce', () => {
    expect(isReportPath('.smetana/reports/2026-08-12-143155-2.html')).toBe(true)
  })

  it('accepts a review the agent wrote, which is the second folder', () => {
    expect(isReportPath('.smetana/reviews/2026-08-31-1345-feature-x.html')).toBe(true)
  })

  it('refuses an ordinary file, however it is named', () => {
    expect(isReportPath('src/index.html')).toBe(false)
    expect(isReportPath('reports/2026-08-12-143155.html')).toBe(false)
    expect(isReportPath('reviews/2026-08-31-1345-feature-x.html')).toBe(false)
  })

  it('refuses a folder whose name merely starts the same way', () => {
    // What the trailing slash on REPORTS_DIR is for, and the trap a tidy-up of
    // the constant falls straight into.
    expect(isReportPath('.smetana/reports-old/2026-08-12-143155.html')).toBe(false)
    expect(isReportPath('.smetana/reviews-old/2026-08-31-1345-feature-x.html')).toBe(false)
  })

  it('refuses anything else in the same folder, so only documents open as one', () => {
    expect(isReportPath('.smetana/reports/notes.txt')).toBe(false)
  })

  it('refuses the markdown a review writes beside its document', () => {
    // The whole reason the extension is tested rather than the folder alone: a
    // review leaves two files at one path and only one of them is a page.
    expect(isReportPath('.smetana/reviews/notes.md')).toBe(false)
    expect(isReportPath('.smetana/reviews/2026-08-31-1345-feature-x.md')).toBe(false)
  })

  it('refuses a path climbing out of the folder', () => {
    expect(isReportPath('.smetana/reports/../../etc/passwd.html')).toBe(false)
    expect(isReportPath('.smetana/reviews/../../etc/passwd.html')).toBe(false)
  })

  it('refuses a document in a folder under either of them', () => {
    expect(isReportPath('.smetana/reports/old/2026-08-12-143155.html')).toBe(false)
    expect(isReportPath('.smetana/reviews/nested/one.html')).toBe(false)
  })

  it('refuses a folder itself, which names no document', () => {
    expect(isReportPath(REPORTS_DIR)).toBe(false)
    expect(isReportPath(REVIEWS_DIR)).toBe(false)
  })

  it('says no to nothing at all rather than throwing', () => {
    expect(isReportPath('')).toBe(false)
    expect(isReportPath(null)).toBe(false)
    expect(isReportPath(undefined)).toBe(false)
  })

  it('says no to the two pinned tabs, which are names and not paths', () => {
    expect(isReportPath('terminal')).toBe(false)
    expect(isReportPath('kanban')).toBe(false)
  })
})

describe('reportTabPath', () => {
  const ROOT = '/Users/you/Projects/smetana'
  const REPORT = `${ROOT}/.smetana/reports/2026-08-12-143155.html`

  it('takes the project off the front of what the worker wrote', () => {
    expect(reportTabPath(REPORT, ROOT)).toBe('.smetana/reports/2026-08-12-143155.html')
  })

  it('does not mind a root with a trailing separator', () => {
    expect(reportTabPath(REPORT, `${ROOT}/`)).toBe('.smetana/reports/2026-08-12-143155.html')
  })

  it('reads a Windows path, where the two vocabularies use different separators', () => {
    const root = 'C:\\Users\\you\\smetana'
    const report = 'C:\\Users\\you\\smetana\\.smetana\\reports\\2026-08-12-143155.html'
    expect(reportTabPath(report, root)).toBe('.smetana/reports/2026-08-12-143155.html')
  })

  it('refuses a document belonging to another project rather than guessing', () => {
    expect(reportTabPath(REPORT, '/Users/you/Projects/other')).toBe(null)
    // The trailing separator again, from the other side: a sibling folder whose
    // name merely starts with this one's is not inside it.
    expect(reportTabPath('/Users/you/Projects/smetana-old/.smetana/reports/a.html', ROOT)).toBe(null)
  })

  it('refuses anything under the project that is not a report', () => {
    expect(reportTabPath(`${ROOT}/src/index.html`, ROOT)).toBe(null)
    expect(reportTabPath(`${ROOT}/.smetana/reports/notes.txt`, ROOT)).toBe(null)
  })

  it('says no to nothing at all rather than throwing', () => {
    expect(reportTabPath(null, ROOT)).toBe(null)
    expect(reportTabPath(REPORT, null)).toBe(null)
    expect(reportTabPath(REPORT, '')).toBe(null)
  })
})

describe('reviewReportPath', () => {
  // The shape `workOf` builds and `SessionWork::ReviewBranch` serialises: the
  // tag is `kind`, the payload is the path the app composed, and the extension
  // is not in it because the agent writes two files at that path.
  const review = (over = {}) => ({
    id: 7,
    state: 'exited',
    work: { kind: 'reviewBranch', report: '.smetana/reviews/2026-08-31-1345-feature-x' },
    ...over
  })

  it('puts the extension back on the path the app composed', () => {
    expect(reviewReportPath(review())).toBe('.smetana/reviews/2026-08-31-1345-feature-x.html')
  })

  it('says nothing about a review still running, which has written nothing', () => {
    expect(reviewReportPath(review({ state: 'running' }))).toBe(null)
    expect(reviewReportPath(review({ state: 'starting' }))).toBe(null)
  })

  it('says nothing about a session doing something else', () => {
    expect(reviewReportPath(review({ work: { kind: 'shell' } }))).toBe(null)
    expect(reviewReportPath(review({ work: { kind: 'editTask', id: 'smetana-8av' } }))).toBe(null)
  })

  it('refuses a path the tab rule declines rather than opening it as source', () => {
    expect(reviewReportPath(review({ work: { kind: 'reviewBranch', report: 'notes' } }))).toBe(null)
    const climbing = { kind: 'reviewBranch', report: '.smetana/reviews/../../etc/passwd' }
    expect(reviewReportPath(review({ work: climbing }))).toBe(null)
  })

  it('says no to a shape it cannot read rather than throwing', () => {
    expect(reviewReportPath(null)).toBe(null)
    expect(reviewReportPath({})).toBe(null)
    expect(reviewReportPath(review({ work: { kind: 'reviewBranch' } }))).toBe(null)
    expect(reviewReportPath(review({ work: null }))).toBe(null)
  })
})

describe('reviewReportTabs', () => {
  const HERE = '/Users/you/Projects/smetana'
  const exited = (id, name, project = HERE) => ({
    id,
    project,
    state: 'exited',
    work: { kind: 'reviewBranch', report: `.smetana/reviews/${name}` }
  })

  it('carries the session id beside every document, so one opens once', () => {
    const sessions = [exited(3, 'a'), exited(4, 'b')]
    expect(reviewReportTabs(sessions, HERE)).toEqual([
      { id: 3, path: '.smetana/reviews/a.html' },
      { id: 4, path: '.smetana/reviews/b.html' }
    ])
  })

  it('leaves out everything else in the panel', () => {
    const sessions = [
      { id: 1, project: HERE, state: 'running', work: { kind: 'shell' } },
      {
        id: 2,
        project: HERE,
        state: 'running',
        work: { kind: 'reviewBranch', report: '.smetana/reviews/a' }
      },
      exited(3, 'b'),
      { id: 4, project: HERE, state: 'exited', work: { kind: 'bare' } }
    ]
    expect(reviewReportTabs(sessions, HERE)).toEqual([{ id: 3, path: '.smetana/reviews/b.html' }])
  })

  it('leaves out a review belonging to another project, so no tab is lost', () => {
    // The gap inside `moveTo`: the active project has already changed and the
    // session list has not. Answering here would open the document into the new
    // project's tab list, where `applySection` discards it, and the caller
    // would have marked the ending answered all the same.
    const session = exited(3, 'a', '/Users/you/Projects/other')
    expect(reviewReportTabs([session], HERE)).toEqual([])
    // And the same session once its own project is the one on screen.
    expect(reviewReportTabs([session], '/Users/you/Projects/other')).toEqual([
      { id: 3, path: '.smetana/reviews/a.html' }
    ])
  })

  it('says nothing when no project is named, since there is no list to open into', () => {
    expect(reviewReportTabs([exited(3, 'a')], null)).toEqual([])
    expect(reviewReportTabs([exited(3, 'a')], '')).toEqual([])
    expect(reviewReportTabs([exited(3, 'a')], undefined)).toEqual([])
  })

  it('says nothing about a panel that is not a list yet', () => {
    expect(reviewReportTabs([], HERE)).toEqual([])
    expect(reviewReportTabs(null, HERE)).toEqual([])
    expect(reviewReportTabs(undefined, HERE)).toEqual([])
  })
})
