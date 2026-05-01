import { createFileRoute } from '@tanstack/react-router'

import imageBase64 from '@plugins/plugin-image-base64'

import { renderWebAppPlugin } from '@/runtime'

export const Route = createFileRoute('/plugin/image-base64')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <div className="w-screen">
      <div className="container mx-auto mt-8">
        {renderWebAppPlugin(imageBase64)}
      </div>
    </div>
  )
}
