import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { ConceptDetail } from '../../entities/concept/model'
import { DesktopError } from '../../shared/errors/DesktopError'
import { DeleteConceptDialog } from './DeleteConceptDialog'

const concept: ConceptDetail = {
  id: 'concept-1',
  name: '费曼学习法',
  explanation: '通过讲解检验理解。',
  mentionCount: 2,
  updatedAt: '2026-07-29T08:00:00Z',
  mentions: [],
}

describe('DeleteConceptDialog', () => {
  it('explains the impact and keeps cancel as the default action', async () => {
    const user = userEvent.setup()
    const onClose = vi.fn()
    render(
      <DeleteConceptDialog
        concept={concept}
        onClose={onClose}
        onConfirm={vi.fn()}
      />,
    )

    const dialog = screen.getByRole('dialog', { name: '删除概念' })
    expect(within(dialog).getByRole('button', { name: '取消' })).toHaveFocus()
    expect(
      within(dialog).getByText(
        /将删除这个概念、概念解释，以及卡片中的 2 处对应高亮。卡片文字不会被删除/,
      ),
    ).toBeInTheDocument()

    await user.click(within(dialog).getByRole('button', { name: '取消' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('closes after deletion and keeps the dialog open when deletion fails', async () => {
    const user = userEvent.setup()
    const onClose = vi.fn()
    const onConfirm = vi
      .fn()
      .mockRejectedValueOnce(new DesktopError('concept_delete_failed', '无法删除概念', '请重试'))
      .mockResolvedValueOnce(undefined)
    render(
      <DeleteConceptDialog concept={concept} onClose={onClose} onConfirm={onConfirm} />,
    )

    await user.click(screen.getByRole('button', { name: '删除概念' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('无法删除概念')
    expect(onClose).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: '删除概念' }))
    expect(onConfirm).toHaveBeenCalledTimes(2)
    expect(onClose).toHaveBeenCalledTimes(1)
  })
})
