import { createFileRoute } from '@tanstack/react-router'
import { useEffect } from 'react'

import helloWorldPlugin from '../../../../plugins/plugin-example-hello-world'
import runHello from '../../../../plugins/plugin-example-run-hello'
import { renderWebAppPlugin, runWebToolPlugin } from '../runtime'

export const Route = createFileRoute('/test')({
  component: RouteComponent,
})

function RouteComponent() {
  useEffect(() => {
    runWebToolPlugin(runHello, 'li hua')
  }, [])
  return (
    <div>
      Hello "/test"!
      <section>{renderWebAppPlugin(helloWorldPlugin)}</section>
    </div>
  )
}
