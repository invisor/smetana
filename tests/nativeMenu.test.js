import { afterEach, describe, expect, it } from 'vitest'
import { suppressNativeMenus } from '../src/nativeMenu.js'

let stop = null

afterEach(() => {
  stop?.()
  stop = null
  document.body.innerHTML = ''
})

const rightClick = (el) => {
  const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
  el.dispatchEvent(event)
  return event
}

const nested = () => {
  const row = document.createElement('div')
  const label = document.createElement('span')
  row.append(label)
  document.body.append(row)
  return { row, label }
}

describe('suppressNativeMenus', () => {
  it('refuses the event wherever in the document it started', () => {
    stop = suppressNativeMenus(document)
    const { label } = nested()
    expect(rightClick(label).defaultPrevented).toBe(true)
  })

  /* The reason it listens in the capture phase. A component that stopped the
     event on its own row would otherwise hand the platform's menu back on the
     one row that has a menu of its own. */
  it('refuses it even where a component stops the event on the way up', () => {
    stop = suppressNativeMenus(document)
    const { row, label } = nested()
    row.addEventListener('contextmenu', (event) => event.stopPropagation())
    expect(rightClick(label).defaultPrevented).toBe(true)
  })

  /* Preventing a default is not stopping a propagation: the rows that draw a
     menu of their own still hear the click. */
  it('leaves a component’s own menu handler to run', () => {
    stop = suppressNativeMenus(document)
    const { row, label } = nested()
    let opened = 0
    row.addEventListener('contextmenu', () => {
      opened += 1
    })
    rightClick(label)
    expect(opened).toBe(1)
  })

  it('hands the platform’s menu back when it is undone', () => {
    suppressNativeMenus(document)()
    const { label } = nested()
    expect(rightClick(label).defaultPrevented).toBe(false)
  })
})
