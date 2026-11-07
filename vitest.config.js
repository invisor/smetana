import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config.js'
import { fileURLToPath, URL } from 'node:url'

/* A config of its own rather than a `test` key in vite.config.js: the app's
   config stays the app's config, and mergeConfig brings the Vue plugin and the
   @ alias over from it when they are needed. */
export default mergeConfig(
  viteConfig,
  defineConfig({
    resolve: { alias: { '@src': fileURLToPath(new URL('./src', import.meta.url)) } },
    test: {
      environment: 'happy-dom',
      include: ['tests/**/*.test.js'],
      setupFiles: ['tests/support/setup.js'],
      globals: false
    }
  })
)
