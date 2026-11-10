import { createApp, h, nextTick, ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import Toast from '../../../src/components/overlays/Toast.vue'

let app
let host

function mount(items) {
  host = document.createElement('div')
  document.body.append(host)
  app = createApp({
    render: () => items.value.map((item) => h(Toast, {
      key: item.id,
      title: item.title,
      event: item.event,
      onClose: item.onClose,
      onExpired: item.onExpired
    }))
  })
  app.mount(host)
}

const titles = () => [...host.querySelectorAll('[role="status"]')].map((node) => node.textContent.trim())

beforeEach(() => vi.useFakeTimers())
afterEach(() => {
  app?.unmount()
  host?.remove()
  app = null
  host = null
  vi.useRealTimers()
})

describe('corner toast lifetime', () => {
  it('stays visible through 9999 ms, hides at 10000 ms even while hovered', async () => {
    const expired = vi.fn()
    const items = ref([{ id: 'one', title: 'Failed', event: 1, onExpired: expired }])
    mount(items)
    host.querySelector('[role="status"]').dispatchEvent(new MouseEvent('mouseenter'))
    await vi.advanceTimersByTimeAsync(9999)
    await nextTick()
    expect(titles()).toEqual(['Failed'])
    await vi.advanceTimersByTimeAsync(1)
    await nextTick()
    expect(titles()).toEqual([])
    expect(expired).toHaveBeenCalledOnce()
  })

  it('uses independent clocks and gives a replacement a full interval', async () => {
    const items = ref([{ id: 'one', title: 'First', event: 1 }])
    mount(items)
    await vi.advanceTimersByTimeAsync(5000)
    items.value = [...items.value, { id: 'two', title: 'Second', event: 1 }]
    await nextTick()
    await vi.advanceTimersByTimeAsync(5000)
    await nextTick()
    expect(titles()).toEqual(['Second'])
    items.value = [items.value[0], { id: 'two', title: 'Replacement', event: 2 }]
    await nextTick()
    await vi.advanceTimersByTimeAsync(9999)
    await nextTick()
    expect(titles()).toEqual(['Replacement'])
    await vi.advanceTimersByTimeAsync(1)
    await nextTick()
    expect(titles()).toEqual([])
  })

  it('reopens on a new event with identical text and clears timers on close and unmount', async () => {
    const close = vi.fn()
    const items = ref([{ id: 'one', title: 'Same error', event: 1, onClose: close }])
    mount(items)
    await vi.advanceTimersByTimeAsync(10000)
    await nextTick()
    expect(titles()).toEqual([])
    items.value = [{ ...items.value[0], event: 2 }]
    await nextTick()
    expect(titles()).toEqual(['Same error'])
    expect(vi.getTimerCount()).toBe(1)
    host.querySelector('[aria-label="Dismiss"]').click()
    await nextTick()
    expect(close).toHaveBeenCalledOnce()
    expect(titles()).toEqual([])
    expect(vi.getTimerCount()).toBe(0)
    items.value = [...items.value, { id: 'two', title: 'Other', event: 1 }]
    await nextTick()
    expect(vi.getTimerCount()).toBe(1)
    app.unmount()
    app = null
    expect(vi.getTimerCount()).toBe(0)
  })
})
