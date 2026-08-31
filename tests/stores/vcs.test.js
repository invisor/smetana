import { describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'
/* The very rule the button is drawn from, imported here so the two answers can
   be asserted to be one answer. Static rather than through `loadStores`: it is
   a pure module with no state to rebuild. */
import { pushAction } from '../../src/components/git/tracking.js'

/* One repository, named after the project it was asked for, so a response
   landing under the wrong project is visible in the assertion. */
const repoIn = (project) => [{ name: '.', path: `${project}/.`, branch: 'main', detached: null }]

/* `vcs_repos` answers with both halves at once — the repositories, and the
   names of the ones on disk `[project].repos` does not hold. Almost every test
   here is about the first half, so this wraps a list in the shape the command
   actually returns and leaves the second empty, which is what a project set up
   properly answers. */
const answer = (repos, unlisted = []) => ({ repos, unlisted })

const cleanTree = { branch: 'main', detached: null, changes: [] }

/* A tree git left mid-operation: unmerged paths and nothing else. The kind is
   `conflicted` — Rust's `ChangeKind::Conflicted` through serde — which is the
   one word `loadStatus` filters on to decide whether to ask git anything at
   all. */
const conflictedTree = (...paths) => ({
  branch: 'main',
  detached: null,
  changes: (paths.length ? paths : ['src/one.js']).map((path) => ({
    path,
    origPath: null,
    kind: 'conflicted',
    staged: false,
    unstaged: true
  }))
})

/* Bring the panel up over one repository, answering every command it reads on
   the way. `progress` is what `vcs_in_progress` replies, or `undefined` to
   leave that command unregistered — which is how a test asserts it was never
   asked, since the mock throws for a command nobody registered. */
const openPanel = async (stores, ipc, tree, progress) => {
  ipc.on('vcs_repos', (args) => answer(repoIn(args.project)))
  ipc.on('vcs_status', tree)
  ipc.on('vcs_branches', [])
  ipc.on('vcs_tracking', [])
  if (progress !== undefined) ipc.on('vcs_in_progress', progress)
  await stores.vcs.loadRepos('/p')
}

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
    ipc.on('vcs_repos', (args) => (args.project === '/old' ? old : answer(repoIn('/new'))))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', branchesOf('/new'))
    ipc.on('vcs_tracking', [])

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

  /* The second half of what `vcs_repos` answers: the folders on disk that
     `[project].repos` does not name. It arrives with the list because it is
     read off the same directory listing, and the panel is the only thing that
     ever says a word about them. */
  it('a repository the configuration does not name arrives beside the list', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p'), ['newrepo']))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])

    await stores.vcs.loadRepos('/p')

    expect(stores.vcs.vcsState.repos).toEqual(repoIn('/p'))
    expect(stores.vcs.vcsState.unlisted).toEqual(['newrepo'])
  })

  /* A sentence naming a folder is a statement about a directory that was read.
     A read that failed has not earned one, and leaving it standing would put
     the panel's one remark about the project's contents over a list nothing
     could be listed into. */
  it('a listing that failed leaves no folder named on screen', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p'), ['newrepo']))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    await stores.vcs.loadRepos('/p')
    expect(stores.vcs.vcsState.unlisted).toEqual(['newrepo'])

    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.on('vcs_repos', () => Promise.reject(new Error('the transport gave way')))
    await stores.vcs.refresh()

    expect(stores.vcs.vcsState.repos).toEqual([])
    expect(stores.vcs.vcsState.unlisted).toEqual([])
    expect(stores.vcs.vcsState.error).not.toBeNull()
  })

  /* It goes with `repos` and for its reason: it is a statement about the
     project being left, and a window with no project must not keep one. */
  it('a window left with no project keeps no folder named either', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p'), ['newrepo']))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    await stores.vcs.loadRepos('/p')
    expect(stores.vcs.vcsState.unlisted).toEqual(['newrepo'])

    await stores.vcs.loadRepos(null)

    expect(stores.vcs.vcsState.repos).toEqual([])
    expect(stores.vcs.vcsState.unlisted).toEqual([])
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
    ipc.on('vcs_repos', answer(siblings))
    /* The opening read answers at once for both, so the race below is the only
       one in the test: a deferred answer here would hang `loadRepos` itself on
       a call this case is not about. */
    ipc.on('vcs_status', (args) => treeOf(args.repo === '/p/admin' ? 'admin' : 'backend'))
    ipc.on('vcs_branches', (args) => branchesOf(args.repo === '/p/admin' ? 'admin' : 'backend'))
    ipc.on('vcs_tracking', [])
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
    ipc.on('vcs_repos', answer(siblings))
    ipc.on('vcs_status', (args) => treeOf(args.repo === '/p/admin' ? 'admin' : 'backend'))
    ipc.on('vcs_branches', (args) => branchesOf(args.repo === '/p/admin' ? 'admin' : 'backend'))
    ipc.on('vcs_tracking', [])
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

  /* The second branch list, which nothing on screen reads yet: what `origin` is
     known to have, for a caller that names its own repository rather than
     taking the one the panel is pointed at. It lands beside `branches` and not
     in it — a remote branch and a local one of the same name are two different
     things to check out. */
  it('the branches origin has arrive in a list of their own', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_remote_branches', ['develop', 'feature/one', 'main'])

    await stores.vcs.loadRemoteBranches('/p/admin')

    expect(stores.vcs.vcsState.remoteBranches).toEqual(['develop', 'feature/one', 'main'])
    /* The repository asked about is the one named in the call, and the names
       arrive without `origin/` on them: the prefix is the caller's to add back
       when it builds a ref for git. */
    expect(ipc.calls('vcs_remote_branches')).toEqual([{ repo: '/p/admin' }])
    expect(stores.vcs.vcsState.branches).toEqual([])
  })

  /* `vcs_remote_branches` refuses nothing, so a rejection is the call itself
     giving way — and what must not survive it is the list read before. Branches
     that may no longer be there, offered under a read that failed, is worse than
     an empty list, which is what a repository nobody has fetched into already
     looks like. */
  it('a remote branch list that failed leaves nothing standing in for one', async () => {
    const { stores, ipc } = await loadStores()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.on('vcs_remote_branches', ['main'])
    await stores.vcs.loadRemoteBranches('/p/admin')
    expect(stores.vcs.vcsState.remoteBranches).toEqual(['main'])

    ipc.fail('vcs_remote_branches', 'the transport gave way')
    await stores.vcs.loadRemoteBranches('/p/admin')

    expect(stores.vcs.vcsState.remoteBranches).toEqual([])
  })

  /* A repository whose HEAD moves when git is told to move it, so a checkout
     can be watched from every place that draws the branch: the repository row,
     the branch list's mark, and the scope bar one store over. */
  const switching = (ipc, at = 'main') => {
    let branch = at
    ipc.on('vcs_repos', () => answer([{ name: '.', path: '/p/.', branch, detached: null }]))
    ipc.on('vcs_status', () => ({ branch, detached: null, changes: [] }))
    ipc.on('vcs_branches', () => [
      { name: 'main', current: branch === 'main' },
      { name: 'develop', current: branch === 'develop' }
    ])
    ipc.on('vcs_tracking', [])
    ipc.on('git_head', () => ({ branch, detached: null }))
    ipc.on('vcs_checkout', (args) => {
      branch = args.branch
      return null
    })
  }

  /* What the repository becomes once a write has stopped on a conflict: git
     leaves the unmerged paths standing, and the refresh that follows the write
     reads them. A fixture answering a clean tree there would be a repository
     git never produces — and since `loadStatus` re-derives the record off the
     tree, it would take the record away as fast as the write put it there.

     `progress` is what the probe over that tree answers. A rebase's `theirs` is
     `null` on purpose: the branch it is going onto is readable nowhere a git
     process can see, which is the whole reason the press's own knowledge has to
     survive the refresh. */
  const stopsOn = (ipc, files, progress) => {
    ipc.on('vcs_status', () => conflictedTree(...files))
    ipc.on('vcs_in_progress', progress)
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
    expect(stores.vcs.vcsState.writeError).toBe(null)
    expect(stores.vcs.vcsState.busy).toBe(null)
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

    expect(stores.vcs.vcsState.writeError).toEqual({
      kind: 'git',
      op: 'checkout',
      message: "fatal: 'develop' is already checked out at '/p/.worktrees/x'"
    })
    expect(stores.vcs.vcsState.error).toBe(null)
    expect(stores.vcs.vcsState.tree).toEqual(before)
    expect(stores.vcs.vcsState.branches.find((b) => b.current)?.name).toBe('main')
    expect(stores.vcs.vcsState.busy).toBe(null)
  })

  /* A repository whose branch list grows when git is told to cut one, so the
     row appearing — and the mark either moving with it or staying put — can be
     watched from the store. */
  const cutting = (ipc, at = 'main') => {
    let branch = at
    const names = ['main', 'develop']
    ipc.on('vcs_repos', () => answer([{ name: '.', path: '/p/.', branch, detached: null }]))
    ipc.on('vcs_status', () => ({ branch, detached: null, changes: [] }))
    ipc.on('vcs_branches', () => names.map((name) => ({ name, current: name === branch })))
    ipc.on('vcs_tracking', [])
    ipc.on('git_head', () => ({ branch, detached: null }))
    ipc.on('vcs_create_branch', (args) => {
      names.push(args.name)
      if (args.switch) branch = args.name
      return null
    })
  }

  /* `start` is the row the menu was opened on and has nothing to do with where
     HEAD is — the whole point of the item is that the row decides. The name is
     trimmed on the way, since that is the name the dialog's own rule judged. */
  it('a new branch is cut from the row it was asked about rather than from HEAD', async () => {
    const { stores, ipc } = await loadStores()
    cutting(ipc)
    await stores.vcs.loadRepos('/p')
    await stores.git.loadHead('/p')

    await stores.vcs.createBranch({ name: '  feat/login  ', from: 'develop', switch: true })

    expect(ipc.calls('vcs_create_branch')).toEqual([
      { repo: '/p/.', name: 'feat/login', start: 'develop', switch: true }
    ])
    expect(stores.vcs.vcsState.branches.map((b) => b.name)).toContain('feat/login')
    // Switched, so everything drawing the branch moves — the list's mark, the
    // repository row, and the scope bar one store over.
    expect(stores.vcs.vcsState.branches.find((b) => b.current)?.name).toBe('feat/login')
    expect(stores.vcs.vcsState.repos[0].branch).toBe('feat/login')
    expect(stores.git.gitState.branch).toBe('feat/login')
    expect(stores.vcs.vcsState.writeError).toBe(null)
    expect(stores.vcs.vcsState.busy).toBe(null)
  })

  /* The other half of the checkbox: one ref written, the working tree untouched
     and the tick exactly where it was. The list still comes back, because the
     row for the new branch has to appear from somewhere. */
  it('a branch created without switching leaves the mark where it was', async () => {
    const { stores, ipc } = await loadStores()
    cutting(ipc)
    await stores.vcs.loadRepos('/p')
    await stores.git.loadHead('/p')

    await stores.vcs.createBranch({ name: 'feat/quiet', from: 'develop', switch: false })

    expect(stores.vcs.vcsState.branches.map((b) => b.name)).toContain('feat/quiet')
    expect(stores.vcs.vcsState.branches.find((b) => b.current)?.name).toBe('main')
    expect(stores.git.gitState.branch).toBe('main')
  })

  /* A repository whose branch list shrinks when git is told to delete one, so
     the row going — and the name coming off the favourites with it — can be
     watched from the store. `refused` makes the very next delete fail with the
     shape Rust hands back, which is the whole of what the window branches on. */
  const deleting = (ipc, refusal = null) => {
    let names = ['main', 'develop', 'spike']
    ipc.on('vcs_repos', () => answer([{ name: '.', path: '/p/.', branch: 'main', detached: null }]))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', () => names.map((name) => ({ name, current: name === 'main' })))
    ipc.on('vcs_tracking', [])
    ipc.on('git_head', { branch: 'main', detached: null })
    ipc.on('settings_save', null)
    if (refusal) ipc.fail('vcs_delete_branch', refusal)
    else {
      ipc.on('vcs_delete_branch', (args) => {
        names = names.filter((name) => name !== args.branch)
        return null
      })
    }
  }

  it('a deleted branch goes from the list and from the favourites with it', async () => {
    const { stores, ipc } = await loadStores()
    deleting(ipc)
    await stores.vcs.loadRepos('/p')
    stores.settings.settings.project.favoriteBranches = ['spike', 'develop']

    expect(await stores.vcs.deleteBranch('spike')).toBe(true)

    expect(ipc.calls('vcs_delete_branch')).toEqual([
      { repo: '/p/.', branch: 'spike', force: false }
    ])
    expect(stores.vcs.vcsState.branches.map((b) => b.name)).not.toContain('spike')
    /* A pinned name with nothing behind it draws no row, but it would come back
       the day somebody cut a branch of that name again — pinned by a decision
       about a different branch. */
    expect(stores.settings.settings.project.favoriteBranches).toEqual(['develop'])
    expect(stores.vcs.vcsState.writeError).toBe(null)
    expect(stores.vcs.vcsState.busy).toBe(null)
  })

  /* The second press is a different command, and the flag rides in from the
     window that asked rather than being worked out here. */
  it('a forced delete carries the flag git is run with', async () => {
    const { stores, ipc } = await loadStores()
    deleting(ipc)
    await stores.vcs.loadRepos('/p')

    await stores.vcs.deleteBranch('spike', { force: true })

    expect(ipc.calls('vcs_delete_branch')).toEqual([
      { repo: '/p/.', branch: 'spike', force: true }
    ])
  })

  /* **The one write in this store that hands its refusal back out.** The window
     that asked the question is what decides whether to offer a second button,
     and it cannot decide that from a field on a panel. The refusal still lands
     in `writeError` on its way past, because what happened is a fact about this
     repository whether or not that window is still standing. */
  it('a refused delete is thrown to the caller and drawn in the panel as well', async () => {
    const { stores, ipc } = await loadStores()
    deleting(ipc, {
      kind: 'notMerged',
      message: 'spike has commits that are not in the branch this repository is on. Deleting it loses them.'
    })
    await stores.vcs.loadRepos('/p')
    stores.settings.settings.project.favoriteBranches = ['spike']

    await expect(stores.vcs.deleteBranch('spike')).rejects.toMatchObject({
      kind: 'notMerged',
      op: 'delete'
    })

    expect(stores.vcs.vcsState.writeError).toMatchObject({ kind: 'notMerged', op: 'delete' })
    expect(stores.vcs.vcsState.branches.map((b) => b.name)).toContain('spike')
    // Nothing was deleted, so nothing is unmarked either.
    expect(stores.settings.settings.project.favoriteBranches).toEqual(['spike'])
    expect(stores.vcs.vcsState.busy).toBe(null)
  })

  /* Git already working is not a refusal and must not be thrown as one: the
     window would offer a second button about a call nobody made. */
  it('a delete that never left the store answers false rather than throwing', async () => {
    const { stores, ipc } = await loadStores()
    deleting(ipc)
    await stores.vcs.loadRepos('/p')
    stores.vcs.vcsState.busy = { op: 'merge', branch: 'develop' }

    expect(await stores.vcs.deleteBranch('spike')).toBe(false)

    expect(ipc.calls('vcs_delete_branch')).toEqual([])
  })

  /* `write()` answers `true` for a project that moved underneath — the branch
     did go on disk — and `settings.project` is merged **in place** on a switch.
     Without a guard the strike would land on whichever project's list is in
     that object by then, and `main` is pinned in plenty of projects at once. */
  it('does not strike the name out of a project somebody switched to mid-delete', async () => {
    const { stores, ipc } = await loadStores()
    let release
    const held = new Promise((resolve) => {
      release = () => resolve(null)
    })
    deleting(ipc)
    ipc.on('vcs_delete_branch', () => held)
    await stores.vcs.loadRepos('/p')
    stores.settings.settings.activeProject = '/p'
    stores.settings.settings.project.favoriteBranches = ['spike']

    const deleting_ = stores.vcs.deleteBranch('spike')
    /* The switch, in the order `projects.js` performs it: the active project
       first, then the layout merged in place over the same object. */
    stores.settings.settings.activeProject = '/other'
    stores.settings.settings.project.favoriteBranches = ['spike', 'main']
    stores.vcs.vcsState.project = '/other'
    release()
    await deleting_

    expect(stores.settings.settings.project.favoriteBranches).toEqual(['spike', 'main'])
  })

  it('a delete with no branch asks git nothing', async () => {
    const { stores, ipc } = await loadStores()
    deleting(ipc)
    await stores.vcs.loadRepos('/p')

    expect(await stores.vcs.deleteBranch('')).toBe(false)

    expect(ipc.calls('vcs_delete_branch')).toEqual([])
  })

  it('a name that is only whitespace asks git nothing', async () => {
    const { stores, ipc } = await loadStores()
    cutting(ipc)
    await stores.vcs.loadRepos('/p')

    await stores.vcs.createBranch({ name: '   ', from: 'develop', switch: true })

    expect(ipc.calls('vcs_create_branch')).toEqual([])
    expect(stores.vcs.vcsState.busy).toBe(null)
  })

  /* The success path's guard is on the **project** and deliberately not on the
     pair the failure path uses, and this is what says so.

     Repository rows are not held by `busy`, so somebody can pick another
     repository while git works. The branch moved on disk either way, and on the
     pair-guard nothing refreshed after it: no second `vcs_repos`, the row still
     naming the branch just left, and the mark still on it until the next window
     focus. `refresh()` re-reads every repository and re-picks the remembered
     one, so it is right whichever repository is selected by the time it runs —
     which the last assertion holds it to. */
  it('a repository picked mid-checkout does not cost the refresh', async () => {
    const { stores, ipc } = await loadStores()
    /* Two repositories in one project — the only shape in which somebody can
       leave the repository a checkout is running in. Only admin's branch moves;
       backend is there to be switched to. */
    let adminBranch = 'main'
    ipc.on('vcs_repos', () =>
      answer([
        { name: 'admin', path: '/p/admin', branch: adminBranch, detached: null },
        { name: 'backend', path: '/p/backend', branch: 'main', detached: null }
      ])
    )
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', (args) =>
      args.repo === '/p/admin'
        ? [
            { name: 'main', current: adminBranch === 'main' },
            { name: 'develop', current: adminBranch === 'develop' }
          ]
        : [{ name: 'main', current: true }]
    )
    ipc.on('vcs_tracking', [])
    ipc.on('git_head', () => ({ branch: adminBranch, detached: null }))
    await stores.vcs.loadRepos('/p')
    expect(stores.vcs.vcsState.selected).toBe('/p/admin')
    const readsBefore = ipc.calls('vcs_repos').length

    let release
    ipc.on('vcs_checkout', (args) => {
      return new Promise((resolve) => {
        release = () => {
          adminBranch = args.branch
          resolve(null)
        }
      })
    })

    const inFlight = stores.vcs.checkout('develop')
    // The person moves to the other repository while git is still working.
    await stores.vcs.selectRepo('/p/backend')
    release()
    await inFlight

    expect(ipc.calls('vcs_repos').length).toBe(readsBefore + 1)
    expect(stores.vcs.vcsState.repos[0].branch).toBe('develop')
    // And the refresh does not drag the person back to the repository they left.
    expect(stores.vcs.vcsState.selected).toBe('/p/backend')
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

  /* A merge that git finished. The tree is read again afterwards for the same
     reason a checkout re-reads it — the row draws each repository's branch, and
     a merge commit is exactly the sort of thing the changes list has to catch
     up with — and no dialog opens, because there is nothing to answer. */
  it('a merge that went through opens nothing and refreshes the panel', async () => {
    const { stores, ipc } = await loadStores()
    switching(ipc)
    ipc.on('vcs_merge', { kind: 'clean' })
    await stores.vcs.loadRepos('/p')
    const readsBefore = ipc.calls('vcs_repos').length

    await stores.vcs.merge('develop')

    expect(ipc.calls('vcs_merge')).toEqual([{ repo: '/p/.', branch: 'develop' }])
    expect(stores.vcs.vcsState.conflict).toBe(null)
    expect(stores.vcs.vcsState.writeError).toBe(null)
    expect(stores.vcs.vcsState.busy).toBe(null)
    expect(ipc.calls('vcs_repos').length).toBe(readsBefore + 1)
  })

  /* The record the modal is drawn from, and the reason it is a record rather
     than the answer alone: the repository is the one the operation ran in, and
     `ours` is read *before* git is asked, because a rebase stopped on a
     conflict leaves HEAD detached and the branch it moved off is then readable
     nowhere at all. */
  it('a conflict is recorded whole, with the branch the rebase left behind', async () => {
    const { stores, ipc } = await loadStores()
    switching(ipc)
    ipc.on('vcs_rebase', { kind: 'conflict', files: ['src/one.rs', 'src/two.rs'] })
    /* What a repository mid-rebase actually answers: no branch at all. */
    ipc.on('git_head', () => ({ branch: null, detached: 'a1b2c3d' }))
    await stores.vcs.loadRepos('/p')
    stopsOn(ipc, ['src/one.rs', 'src/two.rs'], { op: 'rebase', ours: 'main', theirs: null })

    await stores.vcs.rebase('develop')

    expect(ipc.calls('vcs_rebase')).toEqual([{ repo: '/p/.', onto: 'develop' }])
    expect(stores.vcs.vcsState.conflict).toEqual({
      repo: '/p/.',
      op: 'rebase',
      ours: 'main',
      theirs: 'develop',
      files: ['src/one.rs', 'src/two.rs']
    })
    // A conflict is an outcome and not a failure: nothing goes into the block
    // under the branch list, which is where git's refusals are drawn.
    expect(stores.vcs.vcsState.writeError).toBe(null)
    expect(stores.vcs.vcsState.busy).toBe(null)
  })

  /* A conflict survives the refresh that follows it. `refresh()` goes back
     through `loadRepos` with the same project, and a dialog cleared there would
     be one that opened and closed inside a single call — leaving a conflicted
     tree with nothing on screen naming it. */
  it('the refresh that follows a conflict does not take the dialog down', async () => {
    const { stores, ipc, nextTick } = await loadStores()
    switching(ipc)
    ipc.on('vcs_merge', { kind: 'conflict', files: ['src/one.rs'] })
    await stores.vcs.loadRepos('/p')
    stopsOn(ipc, ['src/one.rs'], { op: 'merge', ours: 'main', theirs: 'develop' })

    await stores.vcs.merge('develop')
    await stores.vcs.refresh()
    await nextTick()

    expect(stores.vcs.vcsState.conflict?.files).toEqual(['src/one.rs'])
  })

  /* Both doors of a conflict act on the project it happened in, so neither
     means anything after a switch: the abort would name a repository nobody is
     looking at and the agent would be started in the wrong project. The
     arriving project is then read on its own — its repository has nothing
     unmerged in it, which is the ordinary case, and the record it gets is the
     one its own tree earns. */
  it('a conflict does not follow the person into another project', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', (args) => answer(repoIn(args.project)))
    ipc.on('vcs_status', (args) => (args.repo === '/p/.' ? conflictedTree() : cleanTree))
    ipc.on('vcs_in_progress', { op: 'merge', ours: 'main', theirs: 'develop' })
    ipc.on('vcs_branches', [])
    ipc.on('vcs_tracking', [])
    ipc.on('git_head', null)
    ipc.on('vcs_merge', { kind: 'conflict', files: ['src/one.js'] })
    await stores.vcs.loadRepos('/p')
    await stores.vcs.merge('develop')
    expect(stores.vcs.vcsState.conflict).not.toBe(null)

    await stores.vcs.loadRepos('/other')

    expect(stores.vcs.vcsState.conflict).toBe(null)
  })

  /* The abort names the operation from the record rather than from anything the
     dialog passes: what git is told to undo is what git was asked to do. */
  it('aborting puts the tree back and closes the dialog', async () => {
    const { stores, ipc } = await loadStores()
    switching(ipc)
    ipc.on('vcs_merge', { kind: 'conflict', files: ['src/one.rs'] })
    await stores.vcs.loadRepos('/p')
    stopsOn(ipc, ['src/one.rs'], { op: 'merge', ours: 'main', theirs: 'develop' })
    /* The abort is what puts the tree back, so the tree answers as git would on
       either side of it — unmerged until then, clean after. */
    let aborted = false
    ipc.on('vcs_status', () => (aborted ? cleanTree : conflictedTree('src/one.rs')))
    ipc.on('vcs_abort', () => {
      aborted = true
      return null
    })
    await stores.vcs.merge('develop')

    await stores.vcs.abortConflict()

    expect(ipc.calls('vcs_abort')).toEqual([{ repo: '/p/.', op: 'merge' }])
    expect(stores.vcs.vcsState.conflict).toBe(null)
    expect(stores.vcs.vcsState.conflictError).toBe(null)
    expect(stores.vcs.vcsState.busy).toBe(null)
  })

  /* An abort git refused leaves the dialog standing with git's own words in
     it. Closing it would leave a tree that is still conflicted with nothing on
     screen saying so, and the message cannot go in the panel behind a dialog
     that has no dismiss. */
  it('an abort git refused keeps the dialog open and says why', async () => {
    const { stores, ipc } = await loadStores()
    switching(ipc)
    ipc.on('vcs_merge', { kind: 'conflict', files: ['src/one.rs'] })
    ipc.fail('vcs_abort', {
      kind: 'git',
      message: 'fatal: There is no merge to abort (MERGE_HEAD missing).'
    })
    await stores.vcs.loadRepos('/p')
    stopsOn(ipc, ['src/one.rs'], { op: 'merge', ours: 'main', theirs: 'develop' })
    await stores.vcs.merge('develop')

    await stores.vcs.abortConflict()

    expect(stores.vcs.vcsState.conflict).not.toBe(null)
    expect(stores.vcs.vcsState.conflictError).toEqual({
      kind: 'git',
      message: 'fatal: There is no merge to abort (MERGE_HEAD missing).'
    })
    expect(stores.vcs.vcsState.busy).toBe(null)
  })

  /* A repository somebody left mid-merge — from a terminal, before the app was
     started, or by an agent in the same tree. Nothing in this session ran the
     merge, so the tree and the probe over it are the only sources there are. */
  it('reads a stopped merge off a conflicted tree', async () => {
    const { stores, ipc } = await loadStores()
    await openPanel(stores, ipc, conflictedTree(), {
      op: 'merge',
      ours: 'main',
      theirs: 'feature'
    })

    expect(stores.vcs.vcsState.conflict).toEqual({
      repo: '/p/.',
      op: 'merge',
      ours: 'main',
      theirs: 'feature',
      files: ['src/one.js']
    })
    /* Read and not raised: a tree nobody touched in this session gets a button,
       never a dialog over the panel somebody just opened. */
    expect(stores.vcs.vcsState.conflictOpen).toBe(false)
  })

  it('asks git nothing about a clean tree', async () => {
    const { stores, ipc } = await loadStores()
    await openPanel(stores, ipc, cleanTree)

    expect(ipc.calls('vcs_in_progress')).toEqual([])
    expect(stores.vcs.vcsState.conflict).toBe(null)
  })

  /* A cherry-pick, a revert, a stash pop, a `checkout --merge`: unmerged paths
     with neither of the dialog's two doors true. The probe says so and the
     panel draws no button. */
  it('holds no record when neither operation is in progress', async () => {
    const { stores, ipc } = await loadStores()
    await openPanel(stores, ipc, conflictedTree(), null)

    expect(ipc.calls('vcs_in_progress')).toEqual([{ repo: '/p/.' }])
    expect(stores.vcs.vcsState.conflict).toBe(null)
    expect(stores.vcs.vcsState.conflictOpen).toBe(false)
  })

  /* The whole point of splitting the flag off the record: "Resolve with an
     agent" takes the dialog down and leaves the tree exactly as git left it, so
     the way back in has to survive it. */
  it('keeps the record when the dialog is dismissed', async () => {
    const { stores, ipc } = await loadStores()
    await openPanel(stores, ipc, conflictedTree(), {
      op: 'merge',
      ours: 'main',
      theirs: 'feature'
    })

    stores.vcs.openConflict()
    expect(stores.vcs.vcsState.conflictOpen).toBe(true)

    stores.vcs.dismissConflict()
    expect(stores.vcs.vcsState.conflictOpen).toBe(false)
    expect(stores.vcs.vcsState.conflict).not.toBe(null)
  })

  it('opens the dialog by itself when a merge it ran stopped on a conflict', async () => {
    const { stores, ipc } = await loadStores()
    await openPanel(stores, ipc, conflictedTree(), {
      op: 'merge',
      ours: 'main',
      theirs: 'feature'
    })
    ipc.on('vcs_merge', { kind: 'conflict', files: ['src/one.js'] })
    ipc.on('git_head', null)

    await stores.vcs.merge('feature')

    expect(stores.vcs.vcsState.conflictOpen).toBe(true)
    expect(stores.vcs.vcsState.conflict.op).toBe('merge')
  })

  /* The press knew what the rebase was onto; the probe never can, because a
     stopped rebase leaves HEAD detached. The refresh that follows the press
     must not overwrite the name with nothing. */
  it('keeps the branch a rebase is onto when the probe cannot name it', async () => {
    const { stores, ipc } = await loadStores()
    await openPanel(stores, ipc, conflictedTree(), {
      op: 'rebase',
      ours: 'feature',
      theirs: null
    })
    ipc.on('vcs_rebase', { kind: 'conflict', files: ['src/one.js'] })
    ipc.on('git_head', null)

    await stores.vcs.rebase('main')

    expect(stores.vcs.vcsState.conflict.theirs).toBe('main')
  })

  /* The agent finished, or somebody aborted in a terminal: the next status read
     shows a clean tree and the button has to go with it. */
  it('forgets the conflict when the tree comes back clean', async () => {
    const { stores, ipc } = await loadStores()
    await openPanel(stores, ipc, conflictedTree(), {
      op: 'merge',
      ours: 'main',
      theirs: 'feature'
    })
    expect(stores.vcs.vcsState.conflict).not.toBe(null)

    ipc.on('vcs_status', cleanTree)
    await stores.vcs.refresh()

    expect(stores.vcs.vcsState.conflict).toBe(null)
    expect(stores.vcs.vcsState.conflictOpen).toBe(false)
  })

  /* git declining to answer is not git saying there is nothing. A probe that
     failed leaves the record where it was, because the other arm is a button
     that vanishes under the pointer over a tree that is still conflicted. */
  it('a probe that failed leaves the record standing', async () => {
    const { stores, ipc } = await loadStores()
    await openPanel(stores, ipc, conflictedTree(), {
      op: 'merge',
      ours: 'main',
      theirs: 'feature'
    })
    vi.spyOn(console, 'error').mockImplementation(() => {})
    ipc.fail('vcs_in_progress', { kind: 'noGit', message: 'git is not on this machine' })

    await stores.vcs.refresh()

    expect(stores.vcs.vcsState.conflict?.op).toBe('merge')
  })

  /* One at a time across all three writes and not one apiece: what a second
     press asks for is git working in a tree git is already working in, and
     which of the two operations it was makes no difference to `index.lock`. */
  it('an operation in flight refuses every other write', async () => {
    const { stores, ipc } = await loadStores()
    switching(ipc)
    await stores.vcs.loadRepos('/p')
    let release
    ipc.on('vcs_merge', () => new Promise((resolve) => (release = () => resolve({ kind: 'clean' }))))

    const first = stores.vcs.merge('develop')
    await stores.vcs.checkout('main')
    await stores.vcs.rebase('main')
    expect(ipc.calls('vcs_checkout')).toEqual([])
    expect(ipc.calls('vcs_rebase')).toEqual([])
    release()
    await first
  })

  /* git's refusal of a merge is drawn by the same block a refused checkout
     gets, and the `op` that comes with it is the whole of what tells them
     apart: a title reading "did not switch branch" over this would name an
     operation nobody asked for. */
  it('a refused merge carries which write it was', async () => {
    const { stores, ipc } = await loadStores()
    switching(ipc)
    await stores.vcs.loadRepos('/p')
    ipc.fail('vcs_merge', {
      kind: 'git',
      message: 'error: Your local changes to the following files would be overwritten by merge:'
    })

    await stores.vcs.merge('develop')

    expect(stores.vcs.vcsState.writeError?.op).toBe('merge')
    expect(stores.vcs.vcsState.error).toBe(null)
    expect(stores.vcs.vcsState.conflict).toBe(null)
  })

  it('a failure leaves an error to draw, not a half-filled panel', async () => {
    const { stores, ipc, nextTick } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_branches', [])
    ipc.on('vcs_tracking', [])
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
  /* A repository with something uncommitted in it, which is what the commit box
     needs to be drawn over at all. The tree empties once git has committed, so
     what the panel says afterwards is what the tests below read. */
  const committing = (ipc) => {
    let dirty = true
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', () => ({
      branch: 'main',
      detached: null,
      changes: dirty
        ? [{ path: 'a.rs', origPath: null, kind: 'modified', staged: false, unstaged: true }]
        : []
    }))
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    ipc.on('git_head', () => ({ branch: 'main', detached: null }))
    ipc.on('vcs_commit', () => {
      dirty = false
      return null
    })
  }

  it('a commit takes the draft, empties the tree and clears the field', async () => {
    const { stores, ipc } = await loadStores()
    committing(ipc)
    await stores.vcs.loadRepos('/p')
    stores.vcs.setMessage('fix: the thing')

    await stores.vcs.commit()

    expect(ipc.calls('vcs_commit')).toEqual([{ repo: '/p/.', message: 'fix: the thing' }])
    expect(stores.vcs.vcsState.tree.changes).toEqual([])
    expect(stores.vcs.draftMessage()).toBe('')
    expect(stores.vcs.vcsState.busy).toBe(null)
  })

  /* The one thing a refused commit must not do is throw the sentence away: it
     is what somebody would have to type a second time, and git refusing a hook
     or an unset identity is exactly the case they will fix and press again. */
  it('a refused commit keeps the draft and says which write it was', async () => {
    const { stores, ipc } = await loadStores()
    committing(ipc)
    await stores.vcs.loadRepos('/p')
    stores.vcs.setMessage('fix: the thing')
    ipc.fail('vcs_commit', {
      kind: 'git',
      message: 'Author identity unknown\n\n*** Please tell me who you are.'
    })

    await stores.vcs.commit()

    expect(stores.vcs.vcsState.writeError?.op).toBe('commit')
    expect(stores.vcs.draftMessage()).toBe('fix: the thing')
    expect(stores.vcs.vcsState.busy).toBe(null)
  })

  /* `write` bails on a busy panel without doing anything, and the draft has to
     survive that too: reading "no error afterwards" as "committed" would throw
     the sentence away with nothing committed, which is the same loss as a
     refusal and harder to notice. */
  it('a commit pressed while git is busy keeps the draft', async () => {
    const { stores, ipc } = await loadStores()
    committing(ipc)
    let release
    ipc.on('vcs_checkout', () => new Promise((resolve) => (release = resolve)))
    await stores.vcs.loadRepos('/p')
    stores.vcs.setMessage('fix: the thing')

    const slow = stores.vcs.checkout('main')
    await stores.vcs.commit()

    expect(ipc.calls('vcs_commit')).toEqual([])
    expect(stores.vcs.draftMessage()).toBe('fix: the thing')
    release(null)
    await slow
  })

  it('a commit with nothing written asks git nothing at all', async () => {
    const { stores, ipc } = await loadStores()
    committing(ipc)
    await stores.vcs.loadRepos('/p')
    stores.vcs.setMessage('   \n ')

    await stores.vcs.commit()

    expect(ipc.calls('vcs_commit')).toEqual([])
  })

  /* Drafts are per repository, because a project is often several of them and
     the sentences are about different work. Switching away and back has to find
     the message where it was left — the field is the one place in this panel
     holding something a person typed. */
  it('each repository keeps its own draft', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(siblings))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [])
    ipc.on('vcs_tracking', [])
    await stores.vcs.loadRepos('/p')

    stores.vcs.setMessage('one')
    await stores.vcs.selectRepo('/p/backend')
    expect(stores.vcs.draftMessage()).toBe('')
    stores.vcs.setMessage('two')
    await stores.vcs.selectRepo('/p/admin')

    expect(stores.vcs.draftMessage()).toBe('one')
  })

  it('the drafts do not follow the person into another project', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', (args) => answer(repoIn(args.project)))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [])
    ipc.on('vcs_tracking', [])
    await stores.vcs.loadRepos('/p')
    stores.vcs.setMessage('about this project')

    await stores.vcs.loadRepos('/other')

    expect(stores.vcs.draftMessage()).toBe('')
  })

  it('the agent’s message lands in the field', async () => {
    const { stores, ipc } = await loadStores()
    committing(ipc)
    ipc.on('vcs_suggest_message', 'chore: bump the sidecar')
    await stores.vcs.loadRepos('/p')

    await stores.vcs.suggestMessage()

    expect(stores.vcs.draftMessage()).toBe('chore: bump the sidecar')
    expect(stores.vcs.vcsState.suggesting).toBe(false)
    expect(stores.vcs.vcsState.suggestError).toBe(null)
  })

  /* Generating writes nothing, so it must not go through `busy` and take the
     branch rows down with it — and its failure must not reach `writeError`,
     where the panel would title it "Git refused this operation" over a party
     that was never asked. */
  it('asking the agent leaves the branch rows alone and keeps its own error', async () => {
    const { stores, ipc } = await loadStores()
    committing(ipc)
    ipc.fail('vcs_suggest_message', {
      kind: 'noAgent',
      message: 'Smetana looked for claude on your PATH and found nothing.'
    })
    await stores.vcs.loadRepos('/p')

    await stores.vcs.suggestMessage()

    expect(stores.vcs.vcsState.suggestError).toEqual({
      kind: 'noAgent',
      message: 'Smetana looked for claude on your PATH and found nothing.'
    })
    expect(stores.vcs.vcsState.writeError).toBe(null)
    expect(stores.vcs.vcsState.busy).toBe(null)
    expect(stores.vcs.vcsState.suggesting).toBe(false)
  })

  /* The guard that matters most in this store, since what it stops is a
     sentence about one repository's work sitting in another repository's field
     one keystroke away from being committed there. */
  it('a message arriving after the repository changed is dropped', async () => {
    const { stores, ipc, nextTick } = await loadStores()
    ipc.on('vcs_repos', answer(siblings))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [])
    ipc.on('vcs_tracking', [])
    let release
    ipc.on('vcs_suggest_message', () => new Promise((resolve) => (release = resolve)))
    await stores.vcs.loadRepos('/p')

    const asked = stores.vcs.suggestMessage()
    await stores.vcs.selectRepo('/p/backend')
    release('feat: about admin')
    await asked
    await nextTick()

    expect(stores.vcs.draftMessage()).toBe('')
    expect(stores.vcs.vcsState.messages['/p/admin']).toBe(undefined)
  })

  it('a remembered repository that is no longer there is replaced by the first', async () => {
    const { stores, ipc } = await loadStores()
    stores.settings.settings.project.selectedRepo = '/p/gone'
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', branchesOf('/p'))
    ipc.on('vcs_tracking', [])

    await stores.vcs.loadRepos('/p')

    expect(stores.vcs.vcsState.selected).toBe('/p/.')
    expect(stores.settings.settings.project.selectedRepo).toBe('/p/.')
    expect(stores.vcs.vcsState.error).toBe(null)
  })
  /* The tracking read is a second answer merged by name, which is what lets
     `vcs_branches` stay the process-free call its own documentation promises.
     Keyed rather than listed, because every consumer of it has a branch name in
     its hand and no consumer wants the order. */
  it('tracking is merged into the panel keyed by branch name', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }, { name: 'spike', current: false }])
    ipc.on('vcs_tracking', [
      { branch: 'main', upstream: 'origin/main', ahead: 0, behind: 2, gone: false }
    ])

    await stores.vcs.loadRepos('/p')

    expect(stores.vcs.vcsState.tracking.main.behind).toBe(2)
    expect(stores.vcs.vcsState.tracking.spike).toBeUndefined()
  })

  /* The same guard every read in this store carries. An answer landing after
     somebody switched repositories would mark one repository's branches with
     another repository's counts — and the mark is the thing a person acts on. */
  it('tracking for a repository already left is dropped', async () => {
    const { stores, ipc, nextTick } = await loadStores()
    ipc.on('vcs_repos', answer(siblings))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    await stores.vcs.loadRepos('/p')

    /* Held back until after the switch, exactly as the branch list above is:
       the opening read has to answer at once, or `loadRepos` itself would hang
       on a call this case is not about. */
    let releaseAdmin
    const admin = new Promise((resolve) => {
      releaseAdmin = () =>
        resolve([{ branch: 'main', upstream: 'origin/main', ahead: 0, behind: 9, gone: false }])
    })
    ipc.on('vcs_tracking', (args) => (args.repo === '/p/admin' ? admin : []))

    const first = stores.vcs.selectRepo('/p/admin')
    const second = stores.vcs.selectRepo('/p/backend')
    await second
    releaseAdmin()
    await first
    await nextTick()

    expect(stores.vcs.vcsState.selected).toBe('/p/backend')
    expect(stores.vcs.vcsState.tracking.main).toBeUndefined()
  })

  /* A failure here takes no mark and no message: the row goes back to what it
     looked like before this feature existed, which is a true statement about
     what is known. */
  it('a tracking read that failed leaves the panel unmarked and quiet', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', () => Promise.reject(new Error('no git')))

    await stores.vcs.loadRepos('/p')

    expect(stores.vcs.vcsState.tracking).toEqual({})
    expect(stores.vcs.vcsState.error).toBeNull()
    expect(stores.vcs.vcsState.writeError).toBeNull()
  })
  /* Five minutes, in a store that has no clock of its own: the test moves the
     clock rather than waiting, and what is asserted is that the second sweep
     did not go to the network at all. */
  it('the background fetch runs once and then not again for five minutes', async () => {
    vi.useFakeTimers()
    try {
      const { stores, ipc } = await loadStores()
      let fetches = 0
      ipc.on('vcs_repos', answer(repoIn('/p')))
      ipc.on('vcs_status', cleanTree)
      ipc.on('vcs_branches', [{ name: 'main', current: true }])
      ipc.on('vcs_tracking', [])
      ipc.on('vcs_fetch', () => {
        fetches += 1
        return null
      })

      await stores.vcs.loadRepos('/p')
      await stores.vcs.autoFetch()
      await stores.vcs.autoFetch()
      expect(fetches).toBe(1)

      vi.advanceTimersByTime(5 * 60 * 1000 + 1)
      await stores.vcs.autoFetch()
      expect(fetches).toBe(2)
    } finally {
      vi.useRealTimers()
    }
  })

  /* The switch is the whole of what it promises: off means this app opens no
     socket by itself. */
  it('the background fetch does nothing when the setting is off', async () => {
    const { stores, ipc } = await loadStores()
    let fetches = 0
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    ipc.on('vcs_fetch', () => {
      fetches += 1
      return null
    })

    await stores.vcs.loadRepos('/p')
    stores.settings.settings.git.autoFetch = false
    await stores.vcs.autoFetch()

    expect(fetches).toBe(0)
  })

  /* Nobody started it, so nobody is told. An offline machine must be usable all
     day without a red block in the sidebar. */
  it('a background fetch that failed says nothing on screen', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    ipc.on('vcs_fetch', () => Promise.reject({ kind: 'git', message: 'could not read from remote' }))

    await stores.vcs.loadRepos('/p')
    await stores.vcs.autoFetch()

    expect(stores.vcs.vcsState.writeError).toBeNull()
    expect(stores.vcs.vcsState.error).toBeNull()
  })

  /* A fetch that landed is only half of what the sweep is for: the marks are
     drawn from the tracking read, so one that is not repeated afterwards leaves
     the panel exactly as stale as it was before the network call. */
  it('the marks are read again once a fetch has landed', async () => {
    const { stores, ipc } = await loadStores()
    let fetched = false
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', () =>
      fetched ? [{ branch: 'main', upstream: 'origin/main', ahead: 0, behind: 1, gone: false }] : []
    )
    ipc.on('vcs_fetch', () => {
      fetched = true
      return null
    })

    await stores.vcs.loadRepos('/p')
    expect(stores.vcs.vcsState.tracking.main).toBeUndefined()
    await stores.vcs.autoFetch()

    expect(stores.vcs.vcsState.tracking.main.behind).toBe(1)
  })
  /* The guard is not memory, and the difference shows only here: closing the
     project empties the throttle's stamps, and emptying the set of what is in
     flight with them would drop the guard over a call that is still open —
     leaving the very next sweep free to open a second `git fetch` in the same
     repository. */
  it('a fetch still in flight keeps its guard when the project is closed', async () => {
    const { stores, ipc } = await loadStores()
    let fetches = 0
    let land = null
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    ipc.on('vcs_fetch', () => {
      fetches += 1
      /* Deliberately never resolved until the end: a network call is the one
         thing here that can still be open a minute later. */
      return new Promise((resolve) => {
        land = resolve
      })
    })

    await stores.vcs.loadRepos('/p')
    const inFlight = stores.vcs.autoFetch()
    /* A window with no project at all, which is the only thing that resets this
       store — and then the same project again. */
    await stores.vcs.loadRepos(null)
    await stores.vcs.loadRepos('/p')
    await stores.vcs.autoFetch()

    expect(fetches).toBe(1)

    land(null)
    await inFlight
  })

  /* The button exists because both verbs beside it are refused over a branch
     that is level, and the sweep behind it may have been switched off. A press
     that then had to wait out a five-minute throttle would be a control that
     did nothing and said nothing. */
  it('a pressed check ignores both the setting and the throttle', async () => {
    const { stores, ipc } = await loadStores()
    let fetches = 0
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    ipc.on('vcs_fetch', () => {
      fetches += 1
      return null
    })

    await stores.vcs.loadRepos('/p')
    stores.settings.settings.git.autoFetch = false
    await stores.vcs.fetchNow()
    await stores.vcs.fetchNow()

    expect(fetches).toBe(2)
  })

  /* The whole point of the press: the number the panel dims Pull over is read
     again, so a branch that has fallen behind since the last sweep says so. */
  it('a pressed check reads the marks again', async () => {
    const { stores, ipc } = await loadStores()
    let fetched = false
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', () =>
      fetched ? [{ branch: 'main', upstream: 'origin/main', ahead: 0, behind: 2, gone: false }] : []
    )
    ipc.on('vcs_fetch', () => {
      fetched = true
      return null
    })

    await stores.vcs.loadRepos('/p')
    await stores.vcs.fetchNow()

    expect(stores.vcs.vcsState.tracking.main.behind).toBe(2)
  })

  /* The opposite of the sweep's silence, and for the reason the sweep is
     silent: somebody pressed this one, so a remote that cannot be reached is
     an answer they are waiting for rather than a machine's ordinary state. */
  it('a pressed check that failed says so in the panel', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    ipc.on('vcs_fetch', () => Promise.reject({ kind: 'git', message: 'could not read from remote' }))

    await stores.vcs.loadRepos('/p')
    await stores.vcs.fetchNow()

    expect(stores.vcs.vcsState.writeError).toMatchObject({
      op: 'fetch',
      message: 'could not read from remote'
    })
    expect(stores.vcs.vcsState.fetching).toBe(false)
  })

  /* One call per repository, whoever asked for it: a second `git fetch` would
     only queue behind the first. What the press does instead is join the call
     already running — it spins over the sweep's fetch and answers when that
     one answers, rather than being the no-op a plain guard would have made
     it. */
  it('a press while a sweep is still out joins it instead of opening a second', async () => {
    const { stores, ipc } = await loadStores()
    let fetches = 0
    let land = null
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    ipc.on('vcs_fetch', () => {
      fetches += 1
      return new Promise((resolve) => {
        land = resolve
      })
    })

    await stores.vcs.loadRepos('/p')
    const inFlight = stores.vcs.autoFetch()
    const pressed = stores.vcs.fetchNow()
    await Promise.resolve()

    expect(fetches).toBe(1)
    /* The button is spinning over somebody else's call, which is the whole
       point of joining it. */
    expect(stores.vcs.vcsState.fetching).toBe(true)

    land(null)
    await inFlight
    await pressed

    expect(stores.vcs.vcsState.fetching).toBe(false)
  })

  /* A conflicted pull is an outcome and not a failure, and it must reach the
     same dialog a conflicted merge reaches — with `merge` as its op, since that
     is what decides whether the abort runs `git merge --abort`. */
  it('a pull that conflicted opens the conflict dialog as a merge', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [
      { branch: 'main', upstream: 'origin/main', ahead: 1, behind: 1, gone: false }
    ])
    ipc.on('git_head', { branch: 'main', detached: null })
    ipc.on('vcs_pull', { kind: 'conflict', files: ['src/lib.rs'] })

    await stores.vcs.loadRepos('/p')
    /* The probe over the tree the pull left names no `theirs` at all: a pull
       merges a **remote** ref, and `name-rev --refs=refs/heads/*` has no local
       branch to answer with. So `origin/main` survives only because the press
       knew it, which is the rule `conflictRecord` is. */
    stopsOn(ipc, ['src/lib.rs'], { op: 'merge', ours: 'main', theirs: null })
    await stores.vcs.pull()

    expect(stores.vcs.vcsState.conflict).toMatchObject({
      repo: '/p/.',
      op: 'merge',
      ours: 'main',
      /* What git was bringing in, which for a pull is the upstream and not the
         branch: the modal draws both sides of the sentence, and a record naming
         `main` twice would be one about nothing. */
      theirs: 'origin/main',
      files: ['src/lib.rs']
    })
  })

  /* Git refused a push somebody pressed, so this one is loud — the opposite of
     the background fetch, and the difference is who asked. */
  it('a refused push lands in writeError with its op', async () => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [
      { branch: 'main', upstream: 'origin/main', ahead: 1, behind: 0, gone: false }
    ])
    ipc.on('vcs_push', () => Promise.reject({ kind: 'git', message: 'rejected: non-fast-forward' }))

    await stores.vcs.loadRepos('/p')
    await stores.vcs.push()

    expect(stores.vcs.vcsState.writeError).toMatchObject({
      op: 'push',
      message: 'rejected: non-fast-forward'
    })
  })

  /* The one place the front end decides how git is called: a branch with no
     upstream is published rather than pushed. */
  it('a branch with no upstream is pushed with set-upstream', async () => {
    const { stores, ipc } = await loadStores()
    let asked = null
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'spike', current: true }])
    ipc.on('vcs_tracking', [{ branch: 'spike', upstream: null, ahead: 0, behind: 0, gone: false }])
    ipc.on('git_head', { branch: 'spike', detached: null })
    ipc.on('vcs_push', (args) => {
      asked = args
      return null
    })

    await stores.vcs.loadRepos('/p')
    await stores.vcs.push()

    expect(asked).toMatchObject({ repo: '/p/.', setUpstream: true })
  })

  /* The ordinary shape, and the half of the decision the test above cannot
     see: a branch with an upstream is pushed to the one it has, and `-u` would
     be this app choosing a remote for a branch that already named one. */
  it('a branch with an upstream is pushed without set-upstream', async () => {
    const { stores, ipc } = await loadStores()
    let asked = null
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [
      { branch: 'main', upstream: 'origin/main', ahead: 2, behind: 0, gone: false }
    ])
    ipc.on('git_head', { branch: 'main', detached: null })
    ipc.on('vcs_push', (args) => {
      asked = args
      return null
    })

    await stores.vcs.loadRepos('/p')
    await stores.vcs.push()

    expect(asked).toMatchObject({ repo: '/p/.', setUpstream: false })
  })

  /* The third state, and the one a second copy of the rule would get wrong
     first: a branch whose upstream was deleted on the remote still names one,
     so anything reading `upstream` alone would push plainly and be refused by
     git for a branch this app had just offered to publish. */
  it('a branch whose upstream is gone is published rather than pushed', async () => {
    const { stores, ipc } = await loadStores()
    let asked = null
    ipc.on('vcs_repos', answer(repoIn('/p')))
    ipc.on('vcs_status', cleanTree)
    ipc.on('vcs_branches', [{ name: 'old', current: true }])
    ipc.on('vcs_tracking', [
      { branch: 'old', upstream: 'origin/old', ahead: 1, behind: 0, gone: true }
    ])
    ipc.on('git_head', { branch: 'old', detached: null })
    ipc.on('vcs_push', (args) => {
      asked = args
      return null
    })

    await stores.vcs.loadRepos('/p')
    await stores.vcs.push()

    expect(asked).toMatchObject({ repo: '/p/.', setUpstream: true })
  })

  /* What none of the three above can catch on its own, and the reason the rule
     is one exported function rather than one expression written out twice: the
     word on the button and the arguments git is run with have to be answers to
     the same question. A copy that drifted would leave the caption reading
     "Publish branch" over a plain `git push`, with `tracking.test.js` and this
     file both still green. */
  it('what the caption says it will do is what the store asks for', async () => {
    const records = [
      { branch: 'main', upstream: 'origin/main', ahead: 2, behind: 0, gone: false },
      { branch: 'main', upstream: null, ahead: 0, behind: 0, gone: false },
      { branch: 'main', upstream: 'origin/main', ahead: 1, behind: 0, gone: true }
    ]
    for (const record of records) {
      const { stores, ipc } = await loadStores()
      let asked = null
      ipc.on('vcs_repos', answer(repoIn('/p')))
      ipc.on('vcs_status', cleanTree)
      ipc.on('vcs_branches', [{ name: 'main', current: true }])
      ipc.on('vcs_tracking', [record])
      ipc.on('git_head', { branch: 'main', detached: null })
      ipc.on('vcs_push', (args) => {
        asked = args
        return null
      })

      await stores.vcs.loadRepos('/p')
      await stores.vcs.push()

      expect(asked.setUpstream).toBe(pushAction(record, { allowed: true }).setUpstream)
    }
  })
})

