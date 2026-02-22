import { StrictMode } from 'react'

import ReactDOM from 'react-dom/client'

import { App } from './app'
import './assets/globals.css'

const rootElement = document.getElementById('root')

if (!rootElement) {
  throw new Error('Root element not found')
}

ReactDOM.createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>
)
