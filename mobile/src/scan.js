/* The camera, through the official barcode-scanner plugin. Its own file so the
   plugin is imported where it is used and nowhere else: in a browser (the dev
   server) it does not exist, and the paste field is the way in. */
export async function scanTicket() {
  const { scan, checkPermissions, requestPermissions, Format } = await import(
    '@tauri-apps/plugin-barcode-scanner'
  )
  let permission = await checkPermissions()
  if (permission !== 'granted') permission = await requestPermissions()
  if (permission !== 'granted') throw new Error('The camera is not allowed. Allow it in Settings, or paste the code.')
  const result = await scan({ windowed: false, formats: [Format.QRCode] })
  return result?.content ?? null
}
