/* Opens a store page in the person's own browser or store app. */
import { openUrl } from '@tauri-apps/plugin-opener'

export async function openStore(url) {
  if (!url) return
  try {
    await openUrl(url)
  } catch (err) {
    console.warn('[companion] could not open the store page:', err)
  }
}
