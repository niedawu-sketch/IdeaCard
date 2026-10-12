import { cleanup, render } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { afterEach, describe, expect, it } from 'vitest'
import { routes } from './router'

afterEach(cleanup)

function renderRoute(initialEntry: string) {
  const memoryRouter = createMemoryRouter(routes, {
    initialEntries: [initialEntry],
  })

  return render(<RouterProvider router={memoryRouter} />)
}

describe('App', () => {
  it('renders the desktop shell and index page through the nested routes', () => {
    const view = renderRoute('/')

    expect(view.getByText('IdeaCard')).toBeInTheDocument()
    expect(view.getByRole('link', { name: '收集箱' })).toBeInTheDocument()
    expect(view.getByRole('main')).toBeInTheDocument()
    expect(
      view.getByRole('heading', { name: '收集箱' }),
    ).toBeInTheDocument()
  })

  it.each([
    '/?status=unprocessed&card=x',
    '/?card=x&status=unprocessed',
  ])(
    'marks the status navigation active regardless of query order: %s',
    (initialEntry) => {
      const view = renderRoute(initialEntry)

      expect(view.getByRole('link', { name: '未处理' })).toHaveAttribute(
        'aria-current',
        'page',
      )
      expect(view.getByRole('link', { name: '收集箱' })).toHaveAttribute(
        'aria-current',
        'false',
      )
    },
  )

  it('keeps the inbox active when the URL has no status filter', () => {
    const view = renderRoute('/?card=x')

    expect(view.getByRole('link', { name: '收集箱' })).toHaveAttribute(
      'aria-current',
      'page',
    )
  })

  it('does not mark the brand as the current navigation item', () => {
    const view = renderRoute('/')

    expect(view.getByRole('link', { name: 'IdeaCard' })).not.toHaveAttribute(
      'aria-current',
    )
  })

  it('renders a Chinese not-found page for unknown routes', () => {
    const view = renderRoute('/missing-page')

    expect(
      view.getByRole('heading', { name: '页面未找到' }),
    ).toBeInTheDocument()
    expect(view.queryByText('Unexpected Application Error!')).toBeNull()
  })
})
