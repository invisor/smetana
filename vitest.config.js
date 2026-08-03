import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config.js'

/* Отдельный конфиг, а не ключ test в vite.config.js: конфиг приложения
   остаётся конфигом приложения, а mergeConfig приносит оттуда плагин Vue и
   алиас @, когда они понадобятся. */
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'happy-dom',
      include: ['tests/**/*.test.js'],
      setupFiles: ['tests/support/setup.js'],
      globals: false
    }
  })
)
