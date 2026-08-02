import { beforeEach, describe, expect, it, vi } from 'vitest'
import { loadStores } from '../support/stores.js'
import { delta, edge, issue, snapshot } from '../support/fixtures.js'

let ipc
let emit
let tracker

beforeEach(async () => {
  const loaded = await loadStores()
  ipc = loaded.ipc
  emit = loaded.emit
  tracker = loaded.stores.tracker
})

/* Пустить трекер с заданным снимком. Возвращает управление, когда initTracker
   отработал целиком. */
const start = async (snap = snapshot()) => {
  ipc.on('tracker_health', { state: 'ok' })
  ipc.on('tracker_snapshot', snap)
  await tracker.initTracker()
}

describe('перевод статусов', () => {
  it('три статуса bd превращаются в статусы дизайн-системы', () => {
    expect(tracker.toUiStatus('open')).toBe('ready')
    expect(tracker.toUiStatus('in_progress')).toBe('running')
    expect(tracker.toUiStatus('closed')).toBe('done')
  })

  it('всё остальное проходит насквозь — это и есть задуманное', () => {
    expect(tracker.toUiStatus('blocked')).toBe('blocked')
    expect(tracker.toUiStatus('awaiting-review')).toBe('awaiting-review')
  })
})

describe('boardColumns', () => {
  it('раскладывает задачи по колонкам снимка', async () => {
    await start(
      snapshot({
        issues: [issue({ id: 'bd-1' }), issue({ id: 'bd-2', status: 'in_progress' })]
      })
    )

    const board = tracker.boardColumns.value

    expect(board.map((column) => column.status)).toEqual(['ready', 'running', 'done'])
    expect(board[0].tasks.map((task) => task.id)).toEqual(['bd-1'])
    expect(board[1].tasks.map((task) => task.id)).toEqual(['bd-2'])
  })

  it('статус, которого нет в наборе bd, получает свою колонку, а не пропадает', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-9', status: 'awaiting-review' })] }))

    const board = tracker.boardColumns.value

    expect(board.map((column) => column.status)).toContain('awaiting-review')
  })

  it('считает блокировки по рёбрам с обеих сторон', async () => {
    await start(
      snapshot({
        issues: [
          issue({ id: 'bd-1' }),
          issue({ id: 'bd-2', dependencies: [edge({ issue_id: 'bd-2', depends_on_id: 'bd-1' })] })
        ]
      })
    )

    const tasks = tracker.boardColumns.value.flatMap((column) => column.tasks)
    const first = tasks.find((task) => task.id === 'bd-1')
    const second = tasks.find((task) => task.id === 'bd-2')

    expect(second.blockedBy).toBe(1)
    expect(first.blocks).toBe(1)
    expect(first.blockedBy).toBe(0)
  })

  it('родство не считается блокировкой: иначе у каждой дочерней было бы ложное «заблокировано»', async () => {
    await start(
      snapshot({
        issues: [
          issue({
            id: 'bd-2',
            parent: 'bd-1',
            dependencies: [edge({ issue_id: 'bd-2', depends_on_id: 'bd-1', type: 'parent-child' })]
          })
        ]
      })
    )

    const task = tracker.boardColumns.value.flatMap((column) => column.tasks)[0]

    expect(task.blockedBy).toBe(0)
    expect(task.spawnedFrom).toBe('bd-1')
  })
})

describe('дельты', () => {
  it('добавляют и удаляют задачи и двигают поколение', async () => {
    await start(snapshot({ generation: 5, issues: [issue({ id: 'bd-1' })] }))

    await emit('tracker:delta', delta({
      generation: 6,
      upserted: [issue({ id: 'bd-2' })],
      removed: ['bd-1']
    }))

    expect(tracker.trackerState.issues.has('bd-1')).toBe(false)
    expect(tracker.trackerState.issues.has('bd-2')).toBe(true)
    expect(tracker.trackerState.generation).toBe(6)
  })

  it('разрыв поколения означает потерянное событие — доска берётся целиком через resync', async () => {
    await start(snapshot({ generation: 5, issues: [issue({ id: 'bd-1' })] }))
    ipc.on('tracker_resync', snapshot({ generation: 9, issues: [issue({ id: 'bd-9' })] }))

    await emit('tracker:delta', delta({ generation: 8, upserted: [issue({ id: 'bd-8' })] }))

    await vi.waitFor(() => expect(ipc.calls('tracker_resync')).toHaveLength(1))
    /* Снимок заменяет состояние целиком: прежняя задача ушла, пришедшая с ним — на месте. */
    expect(tracker.trackerState.generation).toBe(9)
    expect(tracker.trackerState.issues.has('bd-9')).toBe(true)
    expect(tracker.trackerState.issues.has('bd-1')).toBe(false)
    /* Дельта с разрывом не применяется вовсе — её задача до доски не доехала. */
    expect(tracker.trackerState.issues.has('bd-8')).toBe(false)
  })

  it('во время переезда дельты игнорируются: они могут быть про старый каталог', async () => {
    await start(snapshot({ generation: 5 }))
    tracker.trackerState.switching = true

    await emit('tracker:delta', delta({ generation: 6, upserted: [issue({ id: 'bd-6' })] }))

    expect(tracker.trackerState.issues.has('bd-6')).toBe(false)
    expect(tracker.trackerState.generation).toBe(5)
  })

  it('снимок, устаревший за время полёта, не раскатывается', async () => {
    ipc.on('tracker_health', { state: 'ok' })
    /* Пока ответ на tracker_snapshot летит обратно, вотчер успевает прислать
       дельту и продвинуть поколение. Снимок в этот момент — прошлое. */
    ipc.on('tracker_snapshot', async () => {
      await emit('tracker:delta', delta({ generation: 9, upserted: [issue({ id: 'bd-9' })] }))
      return snapshot({ generation: 5, issues: [issue({ id: 'bd-старая' })] })
    })

    await tracker.initTracker()

    expect(tracker.trackerState.generation).toBe(9)
    expect(tracker.trackerState.issues.has('bd-9')).toBe(true)
    expect(tracker.trackerState.issues.has('bd-старая')).toBe(false)
  })
})

