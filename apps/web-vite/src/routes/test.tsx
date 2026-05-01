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
    void output
  }, [])
  return (
    <div>
      Hello "/test"!
      <section>
        <h1 className="text-2xl">Hello World Plugin</h1>
        <div>{renderWebAppPlugin(helloWorldPlugin)}</div>
      </section>
      <section>
        <h1 className="text-2xl">Todo List Plugin</h1>
        <div>{renderWebAppPlugin(todoListPlugin)}</div>
      </section>
    </div>
  )
}
