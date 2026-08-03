import { Compartment } from '@codemirror/state'

/* Compartments are keys, not data: each one's value lives in the EditorState.
   That is why they are shared by every editor instance. Were they inside the
   component, a state saved by a previous instance would carry foreign keys, and
   a reconfigure against them would silently do nothing. */
export const readOnlyState = new Compartment()
export const languageState = new Compartment()

/* The update listener closes over its own instance's props and emit, while
   the state outlives the instance: after a return from the board the component
   is new and the state is the old one. So the listener is a compartment too:
   adopting somebody else's state re-points it at the live instance. Otherwise
   edits would go into a destroyed component's emit — silently, because as far
   as CodeMirror is concerned it is still a working extension. */
export const updateListenerState = new Compartment()