describe('resync и смена проекта', () => {
  it('resync заменяет состояние целиком, а не дополняет его', async () => {
    await start(snapshot({ generation: 5, issues: [issue({ id: 'bd-1' })] }))
    ipc.on('tracker_resync', snapshot({ generation: 7, issues: [issue({ id: 'bd-2' })] }))

    await tracker.resync()

    expect(tracker.trackerState.issues.has('bd-1')).toBe(false)
    expect(tracker.trackerState.generation).toBe(7)
  })

  it('упавший resync оставляет доску как была и запоминает ошибку чтения', async () => {
    await start(snapshot({ generation: 5, issues: [issue({ id: 'bd-1' })] }))
    ipc.fail('tracker_resync', new Error('bd не запустился'))

    await tracker.resync()

    expect(tracker.trackerState.issues.has('bd-1')).toBe(true)
    expect(tracker.trackerState.lastError.title).toBe('Could not read the tracker')
  })

  it('setProject снимает признак переезда даже после отказа', async () => {
    await start()
    ipc.fail('tracker_set_project', new Error('каталога нет'))

    await tracker.setProject('/другой')

    expect(tracker.trackerState.switching).toBe(false)
    expect(tracker.trackerState.lastError.title).toBe('Could not read the tracker')
  })
})

describe('запись', () => {
  it('оптимистичное значение видно сразу, до ответа бэкенда', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', title: 'старое' })] }))
    ipc.on('tracker_update', () => issue({ id: 'bd-1', title: 'новое' }))

    const pending = tracker.updateIssue('bd-1', { title: 'новое' })
    expect(tracker.trackerState.issues.get('bd-1').title).toBe('новое')

    await pending
    expect(tracker.trackerState.issues.get('bd-1').title).toBe('новое')
  })

  it('отказ откатывает правку, если её никто не трогал', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', title: 'старое' })] }))
    ipc.fail('tracker_update', new Error('bd упал'))

    await expect(tracker.updateIssue('bd-1', { title: 'новое' })).rejects.toThrow()

    expect(tracker.trackerState.issues.get('bd-1').title).toBe('старое')
    expect(tracker.trackerState.lastError.title).toBe('Could not save to the tracker')
  })

  it('отказ не откатывает, если за время полёта значение изменил кто-то ещё', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1', title: 'старое' })] }))
    ipc.on('tracker_update', () => {
      /* Вотчер принёс чужую правку, пока наша запись была в полёте. */
      tracker.trackerState.issues.set('bd-1', issue({ id: 'bd-1', title: 'чужое' }))
      throw new Error('bd упал')
    })

    await expect(tracker.updateIssue('bd-1', { title: 'новое' })).rejects.toThrow()

    expect(tracker.trackerState.issues.get('bd-1').title).toBe('чужое')
  })

  it('close и reopen шлют свои команды и оптимистично двигают статус', async () => {
    await start(snapshot({ issues: [issue({ id: 'bd-1' })] }))
    ipc.on('tracker_close', () => issue({ id: 'bd-1', status: 'closed' }))
    ipc.on('tracker_reopen', () => issue({ id: 'bd-1', status: 'open' }))

    await tracker.closeIssue('bd-1', 'сделано')
    expect(ipc.calls('tracker_close')).toEqual([{ id: 'bd-1', reason: 'сделано' }])
    expect(tracker.trackerState.issues.get('bd-1').status).toBe('closed')

    await tracker.reopenIssue('bd-1')
    expect(tracker.trackerState.issues.get('bd-1').status).toBe('open')
  })
})

describe('здоровье и проверка каталогов', () => {
  it('health приходит и событием, и ответом команды', async () => {
    ipc.on('tracker_health', { state: 'not-a-beads-repo' })
    ipc.on('tracker_snapshot', snapshot())

    await tracker.initTracker()
    expect(tracker.trackerState.health.state).toBe('not-a-beads-repo')

    await emit('tracker:health', { state: 'ok' })
    expect(tracker.trackerState.health.state).toBe('ok')
  })

  it('упавшая проверка каталогов считает их отслеживаемыми: предупреждение хуже молчания', async () => {
    ipc.fail('tracker_probe', new Error('не смогли'))

    await expect(tracker.probeProjects(['/a', '/b'])).resolves.toEqual([
      { path: '/a', tracked: true },
      { path: '/b', tracked: true }
    ])
  })
})
