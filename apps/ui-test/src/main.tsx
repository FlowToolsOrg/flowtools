import { StrictMode } from 'react'

import ReactDOM from 'react-dom/client'

import { App } from './app'
import { AppearanceValidation } from './appearance/appearance-validation'
import './assets/globals.css'
import '@flowtools/ui/appearance.css'

const rootElement = document.getElementById('root')
const appearanceValidation = new URLSearchParams(window.location.search).has(
  'appearance-validation'
)

if (appearanceValidation) document.documentElement.lang = 'zh-CN'

if (!rootElement) {
  throw new Error('Root element not found')
}

ReactDOM.createRoot(rootElement).render(
  <StrictMode>
    {appearanceValidation ? <AppearanceValidation /> : <App />}
  </StrictMode>
)
