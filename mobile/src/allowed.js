/* What the phone may ask of the desktop, as the epic fixed it (smetana-032e).

   The authority is `ALLOWED_COMMANDS` and `ALLOWED_EVENTS` in
   crates/remote-protocol/src/bridge.rs, which the desktop's bridge enforces; this
   is the phone's own copy of the two lists, held equal to those by
   tests/allowed.test.js, so that the phone can also stop itself: a name that is
   not here is never handed to the core, and the dev server (which has no core)
   refuses it exactly as the core would. */

export const ALLOWED_COMMANDS = [
  'tracker_health',
  'tracker_snapshot',
  'tracker_set_project',
  'settings_load',
  'agents_catalog',
  'agents_installed',
  'project_config',
  'target_branches',
  'run_state',
  'run_reports',
  'run_start',
  'run_stop',
  'session_start',
  'session_attach',
  'session_since',
  'session_send',
  'session_answer',
  'session_stop',
  'crew_tree',
  'crew_attach',
  'crew_send',
  'crew_stop',
  'terminal_list',
  'terminal_attach',
  'attachment_write',
  'session_list',
  'session_close',
  'terminal_restorable',
  'terminal_forget',
  'terminal_remove'
]

export const ALLOWED_EVENTS = [
  'tracker:delta',
  'tracker:health',
  'run:state',
  'session:events',
  'crew:events',
  'terminal:state',
  'terminal:removed',
  'session:state',
  'session:removed',
  'terminal:restorable',
  'settings:changed'
]

export const isAllowedCommand = (name) => ALLOWED_COMMANDS.includes(name)
export const isAllowedEvent = (name) => ALLOWED_EVENTS.includes(name)
