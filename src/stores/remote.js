/* The phone companion, as the Devices tab sees it: begin a pairing, answer the
   request that comes in, list the paired phones and revoke one.

   One of the only files in `src/` that knows Tauri exists, and there is no state
   here at all — `src-tauri/src/remote/` owns the device list and the pairing
   secret, and the settings window that draws them keeps what it was last told.
   What this file does is rename: Rust's fields are snake_case, as the spec for
   the commands fixes them, and a component should read camelCase like every
   other value it is handed. It also decides what a component never sees. A
   device record carries a push token and a push key on disk; Rust does not send
   them here at all, and this file reads only the fields the tab draws.

   Errors are thrown as `Error`s with a readable message, the shape
   `attachments.js` answers with, so the window can put one on a line. */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/* `remote/mod.rs`'s `PAIR_REQUEST_EVENT` and `DEVICES_EVENT`, spelled the same
   on both sides. */
export const PAIR_REQUEST_EVENT = 'remote:pair-request'
export const PAIR_ENDED_EVENT = 'remote:pair-ended'
export const DEVICES_EVENT = 'remote:devices'

function messageOf(err) {
  if (err && typeof err === 'object' && typeof err.message === 'string') return err.message
  return String(err)
}

/* A device in the shape a component reads. Anything that is not a record with a
   key is dropped by the caller rather than drawn as a row with no name. */
export function toDevice(raw) {
  if (!raw || typeof raw !== 'object' || typeof raw.key !== 'string') return null
  return {
    key: raw.key,
    name: typeof raw.name === 'string' && raw.name !== '' ? raw.name : 'Unnamed phone',
    platform: raw.platform === 'ios' || raw.platform === 'android' ? raw.platform : null,
    pairedAt: Number.isFinite(raw.paired_at) ? raw.paired_at : null,
    lastContact: Number.isFinite(raw.last_contact) ? raw.last_contact : null
  }
}

export function toDevices(list) {
  return Array.isArray(list) ? list.map(toDevice).filter(Boolean) : []
}

async function call(command, args) {
  try {
    return await invoke(command, args)
  } catch (err) {
    console.error(`[remote] ${command} failed:`, err)
    throw new Error(messageOf(err))
  }
}

/* Starts the endpoint if it is not up and issues a ticket. `ticket` is the
   string the QR code carries and `expiresAt` the Unix second it stops working.

   The relay is **passed**, the one in the field, and not left for Rust to read
   from `settings.json`: an edit reaches that file only after the app window's
   debounce, so a press right behind an edit would otherwise pair through the old
   relay while the screen showed the new one. */
export async function beginPairing(relayUrl) {
  const answer = await call('remote_pair_begin', { relayUrl })
  return { ticket: String(answer.ticket), expiresAt: Number(answer.expires_at) }
}

export async function cancelPairing() {
  await call('remote_pair_cancel')
}

/* The person's answer to a request that `watchPairRequests` announced. */
export async function decidePairing(accept) {
  await call('remote_pair_decide', { accept: accept === true })
}

export async function listDevices() {
  return toDevices(await call('remote_devices'))
}

export async function revokeDevice(key) {
  await call('remote_device_revoke', { key })
}

/* Each answers with the way to stop listening. A request arrives as
   `{ deviceName, platform }`. */
export async function watchPairRequests(handler) {
  return listen(PAIR_REQUEST_EVENT, (event) => {
    const payload = event.payload ?? {}
    handler({
      deviceName: typeof payload.device_name === 'string' ? payload.device_name : '',
      platform: payload.platform === 'ios' || payload.platform === 'android' ? payload.platform : null
    })
  })
}

/* The request on screen is over — decided, timed out, or the phone left. */
export async function watchPairEnded(handler) {
  return listen(PAIR_ENDED_EVENT, () => handler())
}

export async function watchDevices(handler) {
  return listen(DEVICES_EVENT, (event) => handler(toDevices(event.payload)))
}
