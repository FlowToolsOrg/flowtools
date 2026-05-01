import { useEffect } from 'react'

import { createFileRoute } from '@tanstack/react-router'

import helloWorldPlugin from '@plugins/plugin-example-hello-world'
import todoListPlugin from '@plugins/plugin-todo-list'

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
    <div className="container max-w-7xl m-auto flex flex-col gap-8 mt-8">
      <section className="border p-8 rounded-2xl">
        <h1 className="text-2xl mb-2">Hello World Plugin</h1>
        <div>{renderWebAppPlugin(helloWorldPlugin)}</div>
      </section>
      <section className="border p-8 rounded-2xl">
        <h1 className="text-2xl mb-2">Todo List Plugin</h1>
        <div>{renderWebAppPlugin(todoListPlugin)}</div>
      </section>
    </div>
  )
}
