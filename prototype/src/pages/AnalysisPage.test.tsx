import { cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { StrictMode } from 'react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { AnalysisSet } from '../entities/analysis/model'
import { useIdeaCardStore } from '../store/useIdeaCardStore'
import { AnalysisPage } from './AnalysisPage'
import { InboxPage } from './InboxPage'
import { InsightPage } from './InsightPage'

function renderAnalysis(initialEntry: string) {
  const router = createMemoryRouter(
    [
      { path: '/', element: <InboxPage /> },
      { path: '/analysis/:analysisId', element: <AnalysisPage /> },
      { path: '/insights/:insightId', element: <InsightPage /> },
    ],
    { initialEntries: [initialEntry] },
  )

  render(
    <StrictMode>
      <RouterProvider router={router} />
    </StrictMode>,
  )
  return router
}

function createCompletedAnalysis() {
  const cards = useIdeaCardStore.getState().cards.slice(0, 2)
  return createAnalysis({
    title: '阅读后的思考沉淀',
    goal: '归纳最值得验证的产品机会',
    mode: 'actions',
    status: 'completed',
    cardIds: cards.map((card) => card.id),
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
  })
}

function createAnalysis(overrides: Partial<AnalysisSet> = {}) {
  const cardIds = useIdeaCardStore
    .getState()
    .cards.slice(0, 3)
    .map((card) => card.id)
  const now = '2026-01-01T00:00:00.000Z'
  const analysis: AnalysisSet = {
    id: 'analysis-test',
    title: '未命名分析',
    goal: '',
    mode: 'themes',
    cardIds,
    status: 'draft',
    followUps: [],
    createdAt: now,
    updatedAt: now,
    ...overrides,
  }

  useIdeaCardStore.setState({ analyses: [analysis] })
  return analysis
}

describe('AnalysisPage', () => {
  beforeEach(() => {
    localStorage.clear()
    useIdeaCardStore.getState().reset()
  })

  afterEach(() => {
    cleanup()
    localStorage.clear()
  })

  it('shows a not-found state when the analysis id does not exist', async () => {
    const user = userEvent.setup()
    const router = renderAnalysis('/analysis/missing')

    expect(screen.getByRole('heading', { name: '未找到分析集' })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '返回 Inbox' }))
    expect(router.state.location.pathname).toBe('/')
  })

  it('disables starting analysis until the goal has content', async () => {
    const user = userEvent.setup()
    createAnalysis()
    renderAnalysis('/analysis/analysis-test')

    const startButton = screen.getByRole('button', { name: '开始延伸' })
    expect(startButton).toBeDisabled()

    expect(screen.queryByText('分析模式')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '归纳主题' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '寻找分歧' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '生成建议' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '自定义分析' })).not.toBeInTheDocument()

    await user.type(screen.getByLabelText('想延伸或查询的问题'), '这个想法还能怎么展开')
    expect(startButton).toBeEnabled()
  })

  it('starts AI extension and renders the direct response when generation completes', async () => {
    const user = userEvent.setup()
    createAnalysis()
    renderAnalysis('/analysis/analysis-test')

    await user.type(screen.getByLabelText('想延伸或查询的问题'), '归纳阅读沉淀机会')
    await user.click(screen.getByRole('button', { name: '开始延伸' }))

    expect(screen.getByText('正在延伸想法')).toBeInTheDocument()
    await waitFor(
      () => {
        expect(screen.getByRole('heading', { name: '直接回应' })).toBeInTheDocument()
      },
      { timeout: 1800 },
    )
  })

  it('shows a controlled failure and allows retrying successfully', async () => {
    const user = userEvent.setup()
    createAnalysis()
    renderAnalysis('/analysis/analysis-test')

    await user.type(screen.getByLabelText('想延伸或查询的问题'), '找到这个想法里不清楚的概念')
    await user.click(screen.getByRole('button', { name: '演示查询失败' }))

    await waitFor(
      () => {
        expect(screen.getByText('AI 延伸服务暂时不可用')).toBeInTheDocument()
      },
      { timeout: 1800 },
    )

    await user.click(screen.getByRole('button', { name: '重试' }))
    await waitFor(
      () => {
        expect(screen.getByRole('heading', { name: '直接回应' })).toBeInTheDocument()
      },
      { timeout: 1800 },
    )
  })

  it('cancels a running analysis while preserving the goal and materials', async () => {
    const user = userEvent.setup()
    const analysis = createAnalysis()
    renderAnalysis('/analysis/analysis-test')

    await user.type(screen.getByLabelText('想延伸或查询的问题'), '保留取消后的上下文')
    await user.click(screen.getByRole('button', { name: '开始延伸' }))
    await user.click(screen.getByRole('button', { name: '取消延伸' }))

    expect(screen.getByText('已取消，本次材料和目标已保留')).toBeInTheDocument()
    expect(screen.getByLabelText('想延伸或查询的问题')).toHaveValue('保留取消后的上下文')
    expect(
      useIdeaCardStore.getState().analyses.find((item) => item.id === analysis.id),
    ).toMatchObject({
      status: 'cancelled',
      goal: '保留取消后的上下文',
      cardIds: analysis.cardIds,
    })
    expect(screen.getByRole('button', { name: '开始延伸' })).toBeEnabled()
  })

  it('removes a material from the current analysis without deleting the card', async () => {
    const user = userEvent.setup()
    const analysis = createAnalysis()
    const cardToRemove = useIdeaCardStore.getState().cards[0]
    renderAnalysis('/analysis/analysis-test')

    expect(screen.getByText('参考材料（3）')).toBeInTheDocument()
    await user.click(
      screen.getByRole('button', { name: `移除材料：${cardToRemove.title}` }),
    )

    expect(screen.getByText('参考材料（2）')).toBeInTheDocument()
    expect(
      useIdeaCardStore.getState().cards.some((card) => card.id === cardToRemove.id),
    ).toBe(true)
    expect(
      useIdeaCardStore.getState().analyses.find((item) => item.id === analysis.id)
        ?.cardIds,
    ).not.toContain(cardToRemove.id)
  })

  it('highlights the source material when a result reference is clicked', async () => {
    const user = userEvent.setup()
    createAnalysis()
    renderAnalysis('/analysis/analysis-test')

    await user.type(screen.getByLabelText('想延伸或查询的问题'), '定位引用来源')
    await user.click(screen.getByRole('button', { name: '开始延伸' }))
    await waitFor(
      () => {
        expect(screen.getByRole('heading', { name: '直接回应' })).toBeInTheDocument()
      },
      { timeout: 1800 },
    )

    const materialList = screen.getByRole('list', { name: '参考材料列表' })
    const firstMaterial = useIdeaCardStore.getState().cards[0]
    await user.click(
      screen.getByRole('button', {
        name: `来源卡片：${firstMaterial.title}`,
      }),
    )

    expect(
      within(materialList).getByRole('button', {
        name: `查看材料：${firstMaterial.title}`,
      }),
    ).toHaveAttribute('aria-current', 'true')
  })

  it('adds a follow-up to a completed analysis', async () => {
    const user = userEvent.setup()
    const analysis = createCompletedAnalysis()
    renderAnalysis('/analysis/analysis-test')

    await user.type(
      screen.getByLabelText('追问内容'),
      '最值得优先验证的建议是什么？',
    )
    await user.selectOptions(screen.getByLabelText('引用材料'), analysis.cardIds[0])
    await user.click(screen.getByRole('button', { name: '提交追问' }))

    await waitFor(() => {
      expect(screen.getByText('优先验证快速记录后的回访处理率。')).toBeInTheDocument()
    })
    expect(
      useIdeaCardStore.getState().analyses.find((item) => item.id === analysis.id)
        ?.followUps,
    ).toEqual([
      expect.objectContaining({
        question: '最值得优先验证的建议是什么？',
        answer: '优先验证快速记录后的回访处理率。',
        cardId: analysis.cardIds[0],
      }),
    ])
  })

  it('keeps insight saving disabled while a follow-up answer is being generated', async () => {
    const user = userEvent.setup()
    createCompletedAnalysis()
    renderAnalysis('/analysis/analysis-test')

    await user.type(
      screen.getByLabelText('追问内容'),
      '最值得优先验证的建议是什么？',
    )
    await user.click(screen.getByRole('button', { name: '提交追问' }))

    expect(screen.getByRole('button', { name: '保存为结论' })).toBeDisabled()
    await waitFor(() => {
      expect(screen.getByText('优先验证快速记录后的回访处理率。')).toBeInTheDocument()
    })
    expect(screen.getByRole('button', { name: '保存为结论' })).toBeEnabled()
  })

  it('locks reanalysis controls while a follow-up answer is being generated', async () => {
    const user = userEvent.setup()
    createCompletedAnalysis()
    renderAnalysis('/analysis/analysis-test')

    await user.type(
      screen.getByLabelText('追问内容'),
      '最值得优先验证的建议是什么？',
    )
    await user.click(screen.getByRole('button', { name: '提交追问' }))

    expect(screen.getByRole('button', { name: '开始延伸' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '演示查询失败' })).toBeDisabled()
    expect(screen.queryByRole('button', { name: '生成建议' })).not.toBeInTheDocument()
    await waitFor(() => {
      expect(screen.getByText('优先验证快速记录后的回访处理率。')).toBeInTheDocument()
    })
    expect(screen.getByRole('button', { name: '开始延伸' })).toBeEnabled()
  })

  it('does not write a delayed follow-up after leaving the analysis page', async () => {
    const user = userEvent.setup()
    const analysis = createCompletedAnalysis()
    const router = renderAnalysis('/analysis/analysis-test')

    await user.type(
      screen.getByLabelText('追问内容'),
      '最值得优先验证的建议是什么？',
    )
    await user.click(screen.getByRole('button', { name: '提交追问' }))
    await router.navigate('/')
    await new Promise((resolve) => window.setTimeout(resolve, 600))

    expect(router.state.location.pathname).toBe('/')
    expect(
      useIdeaCardStore.getState().analyses.find((item) => item.id === analysis.id)
        ?.followUps,
    ).toEqual([])
  })

  it('saves a completed analysis as an insight and navigates to it', async () => {
    const user = userEvent.setup()
    createCompletedAnalysis()
    const router = renderAnalysis('/analysis/analysis-test')

    await user.click(screen.getByRole('button', { name: '保存为结论' }))

    await waitFor(() => {
      expect(router.state.location.pathname).toMatch(/^\/insights\//)
    })
    expect(screen.getByRole('heading', { name: '阅读后的思考沉淀' })).toBeInTheDocument()
    expect(useIdeaCardStore.getState().insights).toHaveLength(1)
  })
})
