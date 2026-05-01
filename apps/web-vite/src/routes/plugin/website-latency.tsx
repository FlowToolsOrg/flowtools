import { createFileRoute } from '@tanstack/react-router'

import websiteLatency from '@plugins/plugin-website-latency'

import { renderWebAppPlugin } from '@/runtime'

export const Route = createFileRoute('/plugin/website-latency')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <div className="w-screen">
      <div className="container mx-auto mt-8">
        {renderWebAppPlugin(websiteLatency)}
      </div>
    </div>
  )
}
