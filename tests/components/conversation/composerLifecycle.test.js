import { afterEach, describe, expect, it } from 'vitest'
import { createApp, h, nextTick, reactive } from 'vue'
import Composer from '../../../src/components/conversation/Composer.vue'
import { composerBusy } from '../../../src/components/conversation/journal.js'

describe('the shared Send and Stop control', () => {
  const mounted = []
  afterEach(() => {
    mounted.splice(0).forEach(({ app, host }) => {
      app.unmount()
      host.remove()
    })
  })

  function mount() {
    const state = reactive({ session: 'running', question: null, draft: [{ kind: 'text', text: 'Reply' }] })
    const actions = []
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp({
      render: () => h(Composer, {
        modelValue: state.draft,
        busy: composerBusy(state.session, state.question),
        onSend: () => actions.push('send'),
        onStop: () => actions.push('stop'),
        'onUpdate:modelValue': (draft) => { state.draft = draft }
      })
    })
    app.mount(host)
    mounted.push({ app, host })
    return { state, actions, host }
  }

  it('sends a reply to completed prose through click and Enter', async () => {
    const { state, actions, host } = mount()
    expect(host.textContent).toContain('Stop')
    state.session = 'needs-you'
    await nextTick()
    expect(host.textContent).toContain('Send')
    const send = [...host.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Send')
    send.click()
    expect(actions).toEqual(['send'])
    host.querySelector('textarea').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    expect(actions).toEqual(['send', 'send'])
  })

  it('keeps an active turn on Stop and does not send on Enter', async () => {
    const { actions, host } = mount()
    host.querySelector('textarea').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    expect(actions).toEqual([])
    const stop = [...host.querySelectorAll('button')].find((button) => button.textContent.trim() === 'Stop')
    stop.click()
    expect(actions).toEqual(['stop'])
  })
})
