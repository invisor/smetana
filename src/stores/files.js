/* Файлы проекта во фронте. Четвёртый и последний файл в src/, знающий про
   Tauri, — вместе с tracker.js, settings.js и projects.js. Стор вкладок
   (tabs.js) ходит на диск через него и сам про Tauri не знает.

   Истина здесь снаружи, на диске, как у трекера, — но догонять её нечем:
   вотчера у дерева нет намеренно. Свежесть приносит проход по фокусу окна
   (см. DesktopApp.vue) и кнопка обновления. */
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export const filesState = reactive({
  /* Абсолютный путь активного проекта. Все остальные пути в этом сторе —
     относительные, и разделитель у них всегда "/". */
  root: null,
  /* Путь каталога → { entries, truncated }. Пустая строка — корень.
     Заполняется лениво: каталог появляется здесь только после того, как его
     раскрыли. */
  dirs: new Map(),
  /* Каталоги, которые сейчас читаются. Второе чтение того же каталога не
     начинается: раскрыть-свернуть-раскрыть не должно давать три запроса. */
  loading: new Set(),
  lastError: null
})

export const basenameOf = (path) => path.split('/').filter(Boolean).pop() ?? path

/* Ошибки бэкенда — диагностика: их текст говорит языком файловой системы и
   адресован тому, кто чинит. Человеку показываем короткую фразу, выбранную по
   машинному виду ошибки, а полный текст оставляем в консоли. Тот же приём, что
   в tracker.js. */
const ERRORS = {
  notFound: 'This file is gone from disk.',
  denied: 'No permission to read this file.',
  notAFile: 'This is not a file.',
  binary: 'Binary file — not shown.',
  tooLarge: 'File is too large to open here.',
  notUtf8: 'Not UTF-8 text — not shown.',
  outside: 'That path is outside the project.',
  stale: 'This file changed on disk since it was opened.',
  io: 'Could not read this file.'
}

export function fileErrorText(error) {
  return ERRORS[error?.kind] ?? ERRORS.io
}

/* Те же машинные виды ошибок, но про запись. Отдельная карта, а не общая:
   фраза «No permission to read this file.» после отказавшегося Cmd+S говорит
   не о том, что произошло, и человек ищет причину не там. Ключа `stale` здесь
   нет намеренно — устаревшую метку разбирает своя ветка с кнопками. */
const SAVE_ERRORS = {
  notFound: 'This file is gone from disk — nothing was written.',
  denied: 'No permission to write this file.',
  notAFile: 'This is not a file — nothing was written.',
  outside: 'That path is outside the project — nothing was written.',
  io: 'Could not save this file.'
}

export function saveErrorText(error) {
  return SAVE_ERRORS[error?.kind] ?? SAVE_ERRORS.io
}

/* И третья карта — про каталоги. Отказ чтения каталога человек видит тостом, и
   фраза «This file is gone from disk.» под именем папки говорит не о том, что
   случилось. */
const DIR_ERRORS = {
  notFound: 'This folder is gone from disk.',
  denied: 'No permission to read this folder.',
  notAFile: 'This is not a folder.',
  outside: 'That path is outside the project.',
  io: 'Could not read this folder.'
}

export function dirErrorText(error) {
  return DIR_ERRORS[error?.kind] ?? DIR_ERRORS.io
}

/* Метка строки-заглушки «…N more» в путях дерева. Нулевой байт не бывает в
   имени файла ни на одной файловой системе, поэтому настоящий путь с ним не
   столкнётся. */
const STUB_MARK = '\u0000'

export const isStubPath = (path) => typeof path === 'string' && path.includes(STUB_MARK)

/* Ошибка из Tauri приезжает объектом { kind, message }; ошибка доставки (мок
   бросил Error, IPC не поднялся) — чем угодно. Приводим к одной форме, чтобы
   вызывающие не разбирали два случая. */
function normalize(error) {
  if (error && typeof error === 'object' && typeof error.kind === 'string') return error
  return { kind: 'io', message: String(error?.message ?? error) }
}

/* Отказ чтения каталога виден человеку: дерево в этот момент показывает то,
   что успело прочитаться, и без слов выглядит просто пустой папкой. Полный
   текст ошибки остаётся в консоли, наружу едет короткая фраза — её показывает
   тост в DesktopApp.vue. */
