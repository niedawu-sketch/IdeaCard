import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { Card } from '../../entities/card/model'
import { DeleteCardDialog } from './DeleteCardDialog'

const archivedCard: Card = {
  id: 'card-archived',
  title: '已归档想法',
  originalContent: '不再需要的内容',
  note: '',
  sourceType: 'personal',
  captureMethod: 'quick',
  sourceName: '快速记录',
  sourceUri: '',
  status: 'archived',
  attachments: [],
  createdAt: '2026-07-19T08:00:00Z',
  updatedAt: '2026-07-19T09:00:00Z',
  archivedAt: '2026-07-19T09:00:00Z',
}

describe('DeleteCardDialog', () => {
  it('warns about content and attachment deletion and focuses cancel', () => {
    render(
      <DeleteCardDialog
        card={archivedCard}
        onClose={vi.fn()}
        onConfirm={vi.fn().mockResolvedValue(undefined)}
      />,
    )

    const dialog = screen.getByRole('dialog', { name: '永久删除卡片' })
    expect(within(dialog).getByText('已归档想法')).toBeInTheDocument()
    expect(
      within(dialog).getByText('删除后无法恢复，卡片内容和本地附件将一并删除。'),
    ).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: '取消' })).toHaveFocus()
  })

  it('confirms deletion and closes only after success', async () => {
    const user = userEvent.setup()
    const onClose = vi.fn()
    const onConfirm = vi.fn().mockResolvedValue(undefined)
    render(<DeleteCardDialog card={archivedCard} onClose={onClose} onConfirm={onConfirm} />)

    await user.click(screen.getByRole('button', { name: '永久删除' }))

    expect(onConfirm).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('keeps the dialog open and reports deletion failures', async () => {
    const user = userEvent.setup()
    render(
      <DeleteCardDialog
        card={archivedCard}
        onClose={vi.fn()}
        onConfirm={vi.fn().mockRejectedValue({
          code: 'card_delete_referenced',
          message: '这张卡片仍被其他内容引用',
          recovery: '请先解除引用',
        })}
      />,
    )

    await user.click(screen.getByRole('button', { name: '永久删除' }))

    const dialog = screen.getByRole('dialog', { name: '永久删除卡片' })
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('这张卡片仍被其他内容引用')
    expect(within(dialog).getByRole('alert')).toHaveTextContent('请先解除引用')
  })

  it('closes on Escape without deleting', async () => {
    const user = userEvent.setup()
    const onClose = vi.fn()
    const onConfirm = vi.fn()
    render(<DeleteCardDialog card={archivedCard} onClose={onClose} onConfirm={onConfirm} />)

    await user.keyboard('{Escape}')

    expect(onClose).toHaveBeenCalledTimes(1)
    expect(onConfirm).not.toHaveBeenCalled()
  })
})
