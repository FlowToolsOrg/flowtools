import { createFileRoute } from '@tanstack/react-router'

import randomPicker from '@plugins/plugin-random-picker'

import { renderWebAppPlugin } from '@/runtime'

export const Route = createFileRoute('/plugin/random-picker')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <div className="w-screen">
      <div className="container mx-auto mt-8">
        {renderWebAppPlugin(randomPicker)}
      </div>
    </div>
  )
}
