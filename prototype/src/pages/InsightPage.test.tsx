import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { AnalysisSet } from '../entities/analysis/model'
import { useIdeaCardStore } from '../store/useIdeaCardStore'
import { AnalysisPage } from './AnalysisPage'
import { InboxPage } from './InboxPage'
import { InsightPage } from './InsightPage'

function renderInsights(initialEntry: string) {
  const router = createMemoryRouter(
    [
      { path: '/', element: <InboxPage /> },
      { path: '/analysis/:analysisId', element: <AnalysisPage /> },
      { path: '/insights', element: <InsightPage /> },
      { path: '/insights/:insightId', element: <InsightPage /> },
    ],
    { initialEntries: [initialEntry] },
  )

  render(<RouterProvider router={router} />)
  return router
}

function createSavedInsight() {
  const cards = useIdeaCardStore.getState().cards.slice(0, 2)
  const analysis: AnalysisSet = {
    id: 'analysis-completed',
    title: '阅读后的思考沉淀',
    goal: '归纳最值得验证的产品机会',
    mode: 'actions',
    cardIds: cards.map((card) => card.id),
    status: 'completed',
    output: {
      summary: '低摩擦捕获和后续回访缺一不可。',
      themes: [
        {
          title: '捕获与加工',
          detail: '捕获应快，判断应可追溯。',
          references: [{ cardId: cards[0].id, label: cards[0].title }],
        },
      ],
      conflicts: ['即时整理与稍后整理存在操作成本冲突。'],
      actions: ['验证用户在记录后 24 小时内的回访率。'],
      openQuestions: ['什么提醒方式不会造成打扰？'],
    },
    followUps: [
      {
        id: 'follow-up-1',
        question: '最值得优先验证的建议是什么？',
        answer: '优先验证快速记录后的回访处理率。',
        cardId: cards[0].id,
        createdAt: '2026-06-10T09:00:00.000Z',
      },
    ],
    createdAt: '2026-06-10T08:00:00.000Z',
    updatedAt: '2026-06-10T08:00:00.000Z',
  }

  useIdeaCardStore.setState({ analyses: [analysis] })
  const insightId = useIdeaCardStore.getState().saveInsight(analysis.id)
  return { insightId, cards, analysis }
}

describe('InsightPage', () => {
  beforeEach(() => {
    localStorage.clear()
    useIdeaCardStore.getState().reset()
  })

  afterEach(() => {
    cleanup()
    localStorage.clear()
  })

  it('shows an empty insights list with a return action', async () => {
    const user = userEvent.setup()
    const router = renderInsights('/insights')

    expect(screen.getByRole('heading', { name: '结论' })).toBeInTheDocument()
    expect(screen.getByText('还没有保存的结论')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '返回 Inbox' }))
    expect(router.state.location.pathname).toBe('/')
  })

  it('lists saved insights and opens the detail page', async () => {
    const user = userEvent.setup()
    const { insightId } = createSavedInsight()
    const insight = useIdeaCardStore
      .getState()
      .insights.find((item) => item.id === insightId)!
    const savedAt = new Intl.DateTimeFormat('zh-CN', {
      month: 'numeric',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(insight.createdAt))
    const router = renderInsights('/insights')

    expect(screen.getByText('阅读后的思考沉淀')).toBeInTheDocument()
    expect(screen.getByText('2 张来源卡片')).toBeInTheDocument()
    expect(screen.getByText(`保存于 ${savedAt}`)).toBeInTheDocument()

    await user.click(screen.getByRole('link', { name: '打开结论：阅读后的思考沉淀' }))
    expect(router.state.location.pathname).toBe(`/insights/${insightId}`)
  })

  it('shows insight detail, follow-ups, source cards, and navigates back to sources', async () => {
    const user = userEvent.setup()
    const { insightId, cards } = createSavedInsight()
    const router = renderInsights(`/insights/${insightId}`)

    expect(screen.getByRole('heading', { name: '阅读后的思考沉淀' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: '直接回应' })).toBeInTheDocument()
    expect(screen.getByText('优先验证快速记录后的回访处理率。')).toBeInTheDocument()

    const sourceList = screen.getByRole('list', { name: '来源卡片' })
    await user.click(
      within(sourceList).getByRole('button', {
        name: `打开来源卡片：${cards[0].title}`,
      }),
    )

    expect(router.state.location.pathname).toBe('/')
    expect(router.state.location.search).toBe(`?card=${cards[0].id}`)
    expect(screen.getByLabelText('我的想法')).toHaveValue(cards[0].note)
  })

  it('continues extension from the original context', async () => {
    const user = userEvent.setup()
    const { insightId, analysis } = createSavedInsight()
    const router = renderInsights(`/insights/${insightId}`)

    await user.click(screen.getByRole('button', { name: '基于原上下文继续延伸' }))

    expect(router.state.location.pathname).toBe(`/analysis/${analysis.id}`)
    expect(screen.getByLabelText('想延伸或查询的问题')).toHaveValue(analysis.goal)
  })

  it('shows a not-found state for a missing insight', () => {
    renderInsights('/insights/missing')

    expect(screen.getByRole('heading', { name: '未找到结论' })).toBeInTheDocument()
  })
})
