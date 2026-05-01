import { createFileRoute } from '@tanstack/react-router'

import uuidGenerator from '@plugins/plugin-uuid-generator'

import { renderWebAppPlugin } from '@/runtime'

export const Route = createFileRoute('/plugin/uuid-generator')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <div className="w-screen">
      <div className="container mx-auto mt-8">
        {renderWebAppPlugin(uuidGenerator)}
      </div>
    </div>
  )
}
