/* Состояние редактора на вкладку: документ, каретка, история правок и
   прокрутка. Обычная Map, намеренно вне реактивности Vue — reactive() обернул
   бы EditorState в Proxy, а CodeMirror сравнивает свои объекты по
   идентичности, и подменённый объект сломал бы ему транзакции.

   Прокрутка хранится отдельным числом: EditorState её не содержит — это
   свойство DOM, а не документа.

   Цена решения — третья копия текста в памяти на открытую вкладку (tabs.js
   уже держит text и original). Она принята сознательно: терять историю
   правок на каждом переключении вкладки редактор кода не может. */
const states = new Map()

export function takeState(path) {
  return states.get(path) ?? null
}

export function putState(path, state, scrollTop) {
  states.set(path, { state, scrollTop })
}

/* Уборка идёт от списка вкладок, а не от события закрытия: так одно правило
   покрывает и закрытие вкладки, и переключение проекта, и путь, выпавший из
   списка потому, что файл больше не читается. */
export function keepOnly(paths) {
  const live = new Set(paths)
  for (const path of states.keys()) {
    if (!live.has(path)) states.delete(path)
  }
}
