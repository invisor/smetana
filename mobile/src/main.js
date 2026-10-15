import { createApp } from 'vue'
import App from './App.vue'
import { suppressNativeMenus } from '@src/nativeMenu.js'
import '@src/styles/styles.css'

suppressNativeMenus(document)
createApp(App).mount('#app')
