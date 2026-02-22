import { createFileRoute } from '@tanstack/react-router'

import todoList from '@plugins/plugin-todo-list'

import { renderWebAppPlugin } from '@/runtime'

export const Route = createFileRoute('/plugin/todo-list')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <div className="w-screen">
      <div className="container mx-auto mt-8">
        {renderWebAppPlugin(todoList)}
      </div>
    </div>
  )
}
