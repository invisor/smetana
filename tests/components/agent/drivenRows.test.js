import { describe, expect, it } from 'vitest'
import {
  DRIVEN_PREFIX,
  drivenAgentRow,
  drivenRowId,
  drivenSessionOf,
  mergeAgentCounts,
  mergeAgentRows,
  mergeLiveAgentCount,
  mergeProjectStates
} from '../../../src/components/agent/drivenRows.js'
import { agentKey } from '../../../src/components/agent/agentOrder.js'

/* A PTY session as `agentRows` builds one: the worker's counter for an id and
   the conversation id the app chose at the spawn. */
const live = (id, conversation) => ({ id, conversation, state: 'running' })
/* And one off `.smetana/agents.json`, whose id *is* its conversation id. These
   are the tail of that list, and where they sit is what the merge has to keep. */
const offline = (id) => ({ id, conversation: id, state: 'done', restored: true })
/* A driven session as the view hands it over: already in this design system's
   words, with the elapsed time already worked out. */
const driven = (id, state = 'running', project = '/p') => ({ id, project, state })

describe('a driven conversation among the agents', () => {
  describe('the id a row is known by', () => {
    it('carries a prefix, so two workers counting from 1 cannot collide', () => {
      expect(drivenRowId(1)).toBe(`${DRIVEN_PREFIX}1`)
      /* The point of the prefix, said as the panel asks it: `agentKey` falls
         through to the id for a row with no conversation, and both a PTY
         session and a driven one can be session 1 in the same window. */
      expect(agentKey(drivenAgentRow({ id: 1, state: 'running', elapsed: '1m' }))).not.toBe(
        agentKey(live(1, null))
      )
    })

    it('gives the session back as the number the worker answers to', () => {
      expect(drivenSessionOf(drivenRowId(7))).toBe(7)
    })

    it('says nothing about any other row the panel draws', () => {
      expect(drivenSessionOf(1)).toBe(null)
      expect(drivenSessionOf('start-1')).toBe(null)
      expect(drivenSessionOf('9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60')).toBe(null)
      expect(drivenSessionOf(null)).toBe(null)
      expect(drivenSessionOf(undefined)).toBe(null)
    })

    /* A tail that is not a number must read as "not a conversation" rather than
       as conversation NaN, which would reach the worker as an id. */
    it('refuses a prefix with nothing usable behind it', () => {
      expect(drivenSessionOf(DRIVEN_PREFIX)).toBe(null)
      expect(drivenSessionOf(`${DRIVEN_PREFIX}later`)).toBe(null)
    })
  })

  describe('what such a row offers', () => {
    const row = drivenAgentRow({ id: 3, state: 'needs-you', elapsed: '4m' })

    /* Both refusals the menu draws over this row come from these two fields,
       and both are true of a driven session: there is no conversation id for a
       pin to survive a restart under, and no PTY for a clearing line to be
       written into. */
    it('has no conversation id, so pinning refuses itself', () => {
      expect(row.conversation).toBe(null)
    })

    it('cannot be cleared', () => {
      expect(row.clearable).toBe(false)
    })

    /* A start ticket's flag greys the cross, and this row's cross must work: the
       session exists by the time the row does. */
    it('is not a start, so its cross is live from the first frame', () => {
      expect(row.starting).toBe(undefined)
    })

    it('is captioned the way a bare agent is', () => {
      expect(row.label).toBe('Agent')
      expect(row.tasks).toEqual([])
    })

    /* The caption is the store's own word for the same work, and the title goes
       inside the label rather than beside it: `tasks` is drawn in mono, where a
       person's own sentence would read as an identifier. */
    it('says what a resumed conversation is and names it', () => {
      const resumed = drivenAgentRow({
        id: 4,
        state: 'ready',
        elapsed: '2m',
        work: { kind: 'resumeSession', title: 'Move the card to done' }
      })

      expect(resumed.label).toBe('Resumed session: Move the card to done')
      expect(resumed.tasks).toEqual([])
    })

    /* A transcript nobody typed a word into — a run's batch, a setup, an
       earlier "+ New agent" — has no title, which is an ordinary answer rather
       than a gap to fill with something invented. */
    it('says what it is and stops when the transcript has no title', () => {
      const resumed = drivenAgentRow({
        id: 4,
        state: 'ready',
        elapsed: '2m',
        work: { kind: 'resumeSession', title: null }
      })

      expect(resumed.label).toBe('Resumed session')
    })

    /* The whole point of the id reaching the row: the panel's order and its
       pins are kept under the conversation, which is the one name for this
       session that will still mean something tomorrow. */
    it('is keyed by its conversation the moment the worker has named one', () => {
      const named = drivenAgentRow({
        id: 3,
        state: 'running',
        elapsed: '1m',
        conversation: '9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60'
      })

      expect(named.conversation).toBe('9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60')
      expect(agentKey(named)).toBe('9f1c0a2e-6d4b-4f77-8f1a-0c2b3d4e5f60')
      expect(agentKey(row)).toBe(`${DRIVEN_PREFIX}3`)
    })

    it('carries the state and the elapsed time it was given', () => {
      expect(row.state).toBe('needs-you')
      expect(row.elapsed).toBe('4m')
    })
  })

  describe('the two kinds of session in one list', () => {
    it('puts a conversation under the live rows and above the offline ones', () => {
      const rows = [live(1, 'a1'), offline('9f1c')]

      expect(mergeAgentRows(rows, [driven(1)]).map((row) => row.id)).toEqual([
        1,
        `${DRIVEN_PREFIX}1`,
        '9f1c'
      ])
    })

    it('puts it at the end when the project has nothing offline', () => {
      const rows = [live(1, 'a1'), live(2, 'a2')]

      expect(mergeAgentRows(rows, [driven(5)]).map((row) => row.id)).toEqual([
        1,
        2,
        `${DRIVEN_PREFIX}5`
      ])
    })

    /* `orderAgents` reads identity to tell "this project has never been
       arranged" from "arranged, and this is what it came to", so a project with
       no conversation in it must come back with the very array that went in. */
    it('gives the rows back by reference when there is no conversation', () => {
      const rows = [live(1, 'a1')]

      expect(mergeAgentRows(rows, [])).toBe(rows)
    })

    /* The registry holds a record for a session that is *running* — it is
       written at the spawn — so from the moment an offline row is pressed the
       same conversation would be in the panel twice: the agent somebody is
       watching, and an offer to reopen what they are already looking at.
       `offeredRecords` in stores/terminals.js is the same rule for PTY
       sessions, and it cannot see this one. */
    it('hides the offer a live conversation is standing in front of', () => {
      const rows = [live(1, 'a1'), offline('9f1c'), offline('other')]
      const resumed = { ...driven(2), conversation: '9f1c' }

      expect(mergeAgentRows(rows, [resumed]).map((row) => row.id)).toEqual([
        1,
        `${DRIVEN_PREFIX}2`,
        'other'
      ])
    })

    /* A fork writes no record — `--fork-session` has the harness invent an id
       this app never learns — so there is no offer of its to hide, and the row
       it was started from goes on standing. */
    it('hides nothing for a conversation that was never recorded', () => {
      const rows = [offline('9f1c')]

      expect(mergeAgentRows(rows, [driven(2)]).map((row) => row.id)).toEqual([
        `${DRIVEN_PREFIX}2`,
        '9f1c'
      ])
    })

    it('draws a conversation in a project with no PTY agent at all', () => {
      expect(mergeAgentRows([], [driven(1), driven(2)]).map((row) => row.id)).toEqual([
        `${DRIVEN_PREFIX}1`,
        `${DRIVEN_PREFIX}2`
      ])
    })
  })

  describe('the footer counter', () => {
    it('counts a conversation beside the terminal sessions', () => {
      expect(mergeLiveAgentCount(2, [driven(1), driven(2, 'needs-you')])).toBe(4)
    })

    /* The same pair the terminal store leaves out, arrived at from the other
       end: there it is the one raw `exited`, here it is the two words it turns
       into. */
    it('leaves out the ones that have ended', () => {
      expect(mergeLiveAgentCount(0, [driven(1, 'done'), driven(2, 'failed')])).toBe(0)
    })

    /* A state added to Rust and not yet to this front end reads as a live
       agent, which is the softer way to be wrong: a number one too high beats a
       working agent that stopped being counted. */
    it('counts a state nobody has heard of', () => {
      expect(mergeLiveAgentCount(0, [driven(1, 'thinking')])).toBe(1)
    })
  })

  describe('the sentence beside it', () => {
    it('adds the waiting ones to the loud half and the rest to the live half', () => {
      expect(
        mergeAgentCounts({ loud: 1, live: 1 }, [driven(1, 'needs-you'), driven(2, 'ready')])
      ).toEqual({ loud: 2, live: 2 })
    })

    it('says nothing of its own about a project with no conversation', () => {
      expect(mergeAgentCounts({ loud: 0, live: 3 }, [])).toEqual({ loud: 0, live: 3 })
    })

    /* The store's own clamp, kept for its reason: the subtraction is exact
       today, and if it ever stops being the sentence goes quiet rather than
       announcing "-1 agents running". */
    it('never counts fewer than none', () => {
      expect(mergeAgentCounts({ loud: 0, live: 0 }, [driven(1, 'needs-you')]).live).toBe(0)
    })
  })

  describe('the rail', () => {
    const states = { '/p': { state: 'live', live: 1, loud: 0 }, '/other': { state: 'idle', live: 0, loud: 0 } }

    it('makes a project loud for a conversation waiting on somebody', () => {
      const merged = mergeProjectStates(states, [driven(1, 'needs-you')])

      expect(merged['/p']).toEqual({ state: 'loud', live: 1, loud: 1 })
      expect(merged['/other']).toEqual({ state: 'idle', live: 0, loud: 0 })
    })

    it('lights a project whose only agent is a conversation', () => {
      expect(mergeProjectStates({}, [driven(1, 'running', '/fresh')])['/fresh']).toEqual({
        state: 'live',
        live: 1,
        loud: 0
      })
    })

    /* A session that has spoken and waits on nobody is the driven spelling of
       an `idle` PTY session, which the map counts for neither half. */
    it('leaves a project alone for a conversation with nothing to do', () => {
      expect(mergeProjectStates({}, [driven(1, 'ready', '/quiet')])['/quiet']).toEqual({
        state: 'idle',
        live: 0,
        loud: 0
      })
    })

    /* The map it is given is a computed in another store, and a merge that
       wrote into it would be a second author of a derived value. */
    it('writes nothing into the map it was given', () => {
      mergeProjectStates(states, [driven(1, 'needs-you')])

      expect(states['/p']).toEqual({ state: 'live', live: 1, loud: 0 })
    })
  })
})
