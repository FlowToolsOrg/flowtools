import helloWorldPlugin from '@plugins/plugin-example-hello-world'
import { createFileRoute } from '@tanstack/react-router'
import { useEffect } from 'react'

import { runHello } from '@/plugin/tool'
import { renderWebAppPlugin } from '@/runtime'

export const Route = createFileRoute('/test')({
  component: RouteComponent,
})

function RouteComponent() {
  useEffect(() => {
    const output = runHello('li hua')
    void output
  }, [])
  return (
    <div>
      Hello "/test"!
      <section>{renderWebAppPlugin(helloWorldPlugin)}</section>
    </div>
  )
}
