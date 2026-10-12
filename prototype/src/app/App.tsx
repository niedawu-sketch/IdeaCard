import {
  Archive,
  CircleDotDashed,
  Inbox,
  Lightbulb,
  Sparkles,
} from 'lucide-react'
import { Link, NavLink, Outlet, useLocation } from 'react-router-dom'

const navigationItems = [
  { label: '收集箱', to: '/', icon: Inbox, status: null },
  {
    label: '未处理',
    to: '/?status=unprocessed',
    icon: CircleDotDashed,
    status: 'unprocessed',
  },
  {
    label: '待分析',
    to: '/?status=pending-analysis',
    icon: Sparkles,
    status: 'pending-analysis',
  },
  { label: '结论', to: '/insights', icon: Lightbulb, status: null },
  {
    label: '已归档',
    to: '/?status=archived',
    icon: Archive,
    status: 'archived',
  },
]

function App() {
  const location = useLocation()
  const activeStatus = new URLSearchParams(location.search).get('status')

  return (
    <div className="app-shell">
      <aside className="app-sidebar">
        <Link className="app-brand" to="/">
          <span className="app-brand-mark" aria-hidden="true">
            I
          </span>
          <span>IdeaCard</span>
        </Link>

        <nav className="app-navigation" aria-label="主导航">
          <span className="app-navigation-label">工作区</span>
          <ul className="app-navigation-list">
            {navigationItems.map(({ label, to, icon: Icon, status }) => {
              const isCurrent =
                to === '/insights'
                  ? location.pathname.startsWith('/insights')
                  : location.pathname === '/' && activeStatus === status

              return (
                <li key={to}>
                  <NavLink
                    className={() =>
                      `app-navigation-link${isCurrent ? ' is-current' : ''}`
                    }
                    to={to}
                    aria-current={isCurrent ? 'page' : 'false'}
                  >
                    <Icon aria-hidden="true" size={17} strokeWidth={1.8} />
                    <span>{label}</span>
                  </NavLink>
                </li>
              )
            })}
          </ul>
        </nav>

        <div className="app-sidebar-footer">
          <span className="app-connection-dot" aria-hidden="true" />
          本地工作区
        </div>
      </aside>

      <main className="app-main">
        <Outlet />
      </main>
    </div>
  )
}

export default App
