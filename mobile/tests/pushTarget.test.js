import { describe, expect, it, vi } from 'vitest'
import { installCore } from './support/bridge.js'

const load = async () => {
  vi.resetModules()
  const [event, mod] = await Promise.all([import('@tauri-apps/api/event'), import('../src/pushTarget.js')])
  return { emit: event.emit, ...mod }
}

describe('the notification target', () => {
  it('takes the target of a tap that started the app', async () => {
    installCore({ take_push_target: 'run/12' })
    const { initPushTarget, pushTarget } = await load()
    await initPushTarget()
    expect(pushTarget.value).toBe('run/12')
  })

  it('holds the latest target of a tap while the app runs', async () => {
    installCore({ take_push_target: null })
    const { initPushTarget, pushTarget, emit, clearPushTarget } = await load()
    await initPushTarget()
    expect(pushTarget.value).toBeNull()
    await emit('push:target', 'agent/3')
    await emit('push:target', 'run/9')
    expect(pushTarget.value).toBe('run/9')
    clearPushTarget()
    expect(pushTarget.value).toBeNull()
  })
})
