import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'

/* One row as `run_reports` hands it over, with only the fields a test cares
   about spelled out per case — the wire shape `reports.rs`'s `ReportEntry`
   carries, flattened. */
const entry = (file, over = {}) => ({
  path: `/p/.smetana/reports/${file}`,
  file,
  stamp: '2026-09-17T14:32:05',
  title: 'Task report',
  scope: 'the queue',
  finished: '2026-09-17 14:32',
  closed: 1,
  parked: 0,
  batches: 1,
  total: '12m',
  seconds: 720,
  summary: 'Closed the login bug and added a regression test for it. Nothing else needed touching.',
  ...over
})

describe('the run reports a project has on disk', () => {
  it('the list is read and lands in the store', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('run_reports', [entry('2026-09-17-143205.html')])

    await stores.reports.loadReports('/p')

    expect(stores.reports.reportsState.rows).toEqual([entry('2026-09-17-143205.html')])
    expect(stores.reports.reportsState.project).toBe('/p')
    expect(ipc.calls('run_reports')).toEqual([{ project: '/p' }])
  })

  it('a project with no reports gets an empty list, not a failure', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('run_reports', [])

    await stores.reports.loadReports('/p')

    expect(stores.reports.reportsState.rows).toEqual([])
    expect(stores.reports.reportsState.loading).toBe(false)
  })

  /* The rule the tab rests on: another project's reports must never be on
     screen under the name of the one somebody is looking at. */
  it('another project\'s reports go the moment this one is asked about', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('run_reports', [entry('2026-09-17-143205.html')])
    await stores.reports.loadReports('/p')

    ipc.on('run_reports', () => new Promise(() => {}))
    stores.reports.loadReports('/q')

    expect(stores.reports.reportsState.rows).toEqual([])
    expect(stores.reports.reportsState.project).toBe('/q')
  })

  /* The other half: re-opening the tab on the same project reads again
     without blinking the list empty and back. */
  it('this project\'s reports stay on screen while they are read again', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('run_reports', [entry('2026-09-17-143205.html')])
    await stores.reports.loadReports('/p')

    let answer
    ipc.on('run_reports', () => new Promise((resolve) => (answer = resolve)))
    const again = stores.reports.loadReports('/p')

    expect(stores.reports.reportsState.rows.map((row) => row.file)).toEqual([
      '2026-09-17-143205.html'
    ])
    answer([entry('2026-09-17-143206.html')])
    await again
    expect(stores.reports.reportsState.rows.map((row) => row.file)).toEqual([
      '2026-09-17-143206.html'
    ])
  })

  it('with no project there is nothing to list and nothing to ask', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('run_reports', [entry('2026-09-17-143205.html')])
    await stores.reports.loadReports('/p')

    await stores.reports.loadReports(null)

    expect(stores.reports.reportsState.rows).toEqual([])
    expect(stores.reports.reportsState.loading).toBe(false)
    expect(ipc.calls('run_reports')).toEqual([{ project: '/p' }])
  })

  /* The command itself does not reject — a missing folder, an unreadable file
     and a name it does not recognise all come back as fewer rows. Getting
     here means the call failed, which leaves nothing to draw rather than
     somebody else's list, and a line for whoever reads the console. */
  it('a failed call leaves nothing rather than a list nobody confirmed', async () => {
    const { ipc, stores } = await loadStores()
    ipc.on('run_reports', [entry('2026-09-17-143205.html')])
    await stores.reports.loadReports('/p')

    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('run_reports', 'it broke')
    await stores.reports.loadReports('/p')

    expect(stores.reports.reportsState.rows).toEqual([])
    expect(stores.reports.reportsState.loading).toBe(false)
    expect(console.error).toHaveBeenCalled()
  })

  /* The same guard `sessions.js` and `git.js` carry: two reads can be in
     flight with no ordering guarantee on which invoke resolves first, so the
     last call wins rather than the last answer. */
  it('a stale answer does not overwrite the new project\'s reports', async () => {
    const { stores } = await loadStores()
    const pending = new Map()
    const { mockIPC } = await import('@tauri-apps/api/mocks')
    mockIPC((cmd, args) => new Promise((resolve) => pending.set(args.project, resolve)))

    const slow = stores.reports.loadReports('/old')
    const fast = stores.reports.loadReports('/new')

    pending.get('/new')([entry('new-one.html')])
    await fast
    pending.get('/old')([entry('old-one.html')])
    await slow

    expect(stores.reports.reportsState.rows.map((row) => row.file)).toEqual(['new-one.html'])
  })
})
