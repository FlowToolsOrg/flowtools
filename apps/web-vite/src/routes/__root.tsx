import { createRootRoute, Link, Outlet } from '@tanstack/react-router'
import { TanStackRouterDevtools } from '@tanstack/react-router-devtools'

const navItems = [
  { to: '/', label: 'Home' },
  { to: '/market', label: 'Market' },
  { to: '/detail', label: 'Detail' },
  { to: '/run', label: 'Run' },
  { to: '/settings', label: 'Settings' },
  { to: '/test', label: 'Plugin Test' },
] as const

const RootLayout = () => (
  <>
    <header className="sticky top-0 z-20 border-b border-slate-200/80 bg-white/90 px-4 py-3 backdrop-blur lg:px-8">
      <nav className="mx-auto flex w-full max-w-7xl flex-wrap items-center gap-2">
        {navItems.map(item => (
          <Link
            key={item.to}
            activeProps={{ className: 'bg-slate-900 text-white' }}
            className="rounded-xl border border-slate-300 bg-white px-3 py-1.5 text-sm font-medium text-slate-700 transition hover:bg-slate-50"
            to={item.to}
          >
            {item.label}
          </Link>
        ))}
      </nav>
    </header>
    <Outlet />
    <TanStackRouterDevtools />
  </>
)

export const Route = createRootRoute({ component: RootLayout })
