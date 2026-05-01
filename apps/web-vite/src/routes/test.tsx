import { useEffect } from 'react'

import { createFileRoute } from '@tanstack/react-router'

import { helloWorldPlugin, todoListPlugin } from '@/plugin/app'
import apps from '@/plugin/app'
import { runHello } from '@/plugin/tool'
import { renderWebAppPlugin } from '@/runtime'

export const Route = createFileRoute('/test')({
  component: RouteComponent,
})

function RouteComponent() {
  useEffect(() => {
    const output = runHello('li hua')
    console.log(output)
    console.log(todoListPlugin)
  }, [])
  return (
    <div className="container m-auto mt-8 flex max-w-7xl flex-col gap-8">
      {apps.map(app => (
        <section className="rounded-2xl border p-8" key={app.meta.id}>
          <h1 className="mb-2 text-2xl">{app.meta.name}</h1>
          <div>{renderWebAppPlugin(app)}</div>
        </section>
      ))}
    </div>
  )
}
