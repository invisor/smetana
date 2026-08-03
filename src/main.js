import { createApp } from 'vue'
import App from './App.vue'
import { installMockBackend } from './stores/mockBackend.js'
import './styles/styles.css'

// In a browser we swap the IPC for fixtures; under Tauri it does nothing.
installMockBackend()

createApp(App).mount('#app')
