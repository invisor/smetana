/* Состояние трекера во фронте. Компоненты знают только это хранилище;
   про Tauri знает лишь оно само. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/* bd и дизайн-система называют одно и то же по-разному. RESERVED в status.js —
   ready/running/done, в bd — open/in_progress/closed. Пересечение только по
   blocked. Без перевода карточки потеряли бы глифы и уехали бы в
   генерируемые хэш-цвета. Всё остальное, включая кастомные статусы,
   уходит в normalizeStatus как есть — это и есть задуманное поведение. */
const UI_STATUS = { open: 'ready', in_progress: 'running', closed: 'done' }

export const toUiStatus = (name) => UI_STATUS[name] ?? name

export const trackerState = reactive({
  ready: false,
  generation: 0,
  columns: [],
  issues: new Map(),
  health: { state: 'ok' },
  lastError: null
})

export const issueById = (id) => trackerState.issues.get(id)

/* Родство в bd выражено зависимостью parent-child, и она попадает в
   dependency_count. Считать блокировки по счётчикам нельзя — у каждой
   дочерней задачи появилось бы ложное "заблокировано 1". Считаем по
   рёбрам с типом blocks; bd отдаёт только исходящие, поэтому обратную
   сторону собираем сами. */
const dependencyCounts = computed(() => {
  const blockedBy = new Map()
  const blocks = new Map()
  for (const issue of trackerState.issues.values()) {
    const edges = (issue.dependencies ?? []).filter((d) => d.type === 'blocks')
    if (edges.length) blockedBy.set(issue.id, edges.length)
    for (const edge of edges) {
      blocks.set(edge.depends_on_id, (blocks.get(edge.depends_on_id) ?? 0) + 1)
    }
  }
  return { blockedBy, blocks }
})

export const boardColumns = computed(() => {
  const { blockedBy, blocks } = dependencyCounts.value
  const buckets = new Map(trackerState.columns.map((c) => [c.name, []]))

  for (const issue of trackerState.issues.values()) {
    // Статус, которого нет в наборе bd, всё равно должен быть виден.
    if (!buckets.has(issue.status)) buckets.set(issue.status, [])
    buckets.get(issue.status).push({
      id: issue.id,
      title: issue.title,
      status: toUiStatus(issue.status),
      blockedBy: blockedBy.get(issue.id) ?? 0,
      blocks: blocks.get(issue.id) ?? 0,
      spawnedFrom: issue.parent ?? undefined
    })
  }

  return [...buckets].map(([name, tasks]) => ({ status: toUiStatus(name), tasks }))
})

function applyDelta(delta) {
  if (delta.columns) trackerState.columns = delta.columns
  for (const issue of delta.upserted) trackerState.issues.set(issue.id, issue)
  for (const id of delta.removed) trackerState.issues.delete(id)
  trackerState.generation = delta.generation
}

/* Ошибки бэкенда — диагностика: их текст говорит языком bd и адресован тому,
   кто чинит, а не тому, кто работает. Пользователю показываем короткое
   объяснение, что именно не получилось, полный текст оставляем в консоли.
   Заодно чтение и запись перестали делить одну подпись: read-ошибка под
   заголовком "не удалось записать" врала о происходящем. */
const ERRORS = {
  read: {
    title: 'Could not read the tracker',
    description: 'The board may be out of date. It will catch up on the next change.'
  },
  write: {
    title: 'Could not save to the tracker',
    description: 'Nothing was written. The board shows what the tracker has.'
  }
}

function report(kind, error) {
  console.error(`[tracker] ${kind} failed:`, error)
  trackerState.lastError = ERRORS[kind]
}

function applySnapshot(snapshot) {
  trackerState.columns = snapshot.columns
  trackerState.issues.clear()
  for (const issue of snapshot.issues) trackerState.issues.set(issue.id, issue)
  trackerState.generation = snapshot.generation
  trackerState.ready = true
}

/* tracker_resync теперь может отклониться (bd упал) — в этом случае нельзя
   считать это успехом и стирать состояние: оставляем доску как была и
   запоминаем ошибку. */
export async function resync() {
  try {
    applySnapshot(await invoke('tracker_resync'))
    trackerState.lastError = null
  } catch (err) {
    report('read', err)
  }
}

