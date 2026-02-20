import { Button, Code } from '@flow-tool/ui'

export const App = () => {
  return (
    <main className="shell">
      <h1>UI Testbed</h1>
      <p className="muted">Manual smoke check for @flow-tool/ui components.</p>
      <div className="row">
        <Button appName="ui-test" className="button">
          Trigger Button
        </Button>
      </div>
      <Code className="snippet">npm run test</Code>
    </main>
  )
}
