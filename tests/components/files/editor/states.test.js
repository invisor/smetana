import { beforeEach, describe, expect, it, vi } from 'vitest'

/* Модуль держит обычную Map на уровне модуля — между тестами её надо
   пересоздавать, иначе записи предыдущего теста доживут до следующего. */
let states

beforeEach(async () => {
  vi.resetModules()
  states = await import('../../../../src/components/files/editor/states.js')
})

describe('кэш состояний редактора', () => {
  it('неизвестный путь даёт null, а не undefined', () => {
    expect(states.peekState('нет-такого.txt')).toBe(null)
  })

  it('положенное состояние читается вместе с прокруткой', () => {
    const state = { фиктивное: 'состояние' }
    states.putState('a.txt', state, 120)

    expect(states.peekState('a.txt')).toEqual({ state, scrollTop: 120 })
  })

  it('чтение не удаляет запись: за одно переключение её читают дважды', () => {
    states.putState('a.txt', { раз: 1 }, 0)

    states.peekState('a.txt')
    expect(states.peekState('a.txt')).not.toBe(null)
  })

  it('повторная запись заменяет прежнюю', () => {
    states.putState('a.txt', { раз: 1 }, 10)
    states.putState('a.txt', { два: 2 }, 20)

    expect(states.peekState('a.txt')).toEqual({ state: { два: 2 }, scrollTop: 20 })
  })

  it('keepOnly выбрасывает пути вне списка и сохраняет живые', () => {
    states.putState('a.txt', { a: 1 }, 0)
    states.putState('b.txt', { b: 2 }, 0)
    states.putState('c.txt', { c: 3 }, 0)

    states.keepOnly(['a.txt', 'c.txt'])

    expect(states.peekState('a.txt')).not.toBe(null)
    expect(states.peekState('b.txt')).toBe(null)
    expect(states.peekState('c.txt')).not.toBe(null)
  })

  it('пустой список чистит всё', () => {
    states.putState('a.txt', { a: 1 }, 0)
    states.keepOnly([])

    expect(states.peekState('a.txt')).toBe(null)
  })
})