/* message в health — диагностика: она по-русски и говорит языком bd. В
   интерфейс идёт короткий текст по одному лишь state, а подробность остаётся
   там, где её ищут при отладке. */
function setHealth(health) {
  trackerState.health = health
  if (health.state !== 'ok') console.warn('[tracker] health:', health.state, health.message ?? '')
}

export async function initTracker() {
  await listen('tracker:health', (event) => {
    setHealth(event.payload)
  })
  /* Поколение растёт на единицу с каждой дельтой. Разрыв означает, что
     событие потеряно — берём снимок целиком. */
  await listen('tracker:delta', (event) => {
    const delta = event.payload
    if (trackerState.ready && delta.generation > trackerState.generation + 1) {
      resync()
      return
    }
    applyDelta(delta)
  })

  /* tracker:health уходит за микросекунды после старта — раньше, чем веб-вью
     успевает подписаться. Слушатель выше ловит всё, что случится после;
     эта команда — единственный способ узнать состояние, отправленное до
     подписки. */
  try {
    setHealth(await invoke('tracker_health'))
  } catch (err) {
    report('read', err)
  }

  /* Снимок и дельты идут разными путями: пока ответ команды летит обратно в
     веб-вью, вотчер успевает прислать дельту и продвинуть поколение. Тогда
     снимок — это прошлое, и раскатывать его нельзя. Он вернул бы старые
     значения поверх новых, потерял бы удаления, которые дельта уже
     применила, и откатил бы счётчик поколений назад; следующая дельта
     обычно чинит это разрывом нумерации, но если трекер затих, её не будет,
     а в Rust уже лежит новое значение — его полная сверка не увидит
     расхождения и ничего не пришлёт. Доска осталась бы неправильной молча.
     Поэтому устаревший снимок игнорируем целиком, а свежий заменяет
     состояние так же, как в resync(): с очисткой, иначе те же удаления
     потеряются. */
  const snapshot = await invoke('tracker_snapshot')
  if (snapshot.generation >= trackerState.generation) applySnapshot(snapshot)
  trackerState.ready = true
}

/* Записи в Map заменяются целиком, а не мутируются, поэтому сравнение по
   ссылке ничего не говорит о содержимом — только JSON-по-значению отличает
   "то же самое" от "кто-то успел изменить". */
function sameIssue(a, b) {
  return JSON.stringify(a) === JSON.stringify(b)
}

/* Запись занимает около двух секунд. Оптимистичное значение применяется
   сразу — пользователь видит свою правку без ожидания, и это единственная
   индикация происходящего: отдельной пометки "в полёте" на карточке нет и не
   должно быть, цвет в этой системе принадлежит статусу, а не факту записи.

   Если запись упала, откатывать нужно с оглядкой: за эти две секунды карточку
   мог обновить вотчер или другая запись. Откатываем, только если текущее
   значение всё ещё равно тому, что записал именно этот вызов, — сравнение по
   значению, а не по ссылке. Если оно уже другое, чужие изменения важнее
   нашего отката, и мы просто запоминаем ошибку. */
async function write(id, optimistic, run) {
  const before = id ? trackerState.issues.get(id) : null
  const optimisticValue = before && optimistic ? { ...before, ...optimistic } : null
  if (optimisticValue) trackerState.issues.set(id, optimisticValue)
  trackerState.lastError = null

  try {
    const issue = await run()
    trackerState.issues.set(issue.id, issue)
    return issue
  } catch (error) {
    if (optimisticValue && sameIssue(trackerState.issues.get(id), optimisticValue)) {
      trackerState.issues.set(id, before)
    }
    report('write', error)
    throw error
  }
}

export function createIssue(issue) {
  return write(null, null, () => invoke('tracker_create', { issue }))
}

export function updateIssue(id, patch) {
  const optimistic = {}
  if (patch.title !== undefined) optimistic.title = patch.title
  if (patch.status !== undefined) optimistic.status = patch.status
  if (patch.priority !== undefined) optimistic.priority = patch.priority
  if (patch.assignee !== undefined) optimistic.assignee = patch.assignee
  return write(id, optimistic, () => invoke('tracker_update', { id, patch }))
}

export function closeIssue(id, reason = null) {
  return write(id, { status: 'closed' }, () => invoke('tracker_close', { id, reason }))
}

export function reopenIssue(id) {
  return write(id, { status: 'open' }, () => invoke('tracker_reopen', { id }))
}
