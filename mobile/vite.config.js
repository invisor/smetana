import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath } from 'node:url'
import { dirname, resolve, sep } from 'node:path'

const here = (path) => fileURLToPath(new URL(path, import.meta.url))
const storesDir = here('../src/stores') + sep

/* The seam. A store in ../src/stores/ reaches the desktop through two imports,
   `@tauri-apps/api/core` and `@tauri-apps/api/event`; on the phone there is no
   desktop in the process, so those two specifiers are answered by the bridge
   (./src/bridge/), which calls `bridge_invoke` and `bridge_listen` of the real
   Tauri underneath. The stores themselves are not touched.

   Only an import *from a store* is swapped. The bridge modules import the real
   `@tauri-apps/api/core` themselves, and a rule that swapped every importer
   would send them back to themselves forever. `importer` is what makes the
   difference, and it is compared as a path prefix, so a file elsewhere in the
   tree that happens to be called `stores` is not caught. */
const BRIDGED = {
  '@tauri-apps/api/core': here('./src/bridge/core.js'),
  '@tauri-apps/api/event': here('./src/bridge/event.js')
}

export function bridgeTransport() {
  return {
    name: 'smetana-bridge-transport',
    enforce: 'pre',
    resolveId(source, importer) {
      const target = BRIDGED[source]
      if (!target || !importer) return null
      const file = importer.split('?')[0]
      return file.startsWith(storesDir) ? target : null
    }
  }
}

/* Files under ../src import packages by bare name, and Node looks for those from
   the file's own directory upwards, which leads to the repository root's
   node_modules and not this package's. Two things go wrong that way: the build
   would need the root's `npm install` as well as this one, and `vue` would be
   bundled twice, so a component from the desktop's tree would not share the
   reactivity of the page it is drawn in. A bare import from ../src is therefore
   resolved as if it had been written in this package. */
const srcDir = here('../src') + sep
const entry = here('./index.html')

export function packagesFromHere() {
  return {
    name: 'smetana-packages-from-mobile',
    enforce: 'pre',
    async resolveId(source, importer, options) {
      if (!importer || /^[./\\\0]/.test(source) || source.startsWith('virtual:')) return null
      if (!importer.split('?')[0].startsWith(srcDir)) return null
      return this.resolve(source, entry, { ...options, skipSelf: true })
    }
  }
}

/* Two stores of the desktop are answered by a phone file of the same shape, for the
   reason that what they do has no meaning here, and one of them would reach the desktop
   for it.

   - `stores/app.js` opens windows, reads the build and picks files with the system
     dialog. The conversation panel imports five names from it; the phone's version
     (./src/phone/app.js) gives it those five and nothing else, so the panel's links open
     in the system browser, a picture has no window of its own to open in, and the
     paperclip finds no file to pick (it is not drawn at all, see `attachable`).
   - `stores/windowDrops.js` follows a file dragged over a desktop window, and asks the
     desktop which units the position arrives in (`drag_drop_space`, which is not on the
     allowlist). There is no drag on a phone.

   Matched by the file the import resolves to, so a relative import from any depth is
   caught and a file elsewhere with the same name is not. */
const PHONE_STORES = {
  [here('../src/stores/app.js')]: here('./src/phone/app.js'),
  [here('../src/stores/windowDrops.js')]: here('./src/phone/windowDrops.js')
}

export function phoneStores() {
  return {
    name: 'smetana-phone-stores',
    enforce: 'pre',
    resolveId(source, importer) {
      if (!importer || !/^[./]/.test(source)) return null
      const file = importer.split('?')[0]
      if (!file.startsWith(srcDir)) return null
      return PHONE_STORES[resolve(dirname(file), source)] ?? null
    }
  }
}

export default defineConfig({
  plugins: [bridgeTransport(), packagesFromHere(), phoneStores(), vue()],
  clearScreen: false,
  server: {
    host: process.env.TAURI_DEV_HOST || false,
    port: 5174,
    strictPort: true,
    // ../src is read from outside this package's root.
    fs: { allow: [here('..')] }
  },
  resolve: {
    alias: { '@src': resolve(here('../src')) },
    dedupe: ['vue']
  },
  // The system webviews Tauri runs in on a phone: WKWebView and Android System WebView.
  build: { target: ['es2021', 'chrome100', 'safari15'], chunkSizeWarningLimit: 1200 }
})
