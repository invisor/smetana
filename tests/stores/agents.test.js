import { describe, expect, it } from 'vitest'
import { loadStores } from '../support/stores.js'

/* The catalogue that replaced four hand-written lists keyed by agent id.

   What is worth checking here is not that a `ref` holds what it was given: it
   is the three answers the deleted lists used to give and this store now gives
   in their place — what a named harness can do, what an unknown id gets, and
   what a machine that could not be asked at all draws. The last one is the one
   with a decision behind it: an empty catalogue greys every capability row,
   because Rust refuses an unsupported verb with a sentence anyway and a row
   nobody can press is cheaper than a command written into somebody's prompt. */

/* The shape `agents::catalogue` serializes, with Codex's two absences in it —
   this CLI documents no command that clears a conversation and none that prints
   an allowance. `mockBackend.js` answers with the same rows for the browser. */
const CATALOGUE = [
  {
    id: 'claude',
    label: 'Claude Code',
    capabilities: {
      resume: true,
      fork: true,
      clear: true,
      usage: true,
      batch: true,
      oneshot: true
    },
    models: [
      { id: 'fable', label: 'Fable' },
      { id: 'opus', label: 'Opus' }
    ]
  },
  {
    id: 'codex',
    label: 'Codex',
    capabilities: {
      resume: true,
      fork: true,
      clear: false,
      usage: false,
      batch: true,
      oneshot: true
    },
    models: [{ id: 'gpt-5.6-sol', label: 'GPT-5.6-Sol' }]
  }
]

async function loadCatalogue(answer = CATALOGUE) {
  const { stores, ipc } = await loadStores()
  if (answer instanceof Error) ipc.fail('agents_catalog', answer)
  else ipc.on('agents_catalog', answer)
  await stores.agents.initAgents()
  return { agents: stores.agents, ipc }
}

describe('the agent catalogue', () => {
  it('holds one row per harness once it has been read', async () => {
    const { agents } = await loadCatalogue()

    expect(agents.agents.value.map((row) => row.id)).toEqual(['claude', 'codex'])
  })

  /* The store does not memoise, and that is right rather than an oversight:
     `main.js` calls this once before the app mounts, so a second read is a
     caller's decision and not something to be silently refused. What is checked
     is that asking again asks again — a cache here would be a second answer to
     keep in step with the first, for a fact that cannot change while the app
     runs. */
  it('asks again when it is asked again, rather than holding a cached answer', async () => {
    const { agents, ipc } = await loadCatalogue()

    await agents.initAgents()

    expect(ipc.calls('agents_catalog')).toHaveLength(2)
    expect(agents.agents.value.map((row) => row.id)).toEqual(['claude', 'codex'])
  })

  it('answers what a harness can do, and refuses what it cannot', async () => {
    const { agents } = await loadCatalogue()

    expect(agents.can('claude', 'clear')).toBe(true)
    expect(agents.can('codex', 'clear')).toBe(false)
    expect(agents.can('codex', 'resume')).toBe(true)
    expect(agents.can('codex', 'fork')).toBe(true)
    expect(agents.can('codex', 'usage')).toBe(false)
  })

  it('says no about a harness it has never heard of rather than throwing', async () => {
    const { agents } = await loadCatalogue()

    expect(agents.can('nobody', 'resume')).toBe(false)
    expect(agents.can('', 'resume')).toBe(false)
    expect(agents.can(null, 'resume')).toBe(false)
  })

  it('says no about a verb it has never heard of', async () => {
    const { agents } = await loadCatalogue()

    expect(agents.can('claude', 'teleport')).toBe(false)
  })

  it('names a harness as a person reads it, and an unknown one as it stands', async () => {
    const { agents } = await loadCatalogue()

    expect(agents.agentLabel('claude')).toBe('Claude Code')
    expect(agents.agentLabel('codex')).toBe('Codex')
    expect(agents.agentLabel('somebody-elses-cli')).toBe('somebody-elses-cli')
  })

  it('answers with the models a harness offers, in the order Rust offered them', async () => {
    /* The order is each profile's own `MODELS` — strongest first — and not
       this store's to sort: a dropdown that reordered them would put a
       different model under the same cursor position on two machines. */
    const { agents } = await loadCatalogue()

    expect(agents.modelsOf('claude')).toEqual([
      { id: 'fable', label: 'Fable' },
      { id: 'opus', label: 'Opus' }
    ])
    expect(agents.modelsOf('codex')).toEqual([{ id: 'gpt-5.6-sol', label: 'GPT-5.6-Sol' }])
  })

  it('offers no model at all for a harness it has never heard of', async () => {
    /* Including the empty string, which is what a role with nothing chosen
       holds: the settings window asks about that id on every render. */
    const { agents } = await loadCatalogue()

    expect(agents.modelsOf('somebody-elses-cli')).toEqual([])
    expect(agents.modelsOf('')).toEqual([])
    expect(agents.modelsOf(null)).toEqual([])
  })

  it('leaves every row greyed when the read did not work', async () => {
    const { agents } = await loadCatalogue(new Error('mockBackend: no catalogue here'))

    expect(agents.agents.value).toEqual([])
    expect(agents.can('claude', 'resume')).toBe(false)
    expect(agents.agentLabel('claude')).toBe('claude')
    expect(agents.modelsOf('claude')).toEqual([])
  })

  /* A worker older than this command, or one that answered something else
     entirely: an empty list is the same safe direction a refusal takes, and the
     alternative is every menu in the app throwing on a `.find`. */
  it('takes an answer that is not a list as no catalogue at all', async () => {
    const { agents } = await loadCatalogue({ claude: true })

    expect(agents.agents.value).toEqual([])
    expect(agents.can('claude', 'resume')).toBe(false)
  })
})
