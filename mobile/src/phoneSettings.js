/* The phone's own settings: the few the person can set here instead of taking the desktop's.

   Only what the phone really draws is offered: the theme, the density, the font size, and the
   board's column mode and period. The agent and the model are not here, since `session_start` from
   a phone carries neither and the server picks its own; the column lists (`alwaysShow`,
   `unlimited`) stay the desktop's. Nothing here is ever sent anywhere: the choice lives in this
   phone's `localStorage`, and the desktop's settings are never written from the phone.

   `'desktop'` means "as the desktop has it" and is the default everywhere but the font size, which
   is only ever the phone's own (the desktop's is never copied).

   Pure: the storage is handed in, so that a test needs no DOM. */
import { THEME_CHOICES, FONT_SIZES, UI_FONT_DEFAULT, clampFont } from '../../src/appearance.js'
import { COLUMN_MODE_CHOICES, INTERVAL_CHOICES } from '../../src/components/kanban/boardView.js'

const KEY = 'smetana.companion.settings'

export const DESKTOP = 'desktop'

export const PHONE_DEFAULTS = Object.freeze({
  theme: DESKTOP,
  density: DESKTOP,
  uiFontSize: UI_FONT_DEFAULT,
  columns: DESKTOP,
  interval: DESKTOP
})

const SAME = { value: DESKTOP, label: 'Same as desktop' }

export const THEME_OPTIONS = [SAME, ...THEME_CHOICES]
export const DENSITY_OPTIONS = [
  SAME,
  { value: 'comfortable', label: 'Comfortable' },
  { value: 'compact', label: 'Compact' }
]
export const COLUMN_OPTIONS = [SAME, ...COLUMN_MODE_CHOICES]
export const INTERVAL_OPTIONS = [SAME, ...INTERVAL_CHOICES]
export const FONT_OPTIONS = FONT_SIZES.map((size) => ({ value: size, label: `${size} px` }))

const oneOf = (options, value, fallback) => (options.some((o) => o.value === value) ? value : fallback)

/* Whatever was stored, as a complete valid object: an unknown or damaged value falls to its
   default, as `readKanban` does. */
export function readPhoneSettings(raw) {
  const from = raw && typeof raw === 'object' ? raw : {}
  return {
    theme: oneOf(THEME_OPTIONS, from.theme, PHONE_DEFAULTS.theme),
    density: oneOf(DENSITY_OPTIONS, from.density, PHONE_DEFAULTS.density),
    uiFontSize: clampFont(from.uiFontSize, PHONE_DEFAULTS.uiFontSize),
    columns: oneOf(COLUMN_OPTIONS, from.columns, PHONE_DEFAULTS.columns),
    interval: oneOf(INTERVAL_OPTIONS, from.interval, PHONE_DEFAULTS.interval)
  }
}

export function loadPhoneSettings(storage) {
  try {
    return readPhoneSettings(JSON.parse(storage.getItem(KEY) ?? 'null'))
  } catch {
    // Damaged or unreadable: as if nothing were stored.
    return readPhoneSettings(null)
  }
}

export function savePhoneSettings(storage, value) {
  try {
    storage.setItem(KEY, JSON.stringify(readPhoneSettings(value)))
  } catch {
    // A full or refused store only costs the choice surviving a restart.
  }
}

/* The look to draw: the phone's choice where it made one, the desktop's otherwise. */
export function resolveLook(phone, desktopTheme, desktopDensity) {
  return {
    theme: phone.theme === DESKTOP ? desktopTheme : phone.theme,
    density: phone.density === DESKTOP ? desktopDensity : phone.density
  }
}

/* The board's view with the phone's column mode and period over the desktop's; the lists stay. */
export function resolveKanban(phone, kanban) {
  return {
    ...kanban,
    columns: phone.columns === DESKTOP ? kanban.columns : phone.columns,
    interval: phone.interval === DESKTOP ? kanban.interval : phone.interval
  }
}
