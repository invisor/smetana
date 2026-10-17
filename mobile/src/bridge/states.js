/* What a connection status means, as pure rules. The Rust core says which state
   the link is in (`bridge:status`); what the page does about each is decided
   here and nowhere else. */

export const ONLINE = 'online'
export const RECOMMENDED = 'update-recommended'
export const REQUIRED = 'update-required'
export const UNREACHABLE = 'unreachable'
export const CONNECTING = 'connecting'
export const REVOKED = 'revoked'

/* The link works. A desktop that would like the phone to be newer still serves
   it: `update-recommended` is online with a banner. */
export const isOnline = (status) => status?.state === ONLINE || status?.state === RECOMMENDED

/* The protocol minor of the desktop on the other end, which the core reads from its
   hello and puts on every status that means the link works. What is not online has
   none to report, and 0 hides whatever a newer desktop would have added. */
export const desktopMinor = (status) =>
  isOnline(status) && Number.isInteger(status.protocol_minor) ? status.protocol_minor : 0

/* Which screen the shell draws. A phone that is not paired has one thing to do;
   one that is too old for its desktop has one screen and calls nothing, whatever
   the link is doing. */
export function screenFor(paired, status) {
  if (paired === null) return 'loading'
  if (!paired) return 'pair'
  switch (status?.state) {
    case REQUIRED:
      return 'update'
    case REVOKED:
      return 'revoked'
    case UNREACHABLE:
      return 'unreachable'
    case ONLINE:
    case RECOMMENDED:
      return 'home'
    default:
      return 'connecting'
  }
}

/* The sentence under "Workstation unreachable". `lastSeen` is milliseconds since
   the epoch, or absent when this phone has never reached the desktop. */
export function lastSeenText(lastSeen, now = Date.now(), locale = undefined) {
  if (!lastSeen) return 'This phone has not reached it yet.'
  const when = new Date(lastSeen)
  const sameDay = new Date(now).toDateString() === when.toDateString()
  const time = when.toLocaleTimeString(locale, { hour: 'numeric', minute: '2-digit' })
  if (sameDay) return `Last seen at ${time}.`
  const day = when.toLocaleDateString(locale, { month: 'short', day: 'numeric' })
  return `Last seen ${day} at ${time}.`
}
