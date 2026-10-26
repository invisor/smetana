import { describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

const split = (over = {}) => ({
  mode: 'client',
  state: 'connected',
  epoch: 1,
  attempt: 0,
  reason: null,
  local: true,
  server: null,
  outdated: null,
  ...over
})

const attachment = (path) => ({ path, name: 'shot.png', bytes: 3, mime: 'image/png', data: 'cG5n' })

async function load(view) {
  const loaded = await loadStores()
  loaded.ipc.on('connection_state', view)
  return loaded
}

describe('paths for a server', () => {
  it('hands the paths back untouched when the server is on this machine, and reads nothing', async () => {
    const { ipc, stores } = await load(split({ local: true }))
    const { paths, failures } = await stores.serverFiles.pathsForServer(['/Users/me/a.png', '/Users/me/b.pdf'])
    expect(failures).toEqual([])
    expect(paths).toEqual(['/Users/me/a.png', '/Users/me/b.pdf'])
    expect(ipc.calls('client_file_read')).toHaveLength(0)
    expect(ipc.calls('attachment_write')).toHaveLength(0)
  })

  it('treats a build with no server of its own, and a browser, as local', async () => {
    const { ipc, stores } = await load({ mode: 'browser', state: 'connected', epoch: 0 })
    expect((await stores.serverFiles.pathsForServer(['/x.png'])).paths).toEqual(['/x.png'])
    ipc.fail('connection_state', new Error('no such command'))
    expect(await stores.serverFiles.serverIsRemote()).toBe(false)
  })

  it('uploads each file for a remote server, in order, and answers the paths the server stored', async () => {
    const { ipc, stores } = await load(split({ local: false }))
    ipc.on('client_file_read', (args) => ({ name: args.path.split('/').pop(), bytes: 3, data: 'cG5n' }))
    ipc.on('attachment_write', (args) => attachment(`/srv/attachments/p/${args.name}`))
    const { paths } = await stores.serverFiles.pathsForServer(['/Users/me/a.png', '/Users/me/b.pdf'])
    expect(paths).toEqual(['/srv/attachments/p/a.png', '/srv/attachments/p/b.pdf'])
    expect(ipc.calls('client_file_read').map((c) => c.path)).toEqual(['/Users/me/a.png', '/Users/me/b.pdf'])
    expect(ipc.calls('attachment_write')[0]).toEqual({ name: 'a.png', data: 'cG5n' })
  })

  it('keeps the files that arrive and names each one that did not, instead of losing the batch', async () => {
    const { ipc, stores } = await load(split({ local: false }))
    ipc.on('client_file_read', (args) => {
      if (args.path.endsWith('huge.mov')) throw 'huge.mov is 99999999 bytes; the ceiling is 10485760 bytes'
      return { name: args.path.split('/').pop(), bytes: 3, data: 'cG5n' }
    })
    ipc.on('attachment_write', (args) => attachment(`/srv/attachments/p/${args.name}`))
    const { paths, failures } = await stores.serverFiles.pathsForServer(['/a.png', '/huge.mov', '/c.png'])
    expect(paths).toEqual(['/srv/attachments/p/a.png', '/srv/attachments/p/c.png'])
    expect(failures).toEqual([{ path: '/huge.mov', message: expect.stringContaining('ceiling') }])
    expect(stores.serverFiles.failureSentence(failures)).toMatch(/ceiling/)
    expect(stores.serverFiles.failureSentence([...failures, ...failures])).toMatch(/and 1 more/)
    expect(stores.serverFiles.failureSentence([])).toBe('')
  })

  it('asks the shell fresh in a window that never started the connection store', async () => {
    const { ipc, stores } = await load(split({ local: true }))
    expect(await stores.serverFiles.serverIsRemote()).toBe(false)
    ipc.on('connection_state', split({ local: false }))
    expect(await stores.serverFiles.serverIsRemote()).toBe(true)
  })
})

describe('a report in the person\'s browser', () => {
  it('opens the file where it is when the server is on this machine', async () => {
    const { ipc, stores } = await load(split({ local: true }))
    ipc.on('client_open_document', null)
    await stores.serverFiles.openReportInBrowser('/work/app/.smetana/reports/run-1.html')
    expect(ipc.calls('client_open_document')).toEqual([{ path: '/work/app/.smetana/reports/run-1.html' }])
    expect(ipc.calls('server_file_read')).toHaveLength(0)
  })

  it('fetches the bytes over the connection and has the shell open a temporary copy for a remote server', async () => {
    const { ipc, stores } = await load(split({ local: false }))
    ipc.on('server_file_read', { name: 'run-1.html', bytes: 6, data: 'PGh0bWw+' })
    ipc.on('client_open_fetched', null)
    await stores.serverFiles.openReportInBrowser('/srv/app/.smetana/reports/run-1.html')
    expect(ipc.calls('server_file_read')).toEqual([{ path: '/srv/app/.smetana/reports/run-1.html' }])
    expect(ipc.calls('client_open_fetched')).toEqual([{ name: 'run-1.html', data: 'PGh0bWw+' }])
    expect(ipc.calls('client_open_document')).toHaveLength(0)
  })

  it('says what the server refused with and opens nothing', async () => {
    const { ipc, stores } = await load(split({ local: false }))
    ipc.on('server_file_read', () => {
      throw { kind: 'outside', message: '/etc/passwd is outside the folders this server lets a client reach' }
    })
    await expect(stores.serverFiles.openReportInBrowser('/etc/passwd')).rejects.toThrow(/outside the folders/)
    expect(ipc.calls('client_open_fetched')).toHaveLength(0)
  })
})

describe('what is sent with a message and what is dropped on a terminal', () => {
  it('hands the composer the stored paths as soon as files are chosen, so a chip names the server\'s copy', async () => {
    const { ipc, stores } = await load(split({ local: false }))
    ipc.on('client_file_read', () => ({ name: 'brief.pdf', bytes: 3, data: 'cG5n' }))
    ipc.on('attachment_write', () => attachment('/srv/attachments/p/brief.pdf'))
    expect((await stores.app.pathsForServer(['/Users/me/brief.pdf'])).paths).toEqual(['/srv/attachments/p/brief.pdf'])
  })

  it('sends a message\'s attachments as given: they were made server paths when they were chosen', async () => {
    const { ipc, stores } = await load(split({ local: false }))
    ipc.on('session_send', null)
    await stores.conversation.sendMessage(7, 'see this', ['/srv/attachments/p/brief.pdf'])
    expect(ipc.calls('session_send')).toEqual([{ id: 7, text: 'see this', attachments: ['/srv/attachments/p/brief.pdf'] }])
    expect(ipc.calls('client_file_read')).toHaveLength(0)
  })

  it('types the stored paths of a dropped file into a remote agent\'s terminal', async () => {
    const { ipc, stores } = await load(split({ local: false }))
    ipc.on('client_file_read', () => ({ name: 'a.png', bytes: 3, data: 'cG5n' }))
    ipc.on('attachment_write', () => attachment('/srv/attachments/p/a.png'))
    ipc.on('terminal_write', null)

    await stores.terminals.sendDroppedPaths(3, ['/Users/me/a.png'], (paths) => paths.join(' '))

    expect(ipc.calls('terminal_write')).toEqual([{ id: 3, data: '/srv/attachments/p/a.png' }])
  })

  it('types nothing and says why when a dropped file cannot be sent', async () => {
    const { ipc, stores } = await load(split({ local: false }))
    ipc.on('client_file_read', () => {
      throw 'a.png is 99999999 bytes; the ceiling is 10485760 bytes'
    })
    ipc.on('terminal_write', null)

    await stores.terminals.sendDroppedPaths(3, ['/Users/me/a.png'], (paths) => paths.join(' '))

    expect(ipc.calls('terminal_write')).toHaveLength(0)
    expect(stores.terminals.terminalState.lastError).toEqual({
      title: 'Could not send the file',
      description: expect.stringContaining('ceiling')
    })
  })
})
