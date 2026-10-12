import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { AnalysisOutput } from '../../entities/analysis/model'
import type { Card } from '../../entities/card/model'
import { AnalysisResult } from './AnalysisResult'

const cards: Card[] = [
  {
    id: 'card-alpha',
    title: '从信息到判断的距离',
    content: '材料一',
    note: '笔记一',
    type: 'article',
    sourceType: 'web',
    sourceName: '阅读',
    sourceUrl: '',
    tags: ['阅读'],
    status: 'pending-analysis',
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
  },
  {
    id: 'card-beta',
    title: '捕获动作的摩擦',
    content: '材料二',
    note: '笔记二',
    type: 'thought',
    sourceType: 'personal',
    sourceName: '快速记录',
    sourceUrl: '',
    tags: ['捕获'],
    status: 'pending-analysis',
    createdAt: '2026-01-02T00:00:00.000Z',
    updatedAt: '2026-01-02T00:00:00.000Z',
  },
]

const output: AnalysisOutput = {
  summary: '这个想法可以先延伸成几个待确认的问题。',
  themes: [
    {
      title: '引用驱动判断',
      detail: '重要观点需要附带材料引用。',
      references: [
        { cardId: 'card-alpha', label: '阅读沉淀' },
        { cardId: 'card-beta', label: '捕获阻力' },
      ],
    },
  ],
  conflicts: ['需要查询这个概念是否已有成熟定义。'],
  actions: ['先降低捕获门槛，再补充一个可验证问题。'],
  openQuestions: ['哪些引用最能解释下一步方向？'],
}

describe('AnalysisResult', () => {
  afterEach(cleanup)

  it('renders the five expected result sections', () => {
    render(
      <AnalysisResult
        cards={cards}
        output={output}
        onReferenceClick={vi.fn()}
      />,
    )

    expect(screen.getByRole('heading', { name: '直接回应' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: '可延伸方向' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: '需要查询或确认' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: '下一步行动' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: '尚未解决的问题' })).toBeInTheDocument()
    expect(screen.getByText(output.summary)).toBeInTheDocument()
  })

  it('renders focusable reference buttons and reports the clicked card id', async () => {
    const user = userEvent.setup()
    const onReferenceClick = vi.fn()

    render(
      <AnalysisResult
        cards={cards}
        output={output}
        onReferenceClick={onReferenceClick}
      />,
    )

    const reference = screen.getByRole('button', {
      name: '来源卡片：从信息到判断的距离',
    })

    reference.focus()
    expect(reference).toHaveFocus()

    await user.click(reference)
    expect(onReferenceClick).toHaveBeenCalledWith('card-alpha')
  })
})
