import { describe, expect, it } from 'vitest'
import { promotedNote } from '../../../src/components/run/promotedNote.js'

describe('promotedNote', () => {
  it('names the run start over a card as the source', () => {
    expect(promotedNote('run')).toBe(
      'promoted: человек нажал Run на карточке, и запуск прогона перевёл задачу в ready.'
    )
  })

  it('names the whole-column promote as the source', () => {
    expect(promotedNote('column')).toBe(
      'promoted: человек продвинул колонку Deferred целиком через Promote to ready.'
    )
  })

  it('names a direct status change as the source, covering the card menu and the inspector header alike', () => {
    expect(promotedNote('status')).toBe(
      'promoted: человек перевёл задачу в ready из меню карточки или из заголовка инспектора.'
    )
  })

  it('names an Unblock as the source, covering the card menu and the inspector header alike', () => {
    expect(promotedNote('unblock')).toBe(
      'promoted: человек снял блокировку с задачи из меню карточки или из заголовка инспектора.'
    )
  })

  it('starts every note with the same marker, so a program can find it whichever source it came from', () => {
    expect(promotedNote('run').startsWith('promoted:')).toBe(true)
    expect(promotedNote('column').startsWith('promoted:')).toBe(true)
    expect(promotedNote('status').startsWith('promoted:')).toBe(true)
    expect(promotedNote('unblock').startsWith('promoted:')).toBe(true)
  })

  it('says nothing about a source it has not been told about, rather than guessing at one', () => {
    expect(promotedNote('menu')).toBe('')
    expect(promotedNote(undefined)).toBe('')
    expect(promotedNote('')).toBe('')
  })
})
