import { describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

/* One repository, named after the project it was asked for, so a response
   landing under the wrong project is visible in the assertion. */
const repoIn = (project) => [{ name: '.', path: `${project}/.`, branch: 'main', detached: null }]

const cleanTree = { branch: 'main', detached: null, changes: [] }

/* Two repositories under one project — the workspace the discovery arm in
   `vcs/repos.rs` exists for, and the only shape in which one project's own
   repositories can race each other. */
const siblings = [
  { name: 'admin', path: '/p/admin', branch: 'main', detached: null },
  { name: 'backend', path: '/p/backend', branch: 'develop', detached: null }
]

/* A working tree naming the repository it belongs to, so a tree drawn under the
   wrong repository is visible in the assertion. */
const treeOf = (repo) => ({
  branch: 'main',
  detached: null,
  changes: [{ path: `${repo}/file.rs`, origPath: null, kind: 'modified', staged: false, unstaged: true }]
})

/* The same trick for the branches: a list naming the repository it came from,
   so one repository's branches offered under another's name fails an
   assertion rather than reading as a plausible list. */
const branchesOf = (repo) => [
  { name: `${repo}/current`, current: true },
  { name: `${repo}/other`, current: false }
]

describe('the git panel store', () => {
  /* The same guard git.js, terminals.js and runs.js carry. Two calls can be in
     flight with no ordering guarantee on which invoke resolves first, and
     without it the last response wins rather than the last call — one
     project's files listed under another project's name, and every button in
     the panel then aimed at the wrong repository. */
  it('a slow answer for a project already left is dropped', async () => {
    const { stores, ipc, nextTick } = await loadStores()
    let releaseOld
    const old = new Promise((resolve) => {
      releaseOld = () => resolve(repoIn('/old'))
    })
    ipc.on('vcs_repos', (args) => (args.project === '/old' ? old : repoIn('/new')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', branchesOf('/new'))

    const first = stores.vcs.loadRepos('/old')
    const second = stores.vcs.loadRepos('/new')
    await second
    releaseOld()
    await first
    await nextTick()

    expect(stores.vcs.vcsState.project).toBe('/new')
    expect(stores.vcs.vcsState.repos).toEqual(repoIn('/new'))
    expect(stores.vcs.vcsState.selected).toBe('/new/.')
  })

  /* The guard is on a pair — the project *and* the selected repository — and
     this is its second half. Clicking between two repositories of one
     workspace races two `vcs_status` calls with no ordering guarantee, and
     without the second half of the guard the slower answer would land under the
     repository the person has since picked: one repository's files listed under
     another repository's name, inside a single project, where the project half
     of the guard sees nothing wrong at all. */
  it('a slow status for a repository already left is dropped', async () => {
    const { stores, ipc, nextTick } = await loadStores()
    ipc.on('vcs_repos', siblings)
    /* The opening read answers at once for both, so the race below is the only
       one in the test: a deferred answer here would hang `loadRepos` itself on
       a call this case is not about. */
    ipc.on('vcs_status', (args) => treeOf(args.repo === '/p/admin' ? 'admin' : 'backend'))
    ipc.on('vcs_branches', (args) => branchesOf(args.repo === '/p/admin' ? 'admin' : 'backend'))
    await stores.vcs.loadRepos('/p')

    let releaseAdmin
    const admin = new Promise((resolve) => {
      releaseAdmin = () => resolve(treeOf('admin'))
    })
    ipc.on('vcs_status', (args) => (args.repo === '/p/admin' ? admin : treeOf('backend')))

    const first = stores.vcs.selectRepo('/p/admin')
    const second = stores.vcs.selectRepo('/p/backend')
    await second
    releaseAdmin()
    await first
    await nextTick()

    expect(stores.vcs.vcsState.selected).toBe('/p/backend')
    expect(stores.vcs.vcsState.tree).toEqual(treeOf('backend'))
  })

  /* The branches carry the same pair-guard as the working tree, and the cost of
     losing it is one step worse: a list belonging to the repository somebody
     has just left is a row that, pressed, checks a branch out in a repository
     they are not looking at. */
  it('a slow branch list for a repository already left is dropped', async () => {
    const { stores, ipc, nextTick } = await loadStores()
    ipc.on('vcs_repos', siblings)
    ipc.on('vcs_status', (args) => treeOf(args.repo === '/p/admin' ? 'admin' : 'backend'))
    ipc.on('vcs_branches', (args) => branchesOf(args.repo === '/p/admin' ? 'admin' : 'backend'))
    await stores.vcs.loadRepos('/p')

    let releaseAdmin
    const admin = new Promise((resolve) => {
      releaseAdmin = () => resolve(branchesOf('admin'))
    })
    ipc.on('vcs_branches', (args) => (args.repo === '/p/admin' ? admin : branchesOf('backend')))

    const first = stores.vcs.selectRepo('/p/admin')
    const second = stores.vcs.selectRepo('/p/backend')
    await second
    releaseAdmin()
    await first
    await nextTick()

    expect(stores.vcs.vcsState.selected).toBe('/p/backend')
    expect(stores.vcs.vcsState.branches).toEqual(branchesOf('backend'))
  })

  /* A repository whose HEAD moves when git is told to move it, so a checkout
     can be watched from every place that draws the branch: the repository row,
     the branch list's mark, and the scope bar one store over. */
  const switching = (ipc, at = 'main') => {
    let branch = at
    ipc.on('vcs_repos', () => [{ name: '.', path: '/p/.', branch, detached: null }])
    ipc.on('vcs_status', () => ({ branch, detached: null, changes: [] }))
    ipc.on('vcs_branches', () => [
      { name: 'main', current: branch === 'main' },
      { name: 'develop', current: branch === 'develop' }
    ])
    ipc.on('git_head', () => ({ branch, detached: null }))
    ipc.on('vcs_checkout', (args) => {
      branch = args.branch
      return null
    })
  }

  /* The scope bar's branch is `git.js`'s and is refreshed by window focus and
     by switching project — neither of which somebody switching branches in this
     panel ever reaches. So the write refreshes it itself, and the bar naming
     the branch a person just left is what this pins. */
  it('a checkout that worked moves the panel and the scope bar with it', async () => {
    const { stores, ipc } = await loadStores()
    switching(ipc)
    await stores.vcs.loadRepos('/p')
    await stores.git.loadHead('/p')
    expect(stores.git.gitState.branch).toBe('main')

    await stores.vcs.checkout('develop')

    expect(ipc.calls('vcs_checkout')).toEqual([{ repo: '/p/.', branch: 'develop' }])
    expect(stores.vcs.vcsState.branches.find((b) => b.current)?.name).toBe('develop')
    // The row draws its own branch, so a status-only refresh would leave it
    // naming the branch that was just left.
    expect(stores.vcs.vcsState.repos[0].branch).toBe('develop')
    expect(stores.git.gitState.branch).toBe('develop')
    expect(stores.vcs.vcsState.checkoutError).toBe(null)
    expect(stores.vcs.vcsState.checkingOut).toBe(null)
  })

  /* Git's two refusals — a branch already checked out in another worktree, and
     local changes a checkout would overwrite — arrive as its own stderr and are
     drawn as they stand. Nothing else in the panel moves: the working tree is
     exactly where it was, and an error taking the changes list down with it
     would say a repository could not be read when it was read perfectly. */
  it('a refused checkout keeps git own words and changes nothing else', async () => {
    const { stores, ipc } = await loadStores()
    switching(ipc)
    await stores.vcs.loadRepos('/p')
    const before = stores.vcs.vcsState.tree
    /* git's own sentence, verbatim from a repository where the branch was held
       by a second worktree — which is exactly what a run's provisioning phase
       leaves behind. */
    ipc.fail('vcs_checkout', {
      kind: 'git',
      message: "fatal: 'develop' is already checked out at '/p/.worktrees/x'"
    })

    await stores.vcs.checkout('develop')

    expect(stores.vcs.vcsState.checkoutError).toEqual({
      kind: 'git',
      message: "fatal: 'develop' is already checked out at '/p/.worktrees/x'"
    })
    expect(stores.vcs.vcsState.error).toBe(null)
    expect(stores.vcs.vcsState.tree).toEqual(before)
    expect(stores.vcs.vcsState.branches.find((b) => b.current)?.name).toBe('main')
    expect(stores.vcs.vcsState.checkingOut).toBe(null)
  })

  /* One at a time. A second press while git is working would ask it to check a
     branch out in a tree it is already in, and the refusal for that comes from
     `index.lock` rather than from anything a person did wrong. */
  it('a checkout already in flight refuses a second one', async () => {
    const { stores, ipc } = await loadStores()
    switching(ipc)
    await stores.vcs.loadRepos('/p')
    let release
    ipc.on('vcs_checkout', () => new Promise((resolve) => (release = () => resolve(null))))

    const first = stores.vcs.checkout('develop')
    await stores.vcs.checkout('main')
    expect(ipc.calls('vcs_checkout')).toEqual([{ repo: '/p/.', branch: 'develop' }])
    release()
    await first
  })

  it('a failure leaves an error to draw, not a half-filled panel', async () => {
    const { stores, ipc, nextTick } = await loadStores()
    ipc.on('vcs_repos', repoIn('/p'))
    ipc.on('vcs_branches', [])
    ipc.fail('vcs_status', 'no git on this machine')

    await stores.vcs.loadRepos('/p')
    await stores.vcs.selectRepo('/p/.')
    await nextTick()

    expect(stores.vcs.vcsState.tree).toBe(null)
    expect(stores.vcs.vcsState.error).toBeTruthy()
  })

  /* A stored value is a hint, never the truth — the rule columnOrder.js keeps
     for a status bd no longer has. A repository dropped from the project since
     the last visit must not leave the panel pointed at a folder that is gone,
     and the substitution is silent because nothing a person did today caused
     it. */
  it('a remembered repository that is no longer there is replaced by the first', async () => {
    const { stores, ipc } = await loadStores()
    stores.settings.settings.project.selectedRepo = '/p/gone'
    ipc.on('vcs_repos', repoIn('/p'))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', branchesOf('/p'))

    await stores.vcs.loadRepos('/p')

    expect(stores.vcs.vcsState.selected).toBe('/p/.')
    expect(stores.settings.settings.project.selectedRepo).toBe('/p/.')
    expect(stores.vcs.vcsState.error).toBe(null)
  })
})
