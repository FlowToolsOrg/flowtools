import { StrictMode } from 'react'

import { RouterProvider, createRouter } from '@tanstack/react-router'
import ReactDOM from 'react-dom/client'

import { bootstrap } from '@/app/bootstrap'

import './assets/globals.css'
import { routeTree } from './routeTree.gen'

const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

const rootElement = document.getElementById('root')!

async function start(): Promise<void> {
  await bootstrap((path: string) => {
    router.navigate({ to: path })
  })

  if (!rootElement.innerHTML) {
    const root = ReactDOM.createRoot(rootElement)
    root.render(
      <StrictMode>
        <RouterProvider router={router} />
      </StrictMode>
    )
  }
}

start()
