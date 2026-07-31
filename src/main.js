import { createApp } from 'vue'
import App from './App.vue'
import { installMockBackend } from './stores/mockBackend.js'
import './styles/styles.css'

// В браузере подменяем IPC фикстурами; под Tauri ничего не делает.
installMockBackend()

createApp(App).mount('#app')