function report(where, error) {
  console.error(`[files] ${where}:`, error)
  filesState.lastError = dirErrorText(error)
}

/* Переезд на другой проект. Дерево сбрасывается целиком: показывать каталоги
   старого проекта под именем нового нельзя ни секунды. */
export function setRoot(path) {
  filesState.root = path
  filesState.dirs = new Map()
  /* Экземпляр не подменяется намеренно: иначе finally уже летящего чтения
     снял бы пометку с чужого, только что начатого запроса на новом Set. */
  filesState.loading.clear()
  filesState.lastError = null
}

export async function listDir(dir = '') {
  if (!filesState.root || filesState.loading.has(dir)) return
  const root = filesState.root
  filesState.loading.add(dir)
  try {
    const listing = await invoke('files_list', { root, dir })
    /* Пока каталог читался, проект могли переключить: ответ относится к
       прошлому корню, и класть его в новое дерево нельзя. Побеждает последний
       переезд, а не последний ответ. */
    if (filesState.root !== root) return
    filesState.dirs.set(listing.dir, {
      entries: listing.entries,
      truncated: listing.truncated
    })
  } catch (err) {
    report(`не удалось прочитать каталог ${dir || '(корень)'}`, normalize(err))
  } finally {
    filesState.loading.delete(dir)
  }
}

/* Перечитывание уже известных каталогов — проход по фокусу окна и кнопка
   обновления. Каталоги, которых в карте нет, не читаются: раскрывать их
   никто не просил. */
export async function refreshDirs(dirs) {
  const known = dirs.filter((dir) => filesState.dirs.has(dir))
  await Promise.all(known.map((dir) => listDir(dir)))
}

export async function readFile(path) {
  try {
    return await invoke('files_read', { root: filesState.root, path })
  } catch (err) {
    const error = normalize(err)
    console.error(`[files] не удалось прочитать ${path}:`, error)
    throw error
  }
}

export async function writeFile(path, text, expectedMtime) {
  try {
    return await invoke('files_write', {
      root: filesState.root,
      path,
      text,
      expectedMtime
    })
  } catch (err) {
    const error = normalize(err)
    console.error(`[files] не удалось записать ${path}:`, error)
    throw error
  }
}

/* Метки времени пачкой. Отказ здесь ничего не значит: проход по фокусу — это
   удобство, и ронять из-за него интерфейс незачем. */
export async function statFiles(paths) {
  if (!filesState.root || !paths.length) return []
  try {
    return await invoke('files_stat', { root: filesState.root, paths })
  } catch (err) {
    console.error('[files] не удалось сверить метки времени:', normalize(err))
    return []
  }
}

/* FileTree ждёт вложенные узлы с children, а стор держит плоскую карту
   каталогов. Собираем дерево на лету и спускаемся только в раскрытые: узел,
   чьих детей ещё не читали, отдаёт children: undefined, и FileTree просто не
   идёт вглубь.

   Обрезанный каталог получает лишнюю запись-заглушку: молчаливая обрезка
   читалась бы как «здесь больше нет файлов». Её kind — "file", потому что
   отдельного вида у строки-заглушки в дереве нет, а её путь помечен нулевым
   байтом: имени файла такой символ не достаётся ни на одной файловой системе,
   и `isStubPath` узнаёт заглушку по нему. Отсеивают её обработчики клика в
   DesktopApp.vue — сама строка ничего про себя не знает. */
export function treeNodes(expandedSet) {
  const build = (dir) => {
    const listing = filesState.dirs.get(dir)
    if (!listing) return undefined
    const nodes = listing.entries.map((entry) => ({
      path: entry.path,
      name: entry.name,
      kind: entry.kind,
      children: entry.kind === 'dir' && expandedSet.has(entry.path) ? build(entry.path) : undefined
    }))
    if (listing.truncated > 0) {
      nodes.push({
        path: `${dir}${STUB_MARK}more`,
        name: `…${listing.truncated} more`,
        kind: 'file'
      })
    }
    return nodes
  }
  return build('') ?? []
}
