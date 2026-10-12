import { createBrowserRouter, type RouteObject } from 'react-router-dom'
import { AnalysisPage } from '../pages/AnalysisPage'
import { InboxPage } from '../pages/InboxPage'
import { InsightPage } from '../pages/InsightPage'
import { NotFoundPage } from '../pages/NotFoundPage'
import App from './App'

export const routes: RouteObject[] = [
  {
    path: '/',
    element: <App />,
    children: [
      {
        index: true,
        element: <InboxPage />,
      },
      {
        path: 'analysis/:analysisId',
        element: <AnalysisPage />,
      },
      {
        path: 'insights',
        element: <InsightPage />,
      },
      {
        path: 'insights/:insightId',
        element: <InsightPage />,
      },
      {
        path: '*',
        element: <NotFoundPage />,
      },
    ],
  },
]

export const router = createBrowserRouter(routes)
