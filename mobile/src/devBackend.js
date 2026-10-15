/* The page in a browser: no Tauri, no core, no desktop. `npm run dev` shows the working screens
   over the desktop's own mock backend (src/stores/mockBackend.js), which is what the screens are
   checked against by eye (CLAUDE.md, Commands).

   The mock answers the commands by their own names; the phone's stores ask `bridge_invoke`. This
   puts the one in front of the other, and stands in for the three things the Rust core says
   about itself (`bridge_state`, `bridge_listen`, `take_push_target`). It also refuses what the
   core would refuse: a command off the allowlist rejects here as `forbidden`, with a warning, so
   a screen that reached for one fails in the browser exactly as it would on the phone.

   Nothing here is in the production build: main.js imports this file only under
   `import.meta.env.DEV`, which the bundler removes.

   Query parameters, beside the theme and density the app already reads: `target` is a push
   notification's target (`run:/path`, `session:1`, `terminal:1`) as if it had been tapped, and
   `window.__phone.emit(name, payload)` delivers an event from the "desktop". */
import { emit } from '@tauri-apps/api/event'
import { installMockBackend } from '@src/stores/mockBackend.js'
import { isAllowedCommand } from './allowed.js'

export function installDevBackend() {
  if (!installMockBackend()) return false
  const internals = window.__TAURI_INTERNALS__
  const mock = internals.invoke.bind(internals)
  const target = new URLSearchParams(window.location.search).get('target')

  internals.invoke = (cmd, args, options) => {
    switch (cmd) {
      case 'bridge_state':
        return Promise.resolve({ paired: true, version: 'dev', status: { state: 'online' } })
      case 'bridge_listen':
      case 'bridge_unlisten':
        return Promise.resolve(null)
      case 'take_push_target':
        return Promise.resolve(target)
      case 'bridge_invoke':
        if (!isAllowedCommand(args.cmd)) {
          console.warn(`[devBackend] "${args.cmd}" is not on the allowlist; the core would refuse it`)
          return Promise.reject('forbidden')
        }
        return mock(args.cmd, args.args, options)
      default:
        return mock(cmd, args, options)
    }
  }

  window.__phone = { emit: (name, payload) => emit('bridge:event', { name, payload }) }
  return true
}