/* The status footer's first counter. It reads the very list the panel draws,
   and these three cases are the whole of the rule: every kind of change
   counts, nothing to commit is zero, and a tree nobody could read is not
   zero. */
describe('the uncommitted file count in the status footer', () => {
  const load = async (statusReply) => {
    const { stores, ipc } = await loadStores()
    ipc.on('vcs_repos', answer(repoIn('/p')))
    statusReply(ipc)
    ipc.on('vcs_branches', [{ name: 'main', current: true }])
    ipc.on('vcs_tracking', [])
    await stores.vcs.loadRepos('/p')
    return stores.vcs
  }

  it('a clean tree counts zero, which is a fact and not an absence', async () => {
    const vcs = await load((ipc) => ipc.on('vcs_status', cleanTree))
    expect(vcs.vcsState.tree).not.toBeNull()
    expect(vcs.dirtyCount.value).toBe(0)
  })

  /* Every kind is one file, untracked and conflicted included. The counter is
     the length of the list on screen and nothing cleverer, so a person can
     check the bar against the panel by looking; a version that counted only
     tracked files would differ from the list under it by however many files
     had just been created. */
  it('every kind of change counts as one file', async () => {
    const changes = [
      { path: 'a.rs', origPath: null, kind: 'modified', staged: false, unstaged: true },
      { path: 'b.rs', origPath: null, kind: 'added', staged: true, unstaged: false },
      { path: 'c.rs', origPath: null, kind: 'deleted', staged: true, unstaged: false },
      { path: 'd.rs', origPath: 'was.rs', kind: 'renamed', staged: true, unstaged: false },
      { path: 'e.rs', origPath: null, kind: 'untracked', staged: false, unstaged: true },
      { path: 'f.rs', origPath: null, kind: 'conflicted', staged: false, unstaged: true }
    ]
    const vcs = await load((ipc) => ipc.on('vcs_status', { branch: 'main', detached: null, changes }))
    expect(vcs.dirtyCount.value).toBe(6)
  })

  /* Unknown is not zero — the opposition the store keeps for `tree` itself. A
     repository whose status could not be read has an unknown number of
     uncommitted files, and the strip draws no counter for it; answering `0` would
     be this app telling somebody their work is committed. */
  it('a tree that could not be read counts nothing at all, not zero', async () => {
    const vcs = await load((ipc) => ipc.fail('vcs_status', { kind: 'noGit', message: 'git not found' }))
    expect(vcs.vcsState.tree).toBeNull()
    expect(vcs.dirtyCount.value).toBeNull()
  })
})
