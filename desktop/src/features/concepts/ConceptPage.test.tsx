import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { ConceptDetail, ConceptSummary } from '../../entities/concept/model'
import { ConceptPage } from './ConceptPage'

const summaries: ConceptSummary[] = [{
  id: 'concept-1',
  name: '费曼学习法',
  explanation: '通过讲解检验自己是否真正理解。',
  mentionCount: 2,
  updatedAt: '2026-07-19T08:00:00Z',
}]

const detail: ConceptDetail = {
  ...summaries[0],
  mentions: [
    {
      mentionId: 'mention-1',
      cardId: 'card-1',
      cardTitle: '如何学习复杂概念',
      selectedText: '费曼学习法',
      cardStatus: 'annotated',
      createdAt: '2026-07-19T08:00:00Z',
    },
    {
      mentionId: 'mention-2',
      cardId: 'card-2',
      cardTitle: '旧读书笔记',
      selectedText: '费曼技巧',
      cardStatus: 'archived',
      createdAt: '2026-07-18T08:00:00Z',
    },
  ],
}

describe('ConceptPage', () => {
  it('shows a concept explanation and opens a selected source mention', async () => {
    const user = userEvent.setup()
    const onOpenSource = vi.fn()
    render(
      <ConceptPage
        onOpenLink={vi.fn()}
        concepts={summaries}
        detail={detail}
        selectedConceptId="concept-1"
        onDelete={vi.fn()}
        onOpenSource={onOpenSource}
        onSelect={vi.fn()}
      />,
    )

    expect(screen.getByRole('heading', { name: '费曼学习法' })).toBeInTheDocument()
    expect(screen.getAllByText('通过讲解检验自己是否真正理解。')).toHaveLength(2)
    expect(screen.getByText('2 处引用')).toBeInTheDocument()
    expect(screen.getByText('已归档')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /如何学习复杂概念/ }))
    expect(onOpenSource).toHaveBeenCalledWith({
      cardId: 'card-1',
      mentionId: 'mention-1',
      cardStatus: 'annotated',
    })
  })

  it('uses a quiet empty state before any concept is saved', () => {
    render(
      <ConceptPage
        onOpenLink={vi.fn()}
        concepts={[]}
        detail={null}
        selectedConceptId={null}
        onDelete={vi.fn()}
        onOpenSource={vi.fn()}
        onSelect={vi.fn()}
      />,
    )

    expect(screen.getByText('还没有保存的概念')).toBeInTheDocument()
  })

  it('requires confirmation before deleting the selected concept explanation', async () => {
    const user = userEvent.setup()
    const onDelete = vi.fn().mockResolvedValue(undefined)
    render(
      <ConceptPage
        onOpenLink={vi.fn()}
        concepts={summaries}
        detail={detail}
        selectedConceptId="concept-1"
        onDelete={onDelete}
        onOpenSource={vi.fn()}
        onSelect={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('button', { name: '删除概念' }))
    const dialog = screen.getByRole('dialog', { name: '删除概念' })
    expect(dialog).toHaveTextContent('2 处对应高亮')
    expect(onDelete).not.toHaveBeenCalled()

    await user.click(within(dialog).getByRole('button', { name: '删除概念' }))
    expect(onDelete).toHaveBeenCalledWith('concept-1')
  })
})
