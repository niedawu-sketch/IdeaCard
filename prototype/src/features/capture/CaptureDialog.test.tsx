import { cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { InboxPage } from '../../pages/InboxPage'
import { useIdeaCardStore } from '../../store/useIdeaCardStore'

function renderInbox() {
  const router = createMemoryRouter(
    [{ path: '/', element: <InboxPage /> }],
    { initialEntries: ['/'] },
  )

  render(<RouterProvider router={router} />)
  return router
}

describe('CaptureDialog', () => {
  beforeEach(() => {
    localStorage.clear()
    useIdeaCardStore.getState().reset()
  })

  afterEach(() => {
    cleanup()
    localStorage.clear()
  })

  it('saves quick capture with content only, derives the title, and inserts it at the top', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('button', { name: '打开快速记录' }))

    const dialog = screen.getByRole('dialog', { name: '快速记录' })
    const content = within(dialog).getByLabelText('记录内容')
    expect(content).toHaveFocus()

    await user.type(content, '这是一个快速记录内容超过二十个字符用于标题截断')
    await user.click(within(dialog).getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })

    const [topCard] = useIdeaCardStore.getState().cards
    expect(topCard).toMatchObject({
      title: '这是一个快速记录内容超过二十个字符用于标',
      content: '这是一个快速记录内容超过二十个字符用于标题截断',
      note: '',
      type: 'thought',
      sourceType: 'personal',
      sourceName: '快速记录',
    })

    const list = screen.getByLabelText('卡片列表')
    expect(within(list).getAllByRole('button')[0]).toHaveTextContent(
      '这是一个快速记录内容超过二十个字符',
    )
  })

  it('loads the clipboard demo content and allows saving it', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('button', { name: '打开剪贴板捕获' }))

    const dialog = screen.getByRole('dialog', { name: '剪贴板捕获' })
    expect(within(dialog).getByText('正在读取模拟剪贴板...')).toBeInTheDocument()

    await waitFor(() => {
      expect(within(dialog).getByLabelText('标题')).toHaveValue('剪贴板摘录')
    })
    await waitFor(() => {
      expect(within(dialog).getByRole('button', { name: '保存' })).toBeEnabled()
    })
    expect(within(dialog).getByLabelText('记录内容')).toHaveValue(
      'AI 总结需要引用原始卡片，不能只给无来源结论。',
    )

    await user.click(within(dialog).getByRole('button', { name: '保存' }))
    await user.click(within(dialog).getByRole('button', { name: '仍然保存' }))

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
    expect(useIdeaCardStore.getState().cards[0]).toMatchObject({
      title: '剪贴板摘录',
      type: 'clipboard',
      sourceType: 'personal',
      sourceName: '系统剪贴板',
    })
  })

  it('shows duplicate warning, returns without saving, and can still save after confirmation', async () => {
    const user = userEvent.setup()
    const initialCount = useIdeaCardStore.getState().cards.length
    renderInbox()

    await user.click(screen.getByRole('button', { name: '打开快速记录' }))
    const dialog = screen.getByRole('dialog', { name: '快速记录' })
    await user.type(
      within(dialog).getByLabelText('记录内容'),
      '产品的核心不是记录更多，而是缩短从信息到判断的距离。',
    )
    await user.click(within(dialog).getByRole('button', { name: '保存' }))

    expect(within(dialog).getByRole('alert')).toHaveTextContent('可能重复')

    await user.click(within(dialog).getByRole('button', { name: '返回修改' }))
    expect(useIdeaCardStore.getState().cards).toHaveLength(initialCount)
    expect(screen.getByRole('dialog', { name: '快速记录' })).toBeInTheDocument()

    await user.click(within(dialog).getByRole('button', { name: '保存' }))
    await user.click(within(dialog).getByRole('button', { name: '仍然保存' }))

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
    expect(useIdeaCardStore.getState().cards).toHaveLength(initialCount + 1)
  })

  it('keeps the screenshot dialog open on OCR failure, then retries and saves', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('button', { name: '打开截图捕获' }))
    const dialog = screen.getByRole('dialog', { name: '截图捕获' })
    await user.click(within(dialog).getByRole('button', { name: '演示识别失败' }))

    await waitFor(() => {
      expect(within(dialog).getByRole('alert')).toHaveTextContent(
        '未识别到清晰文字',
      )
    })
    expect(screen.getByRole('dialog', { name: '截图捕获' })).toBeInTheDocument()

    await user.click(within(dialog).getByRole('button', { name: '重新识别' }))

    await waitFor(() => {
      expect(within(dialog).getByLabelText('标题')).toHaveValue(
        '截图中的产品观点',
      )
    })
    await waitFor(() => {
      expect(within(dialog).getByRole('button', { name: '保存' })).toBeEnabled()
    })

    await user.click(within(dialog).getByRole('button', { name: '保存' }))
    await user.click(within(dialog).getByRole('button', { name: '仍然保存' }))

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
    expect(useIdeaCardStore.getState().cards[0]).toMatchObject({
      title: '截图中的产品观点',
      type: 'screenshot',
      sourceType: 'image',
      sourceName: '屏幕截图 OCR',
    })
  })

  it('ignores pending screenshot recognition after the dialog is closed', async () => {
    const user = userEvent.setup()
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined)
    const initialCount = useIdeaCardStore.getState().cards.length
    renderInbox()

    try {
      await user.click(screen.getByRole('button', { name: '打开截图捕获' }))
      const dialog = screen.getByRole('dialog', { name: '截图捕获' })
      await user.click(within(dialog).getByRole('button', { name: '演示识别失败' }))
      await user.click(within(dialog).getByRole('button', { name: '关闭' }))

      await new Promise((resolve) => window.setTimeout(resolve, 500))

      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      expect(useIdeaCardStore.getState().cards).toHaveLength(initialCount)
      expect(
        consoleError.mock.calls
          .flat()
          .some((message) =>
            String(message).match(/act|unmounted|state update/i),
          ),
      ).toBe(false)
    } finally {
      consoleError.mockRestore()
    }
  })

  it('ignores stale screenshot recognition after switching to quick capture', async () => {
    const user = userEvent.setup()
    const initialCount = useIdeaCardStore.getState().cards.length
    renderInbox()

    await user.click(screen.getByRole('button', { name: '打开截图捕获' }))
    const screenshotDialog = screen.getByRole('dialog', { name: '截图捕获' })
    await user.click(within(screenshotDialog).getByRole('button', { name: '正常识别' }))
    await user.click(screen.getByRole('button', { name: '打开快速记录' }))

    const quickDialog = screen.getByRole('dialog', { name: '快速记录' })
    await new Promise((resolve) => window.setTimeout(resolve, 500))

    expect(within(quickDialog).getByLabelText('标题')).toHaveValue('')
    expect(within(quickDialog).getByLabelText('记录内容')).toHaveValue('')
    expect(useIdeaCardStore.getState().cards).toHaveLength(initialCount)
  })

  it('resets capture state when switching capture kinds without closing first', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('button', { name: '打开剪贴板捕获' }))
    const clipboardDialog = screen.getByRole('dialog', { name: '剪贴板捕获' })
    await waitFor(() => {
      expect(within(clipboardDialog).getByLabelText('标题')).toHaveValue(
        '剪贴板摘录',
      )
    })

    await user.click(screen.getByRole('button', { name: '打开快速记录' }))

    const quickDialog = screen.getByRole('dialog', { name: '快速记录' })
    expect(within(quickDialog).getByLabelText('标题')).toHaveValue('')
    expect(within(quickDialog).getByLabelText('记录内容')).toHaveValue('')
    expect(
      within(quickDialog).queryByText('正在读取模拟剪贴板...'),
    ).not.toBeInTheDocument()
    expect(within(quickDialog).queryByRole('alert')).not.toBeInTheDocument()
  })

  it('does not allow saving a card with only a title', async () => {
    const user = userEvent.setup()
    const initialCount = useIdeaCardStore.getState().cards.length
    renderInbox()

    await user.click(screen.getByRole('button', { name: '打开快速记录' }))
    const dialog = screen.getByRole('dialog', { name: '快速记录' })
    await user.type(within(dialog).getByLabelText('标题'), '只有标题')

    expect(within(dialog).getByRole('button', { name: '保存' })).toBeDisabled()
    await user.click(within(dialog).getByRole('button', { name: '保存' }))

    expect(useIdeaCardStore.getState().cards).toHaveLength(initialCount)
    expect(screen.getByRole('dialog', { name: '快速记录' })).toBeInTheDocument()
  })

  it('does not create a card when the dialog is closed', async () => {
    const user = userEvent.setup()
    const initialCount = useIdeaCardStore.getState().cards.length
    renderInbox()

    await user.click(screen.getByRole('button', { name: '打开快速记录' }))
    await user.type(screen.getByLabelText('记录内容'), '关闭时不应该创建卡片')
    await user.click(screen.getByRole('button', { name: '关闭' }))

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
    expect(useIdeaCardStore.getState().cards).toHaveLength(initialCount)
  })
})
