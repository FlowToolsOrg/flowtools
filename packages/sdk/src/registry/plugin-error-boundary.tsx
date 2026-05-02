import { Component, type ErrorInfo, type ReactNode } from 'react'

interface PluginErrorBoundaryProps {
  pluginId: string
  fallback?: ReactNode
  children: ReactNode
}

interface PluginErrorBoundaryState {
  hasError: boolean
  error?: Error
}

/**
 * Error boundary that catches rendering errors from plugin panels.
 * Shows a fallback UI with the option to reload the plugin.
 */
export class PluginErrorBoundary extends Component<
  PluginErrorBoundaryProps,
  PluginErrorBoundaryState
> {
  constructor(props: PluginErrorBoundaryProps) {
    super(props)
    this.state = { hasError: false }
  }

  static getDerivedStateFromError(error: Error): PluginErrorBoundaryState {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    void info
    void error
  }

  handleRetry = (): void => {
    this.setState({ hasError: false, error: undefined })
  }

  render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback
      }

      return (
        <div className="flex flex-col items-center justify-center gap-4 p-8 text-center">
          <div className="text-4xl">⚠️</div>
          <div>
            <h3 className="text-lg font-semibold text-(--foreground)">
              Plugin Error
            </h3>
            <p className="mt-1 text-sm text-(--muted)">
              {this.props.pluginId} encountered an error during rendering.
            </p>
          </div>
          {this.state.error ? (
            <pre className="max-w-full overflow-x-auto rounded-(--radius) border border-(--border) bg-(--surface) p-3 text-xs text-(--muted)">
              {this.state.error.message}
            </pre>
          ) : null}
          <button
            className="rounded-(--radius) bg-(--accent) px-4 py-2 text-sm font-medium text-(--accent-foreground) transition hover:opacity-90"
            onClick={this.handleRetry}
            type="button"
          >
            Reload Plugin
          </button>
        </div>
      )
    }

    return this.props.children
  }
}
