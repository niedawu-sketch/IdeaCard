import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { Card } from '../entities/card/model'
import { useIdeaCardStore } from '../store/useIdeaCardStore'
import { InboxPage } from './InboxPage'

function renderInbox(initialEntry = '/') {
  const router = createMemoryRouter(
    [
      { path: '/', element: <InboxPage /> },
      {
        path: '/analysis/:analysisId',
        element: <div>分析工作台占位</div>,
      },
    ],
    { initialEntries: [initialEntry] },
  )

  render(<RouterProvider router={router} />)
  return router
}

function createCards(count: number): Card[] {
  const template = useIdeaCardStore.getState().cards[0]

  return Array.from({ length: count }, (_, index) => ({
    ...template,
    id: `card-bulk-${index + 1}`,
    title: `批量卡片 ${index + 1}`,
    content: `批量内容 ${index + 1}`,
    note: '',
    tags: [...template.tags],
    createdAt: `2026-01-01T00:${String(index).padStart(2, '0')}:00.000Z`,
    updatedAt: `2026-01-01T00:${String(index).padStart(2, '0')}:00.000Z`,
  }))
}

describe('InboxPage', () => {
  beforeEach(() => {
    localStorage.clear()
    useIdeaCardStore.getState().reset()
  })

  afterEach(() => {
    cleanup()
    localStorage.clear()
  })

  it('searches card text and opens the seeded note', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.type(
      screen.getByPlaceholderText('搜索标题、内容或我的想法'),
      '判断',
    )

    expect(screen.getByText('从信息到判断的距离')).toBeInTheDocument()
    expect(screen.queryByText('捕获动作的摩擦')).not.toBeInTheDocument()

    await user.click(screen.getByText('从信息到判断的距离'))

    expect(screen.getByLabelText('我的想法')).toHaveValue(
      '需要把记录后的加工动作放在主流程里。',
    )
    expect(screen.getByText('网页文章', { selector: 'dd' })).toBeInTheDocument()
    expect(screen.getByText('公众号文章', { selector: 'dd' })).toBeInTheDocument()
  })

  it('saves note edits on blur and annotates an unprocessed card', async () => {
    const user = userEvent.setup()
    renderInbox('/?card=card-citation')

    const note = screen.getByLabelText('我的想法')
    await user.type(note, '需要保留可追溯的引用。')
    await user.tab()

    expect(
      useIdeaCardStore.getState().cards.find(
        (card) => card.id === 'card-citation',
      ),
    ).toMatchObject({
      note: '需要保留可追溯的引用。',
      status: 'annotated',
    })
  })

  it('does not show tag filters or tag editing in the card detail', () => {
    renderInbox('/?card=card-reading-distance')

    expect(screen.queryAllByText('标签')).toHaveLength(0)
    expect(
      screen.queryByRole('button', { name: '全部标签' }),
    ).not.toBeInTheDocument()
  })

  it('marks a card pending analysis and shows the updated status', async () => {
    const user = userEvent.setup()
    renderInbox('/?card=card-citation')

    await user.click(screen.getByRole('button', { name: '标记待分析' }))

    expect(
      useIdeaCardStore.getState().cards.find(
        (card) => card.id === 'card-citation',
      )?.status,
    ).toBe('pending-analysis')
    expect(screen.getAllByText('待分析').length).toBeGreaterThan(0)
  })

  it('archives a card, clears the detail, and removes it from a status-filtered list', async () => {
    const user = userEvent.setup()
    const router = renderInbox(
      '/?status=pending-analysis&card=card-reading-distance',
    )

    await user.click(screen.getByRole('button', { name: '归档' }))

    expect(
      useIdeaCardStore.getState().cards.find(
        (card) => card.id === 'card-reading-distance',
      )?.status,
    ).toBe('archived')
    expect(screen.getByText('选择一张卡片开始处理')).toBeInTheDocument()
    expect(screen.queryByText('从信息到判断的距离')).not.toBeInTheDocument()
    expect(screen.getByText('先收集再整理')).toBeInTheDocument()
    expect(router.state.location.search).toBe('?status=pending-analysis')
  })

  it('opens a card from the initial query and preserves status when another card opens', async () => {
    const user = userEvent.setup()
    const router = renderInbox(
      '/?status=pending-analysis&card=card-reading-distance',
    )

    expect(screen.getByLabelText('我的想法')).toHaveValue(
      '需要把记录后的加工动作放在主流程里。',
    )

    await user.click(screen.getByText('先收集再整理'))

    expect(router.state.location.search).toContain(
      'status=pending-analysis',
    )
    expect(router.state.location.search).toContain(
      'card=card-inbox-principle',
    )
  })

  it('shows all five fixed source filters', () => {
    renderInbox()

    for (const label of [
      '个人记录',
      '网页文章',
      '文档资料',
      '图片材料',
      '音视频',
    ]) {
      expect(screen.getByRole('button', { name: label })).toBeInTheDocument()
    }
  })

  it('filters the inbox to personal cards', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('button', { name: '个人记录' }))
    expect(screen.getByText('捕获动作的摩擦')).toBeInTheDocument()
    expect(screen.getByText('AI 结论需要来源')).toBeInTheDocument()
    expect(screen.getByText('阅读后的思考沉淀')).toBeInTheDocument()
    expect(screen.queryByText('从信息到判断的距离')).not.toBeInTheDocument()
    expect(screen.queryByText('先收集再整理')).not.toBeInTheDocument()
    expect(screen.queryByText('用户决定分析上下文')).not.toBeInTheDocument()
  })

  it('filters the inbox to document cards', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('button', { name: '文档资料' }))

    expect(screen.getByText('用户决定分析上下文')).toBeInTheDocument()
    expect(screen.queryByText('从信息到判断的距离')).not.toBeInTheDocument()
    expect(screen.queryByText('捕获动作的摩擦')).not.toBeInTheDocument()
    expect(screen.queryByText('AI 结论需要来源')).not.toBeInTheDocument()
    expect(screen.queryByText('先收集再整理')).not.toBeInTheDocument()
    expect(screen.queryByText('阅读后的思考沉淀')).not.toBeInTheDocument()
  })

  it('filters the inbox to web article cards', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('button', { name: '网页文章' }))

    expect(screen.getByText('从信息到判断的距离')).toBeInTheDocument()
    expect(screen.queryByText('捕获动作的摩擦')).not.toBeInTheDocument()
    expect(screen.queryByText('先收集再整理')).not.toBeInTheDocument()
    expect(screen.queryByText('用户决定分析上下文')).not.toBeInTheDocument()
    expect(screen.queryByText('阅读后的思考沉淀')).not.toBeInTheDocument()
  })

  it('filters by source type when source names are identical', async () => {
    const user = userEvent.setup()
    const template = useIdeaCardStore.getState().cards[0]
    useIdeaCardStore.setState({
      cards: [
        {
          ...template,
          id: 'card-same-source-web',
          title: '同名来源网页卡片',
          sourceName: '同一来源',
          sourceType: 'web',
        },
        {
          ...template,
          id: 'card-same-source-personal',
          title: '同名来源个人卡片',
          sourceName: '同一来源',
          sourceType: 'personal',
        },
      ],
    })
    renderInbox()

    await user.click(screen.getByRole('button', { name: '网页文章' }))

    expect(screen.getByText('同名来源网页卡片')).toBeInTheDocument()
    expect(screen.queryByText('同名来源个人卡片')).not.toBeInTheDocument()
  })

  it('filters the inbox to image material cards', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('button', { name: '图片材料' }))

    expect(screen.getByText('先收集再整理')).toBeInTheDocument()
    expect(screen.queryByText('捕获动作的摩擦')).not.toBeInTheDocument()
    expect(screen.queryByText('从信息到判断的距离')).not.toBeInTheDocument()
    expect(screen.queryByText('用户决定分析上下文')).not.toBeInTheDocument()
    expect(screen.queryByText('阅读后的思考沉淀')).not.toBeInTheDocument()
  })

  it('shows an empty message for audio and video cards', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('button', { name: '音视频' }))

    expect(screen.getByText('没有匹配的卡片')).toBeInTheDocument()
  })

  it('shows a concise empty message when search has no results', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.type(
      screen.getByPlaceholderText('搜索标题、内容或我的想法'),
      '不存在的关键词',
    )

    expect(screen.getByText('没有匹配的卡片')).toBeInTheDocument()
  })

  it('creates an analysis set from two selected cards and navigates to it', async () => {
    const user = userEvent.setup()
    const router = renderInbox()

    await user.click(screen.getByRole('checkbox', {
      name: '选择「从信息到判断的距离」',
    }))
    await user.click(screen.getByRole('checkbox', {
      name: '选择「捕获动作的摩擦」',
    }))

    expect(screen.getByText('已选择 2 张卡片')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '创建分析集' }))

    const analysis = useIdeaCardStore.getState().analyses[0]
    expect(analysis).toMatchObject({
      title: '未命名分析',
      cardIds: ['card-reading-distance', 'card-capture-friction'],
      status: 'draft',
    })
    expect(router.state.location.pathname).toBe(`/analysis/${analysis.id}`)
  })

  it('shows the single-card analysis label when one card is selected', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('checkbox', {
      name: '选择「从信息到判断的距离」',
    }))

    expect(screen.getByText('已选择 1 张卡片')).toBeInTheDocument()
    expect(screen.getByText('单卡片分析')).toBeInTheDocument()
  })

  it('clears selection and hides the bulk action bar', async () => {
    const user = userEvent.setup()
    renderInbox()

    await user.click(screen.getByRole('checkbox', {
      name: '选择「从信息到判断的距离」',
    }))
    expect(screen.getByText('已选择 1 张卡片')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '清除选择' }))

    expect(useIdeaCardStore.getState().selectedCardIds).toEqual([])
    expect(screen.queryByText('已选择 1 张卡片')).not.toBeInTheDocument()
  })

  it('keeps selection and shows an error instead of navigating when more than 20 cards are selected', async () => {
    const user = userEvent.setup()
    const cards = createCards(21)
    const selectedCardIds = cards.map((card) => card.id)
    useIdeaCardStore.setState({ cards, selectedCardIds })
    const router = renderInbox()

    await user.click(screen.getByRole('button', { name: '创建分析集' }))

    expect(screen.getByText('单次分析最多选择 20 张卡片')).toBeInTheDocument()
    expect(router.state.location.pathname).toBe('/')
    expect(useIdeaCardStore.getState().analyses).toEqual([])
    expect(useIdeaCardStore.getState().selectedCardIds).toEqual(selectedCardIds)
  })

  it('shows a generic error instead of the limit error when analysis creation fails for another reason', async () => {
    const user = userEvent.setup()
    const originalCreateAnalysisSet =
      useIdeaCardStore.getState().createAnalysisSet
    useIdeaCardStore.setState({
      createAnalysisSet: () => {
        throw new Error('存储写入失败')
      },
    })
    try {
      const router = renderInbox()

      await user.click(screen.getByRole('checkbox', {
        name: '选择「从信息到判断的距离」',
      }))
      await user.click(screen.getByRole('button', { name: '创建分析集' }))

      expect(screen.queryByText('单次分析最多选择 20 张卡片')).not.toBeInTheDocument()
      expect(screen.getByText('创建分析集失败，请稍后再试')).toBeInTheDocument()
      expect(router.state.location.pathname).toBe('/')
    } finally {
      useIdeaCardStore.setState({
        createAnalysisSet: originalCreateAnalysisSet,
      })
    }
  })

  it('clears the limit error when selection drops back to 20 cards', async () => {
    const user = userEvent.setup()
    const cards = createCards(21)
    const selectedCardIds = cards.map((card) => card.id)
    useIdeaCardStore.setState({ cards, selectedCardIds })
    renderInbox()

    await user.click(screen.getByRole('button', { name: '创建分析集' }))
    expect(screen.getByText('单次分析最多选择 20 张卡片')).toBeInTheDocument()

    await user.click(screen.getByRole('checkbox', {
      name: '选择「批量卡片 21」',
    }))

    await waitFor(() => {
      expect(
        screen.queryByText('单次分析最多选择 20 张卡片'),
      ).not.toBeInTheDocument()
    })
    expect(useIdeaCardStore.getState().selectedCardIds).toHaveLength(20)
  })
})
