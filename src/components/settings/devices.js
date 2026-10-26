/* The Devices tab's rules, pure: what the pairing code is drawn as, how long it
   has left, how a device and a request are put into words, and what a relay
   address has to look like to be worth sending.

   Out of the component for the reason every module beside this one is: a
   `.vue` file is the one thing no test in this repository can reach, so the
   whole of each rule lives here. */
import QRCode from 'qrcode'

/* The relay a desktop uses until its owner names another: `DEFAULT_RELAY_URL`
   in `crates/remote-server/src/lib.rs`, the single copy this front end holds.
   `defaults()` in `stores/settings.js`, `view` in `SettingsWindow.vue` and the
   Devices tab's prop default all import it rather than spell it again, and
   `tests/components/settings/devices.test.js` reads the Rust file to keep the
   one remaining pair in step. */
export const DEFAULT_RELAY_URL = 'https://relay.smetana.io'

/* The longest stretch the tab will draw a code for after the desktop's answer
   comes back wrong about it. The desktop issues two-minute codes
   (`crates/remote-server/src/pairing.rs`, `SECRET_TTL_SECS`); this is only a ceiling on what the
   countdown will believe, so a reply with a nonsense `expires_at` cannot park
   a code on screen for a day. */
export const MAX_CODE_SECONDS = 600

/* The module matrix of a code as one SVG path, and nothing else.

   Drawn by the component from this path rather than by the library's own
   `toString(…, { type: 'svg' })`, for the one reason that matters in this tree:
   that string carries its colours as attributes, and a colour here is a
   `var(--token)`. A custom property is not valid in a presentation attribute,
   so the library's markup could not be themed — and the only way to put it on
   the page would be `v-html`, which this project keeps out of its components.
   Runs of dark modules in a row are merged into one segment, which is what
   keeps the path a few kilobytes rather than a few tens.

   Error correction is low: this is shown on a screen, held still and lit from
   behind, which is the easy case for a camera, and every level up makes the
   modules smaller on a window this narrow.

   Returns `null` for text that cannot be encoded (empty, or past what the
   largest code holds), which the caller draws as no code rather than a crash. */
export function qrCode(text) {
  if (typeof text !== 'string' || text === '') return null
  let modules
  try {
    modules = QRCode.create(text, { errorCorrectionLevel: 'L' }).modules
  } catch {
    return null
  }
  const size = modules.size
  let path = ''
  for (let y = 0; y < size; y++) {
    let x = 0
    while (x < size) {
      if (!modules.get(y, x)) {
        x++
        continue
      }
      let end = x
      while (end < size && modules.get(y, end)) end++
      path += `M${x} ${y}h${end - x}v1h-${end - x}z`
      x = end
    }
  }
  return { size, path }
}

/* Whole seconds a code has left, never negative. `expiresAt` is the desktop's
   Unix seconds and `nowMs` the browser's clock in milliseconds: the two clocks
   are one machine's here, since the desktop is the process that opened this
   window. */
export function remainingSeconds(expiresAt, nowMs) {
  if (!Number.isFinite(expiresAt)) return 0
  const left = Math.ceil(expiresAt - nowMs / 1000)
  return Math.min(Math.max(left, 0), MAX_CODE_SECONDS)
}

export function countdownLine(seconds) {
  if (seconds <= 0) return 'This code has expired.'
  const minutes = Math.floor(seconds / 60)
  const rest = String(seconds % 60).padStart(2, '0')
  return `Expires in ${minutes}:${rest}`
}

export function platformLabel(platform) {
  if (platform === 'ios') return 'iOS'
  if (platform === 'android') return 'Android'
  return 'Unknown platform'
}

/* The sentence over the Accept and Reject buttons. The name is whatever the
   phone called itself, so it is shown as text and never interpreted, and the
   platform beside it is the one thing a person can compare against the phone
   in their hand. */
export function requestLine(request) {
  if (!request) return ''
  const name = request.deviceName?.trim() || 'A phone'
  return `${name} wants to connect`
}

export function requestDetail(request) {
  if (!request) return ''
  return `${platformLabel(request.platform)}. Only accept if you are pairing this phone yourself.`
}

/* "Paired 3 Oct" is the date, "Last seen 4 min ago" is the contact. Relative
   for the contact because that is the question a person has about a device
   they are about to revoke — is it in use right now. */
export function lastContactLine(device, nowMs) {
  if (!device || device.lastContact == null) return 'Not connected yet'
  const seconds = Math.max(0, Math.floor(nowMs / 1000 - device.lastContact))
  if (seconds < 60) return 'Last seen just now'
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `Last seen ${minutes} min ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `Last seen ${hours} h ago`
  const days = Math.floor(hours / 24)
  return `Last seen ${days} ${days === 1 ? 'day' : 'days'} ago`
}

export function pairedLine(device) {
  if (!device || !Number.isFinite(device.pairedAt)) return ''
  const date = new Date(device.pairedAt * 1000)
  return `Paired ${date.toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })}`
}

const MAX_RELAY_URL = 2048

/* Whether a relay address can be handed to the desktop. A second copy of
   `valid_relay_url` in `settings/model.rs`, and under the same obligation the
   column list of the Kanban tab carries: Rust is the authority and replaces
   anything it refuses with the default on the next save, **silently** — so the
   field must not offer a value Rust will throw away, or the person types an
   address, sees it stay, and finds the default back after a restart. This
   copy exists to say so before that. */
export function validRelayUrl(raw) {
  const url = typeof raw === 'string' ? raw.trim() : ''
  if (!url.startsWith('https://') || url.length > MAX_RELAY_URL) return false
  const rest = url.slice('https://'.length)
  // eslint-disable-next-line no-control-regex
  if (/[\s\u0000-\u001f\u007f-\u009f]/.test(rest)) return false
  let host = rest.split(/[/?#]/)[0]
  const colon = host.lastIndexOf(':')
  if (colon >= 0) {
    const port = host.slice(colon + 1)
    if (port !== '' && /^[0-9]+$/.test(port)) host = host.slice(0, colon)
  }
  return (
    host !== '' &&
    !host.includes('@') &&
    /^[A-Za-z0-9.-]+$/.test(host) &&
    !/^[.-]/.test(host) &&
    !/^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$/.test(host)
  )
}

/* The line under the relay field, or `null` when the value is acceptable. */
export function relayProblem(raw) {
  const url = typeof raw === 'string' ? raw.trim() : ''
  if (url === '') return 'Enter the address of a relay.'
  if (!url.startsWith('https://')) return 'A relay address must start with https://.'
  if (!validRelayUrl(url)) return 'That is not a usable relay address. Use a host name, not an IP address.'
  return null
}
