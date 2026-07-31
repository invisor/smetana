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
  inflight: new Set(),
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
      spawnedFrom: issue.parent ?? undefined,
      state: trackerState.inflight.has(issue.id) ? 'changed' : 'default'
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

/* tracker_resync теперь может отклониться (bd упал) — в этом случае нельзя
   считать это успехом и стирать состояние: оставляем доску как была и
   запоминаем ошибку. */
export async function resync() {
  try {
    const snapshot = await invoke('tracker_resync')
    trackerState.columns = snapshot.columns
    trackerState.issues.clear()
    for (const issue of snapshot.issues) trackerState.issues.set(issue.id, issue)
    trackerState.generation = snapshot.generation
    trackerState.ready = true
    trackerState.lastError = null
  } catch (err) {
    trackerState.lastError = err
  }
}

export async function initTracker() {
  await listen('tracker:health', (event) => {
    trackerState.health = event.payload
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
    trackerState.health = await invoke('tracker_health')
  } catch (err) {
    trackerState.lastError = err
  }

  const snapshot = await invoke('tracker_snapshot')
  trackerState.columns = snapshot.columns
  for (const issue of snapshot.issues) trackerState.issues.set(issue.id, issue)
  trackerState.generation = snapshot.generation
  trackerState.ready = true
}
