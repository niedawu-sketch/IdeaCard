import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { InboxPage } from '../../pages/InboxPage'
import { useIdeaCardStore } from '../../store/useIdeaCardStore'

function renderInbox(initialEntry = '/') {
  const router = createMemoryRouter(
    [{ path: '/', element: <InboxPage /> }],
    { initialEntries: [initialEntry] },
  )

  render(<RouterProvider router={router} />)
  return router
}

describe('CardList', () => {
  beforeEach(() => {
    localStorage.clear()
    useIdeaCardStore.getState().reset()
  })

  afterEach(() => {
    cleanup()
    localStorage.clear()
  })

  it('toggles selection without opening the card detail', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('checkbox', {
      name: '选择「从信息到判断的距离」',
    }))

    expect(useIdeaCardStore.getState().selectedCardIds).toEqual([
      'card-reading-distance',
    ])
    expect(screen.getByText('选择一张卡片开始处理')).toBeInTheDocument()
  })

  it('opens a card from the list body and keeps the existing status query', async () => {
    const user = userEvent.setup()
    const router = renderInbox('/?status=pending-analysis')

    await user.click(screen.getByText('从信息到判断的距离'))

    expect(router.state.location.search).toContain(
      'status=pending-analysis',
    )
    expect(router.state.location.search).toContain(
      'card=card-reading-distance',
    )
  })
})
