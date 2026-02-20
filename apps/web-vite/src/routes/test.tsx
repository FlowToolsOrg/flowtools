import { createFileRoute } from '@tanstack/react-router'

import helloWorldPlugin from '../../../../plugins/plugin-example-hello-world'
import { renderWebAppPlugin } from '../runtime'

export const Route = createFileRoute('/test')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <div>
      Hello "/test"!
      <section>{renderWebAppPlugin(helloWorldPlugin)}</section>
    </div>
  )
}
