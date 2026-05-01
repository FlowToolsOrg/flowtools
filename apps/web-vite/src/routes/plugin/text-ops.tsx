import { createFileRoute } from '@tanstack/react-router'

import textOps from '@plugins/plugin-text-ops'

import { renderWebAppPlugin } from '@/runtime'

export const Route = createFileRoute('/plugin/text-ops')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <div className="w-screen">
      <div className="container mx-auto mt-8">
        {renderWebAppPlugin(textOps)}
      </div>
    </div>
  )
}
