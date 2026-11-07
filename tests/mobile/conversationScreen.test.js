import { createApp, h, nextTick } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'

vi.mock('../../src/components/conversation/ConversationView.vue', () => ({
  default: { props: ['sessionId'], template: '<div data-conversation></div>' }
}))
vi.mock('../../src/stores/conversation.js', async () => {
  const { ref } = await import('vue')
  return { drivenSessions: ref([]), crewAgents: ref([]) }
})
vi.mock('../../src/stores/terminals.js', () => ({ terminalState: { sessions: [] }, toUiState: () => null }))
vi.mock('../../mobile/src/bridge/core.js', () => ({ invoke: vi.fn() }))

import ConversationScreen from '../../mobile/src/screens/ConversationScreen.vue'
import { crewAgents } from '../../src/stores/conversation.js'

let app
let host
afterEach(() => {
  app?.unmount()
  host?.remove()
  app = null
  host = null
})

it('keeps a phone Crew conversation open while present and closes it when the tree removes it', async () => {
  crewAgents.value = [{ id: 'crew:41:41', state: 'needs-you' }]
  let closed = 0
  host = document.createElement('div')
  document.body.append(host)
  app = createApp({ render: () => h(ConversationScreen, {
    id: 'crew:41:41', driven: true, onClose: () => { closed++ }
  }) })
  app.mount(host)
  await nextTick()
  expect(host.querySelector('[data-conversation]')).not.toBeNull()
  expect(closed).toBe(0)

  crewAgents.value = []
  await nextTick()
  expect(closed).toBe(1)
})
