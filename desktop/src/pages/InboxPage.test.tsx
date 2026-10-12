import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { App } from '../app/App'
import type { Card } from '../entities/card/model'
import { documentFromPlainText } from '../entities/document/model'
import type { DesktopService } from '../services/desktop/contracts'
import { DesktopServiceProvider } from '../services/desktop/context'
import { useDesktopStore } from '../store/useDesktopStore'
import { InboxPage } from './InboxPage'

const personalCard: Card = {
  id: 'card-1',
  title: '产品验证问题',
  originalContent: '先验证收集流程，再观察后续处理率。',
  note: '',
  sourceType: 'personal',
  captureMethod: 'quick',
  sourceName: '快速记录',
  sourceUri: '',
  status: 'unprocessed',
  attachments: [],
  createdAt: '2026-07-14T10:00:00Z',
  updatedAt: '2026-07-14T10:00:00Z',
  archivedAt: null,
}

const documentCard: Card = {
  ...personalCard,
  id: 'card-2',
  title: '需求方案摘录',
  originalContent: '文件材料中的关键结论。',
  sourceType: 'document',
  captureMethod: 'file',
  sourceName: '需求方案.pdf',
  updatedAt: '2026-07-14T11:00:00Z',
}

const legacyWebCard: Card = {
  ...documentCard,
  id: 'card-web-legacy',
  title: '历史网页摘录',
  sourceType: 'web',
  sourceName: 'example.com',
  sourceUri: 'https://example.com/article',
}

const archivedCard: Card = {
  ...personalCard,
  id: 'card-archived',
  title: '已归档的旧想法',
  originalContent: '已经处理完，暂时移出收集箱。',
  status: 'archived',
  archivedAt: '2026-07-15T09:00:00Z',
  updatedAt: '2026-07-15T09:00:00Z',
}

function createService(overrides: Partial<DesktopService> = {}): DesktopService {
  return {
    listCards: vi.fn().mockResolvedValue([personalCard, documentCard]),
    getCard: vi.fn().mockResolvedValue(personalCard),
    createCard: vi.fn().mockResolvedValue({ card: personalCard, duplicateCardIds: [] }),
    createRichCard: vi.fn().mockResolvedValue({ card: personalCard, duplicateCardIds: [] }),
    getCardDocument: vi.fn().mockImplementation(async (cardId) => ({
      cardId,
      document: documentFromPlainText(personalCard.originalContent),
      schemaVersion: 1,
      updatedAt: personalCard.updatedAt,
    })),
    saveCardDocument: vi.fn(),
    loadInlineImage: vi.fn(),
    listConcepts: vi.fn().mockResolvedValue([]),
    getConcept: vi.fn(),
    deleteConcept: vi.fn(),
    listCardConcepts: vi.fn().mockResolvedValue([]),
    saveConceptAnnotation: vi.fn(),
    getAiSettings: vi.fn(),
    saveAiSettings: vi.fn(),
    getConceptAssistantSettings: vi.fn(),
    saveConceptAssistantSettings: vi.fn(),
    generateCardTitle: vi.fn(),
    getTitleGenerationSettings: vi.fn(),
    saveTitleGenerationSettings: vi.fn(),
    removeAiKey: vi.fn(),
    testAiConnection: vi.fn(),
    listCardAiMessages: vi.fn().mockResolvedValue([]),
    sendCardAiMessage: vi.fn(),
    regenerateCardAiMessage: vi.fn(),
    explainCardConcept: vi.fn(),
    cancelAiRequest: vi.fn(),
    openExternalUrl: vi.fn(),
    updateCard: vi.fn().mockImplementation(async (id, patch) => ({
      ...personalCard,
      ...patch,
      id,
    })),
    archiveCard: vi.fn().mockResolvedValue(archivedCard),
    restoreCard: vi.fn().mockResolvedValue(personalCard),
    deleteCard: vi.fn().mockResolvedValue(undefined),
    readClipboardDraft: vi.fn(),
    saveCaptureDraft: vi.fn(),
    selectAndImportFiles: vi.fn().mockResolvedValue([]),
    captureScreenshot: vi.fn().mockResolvedValue({ status: 'cancelled' }),
    getCaptureStatus: vi.fn(),
    revealAttachment: vi.fn().mockResolvedValue(undefined),
    readClipboardText: vi.fn().mockResolvedValue(null),
    writeClipboardText: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  }
}

function renderInbox(service: DesktopService) {
  return render(
    <DesktopServiceProvider service={service}>
      <InboxPage />
    </DesktopServiceProvider>,
  )
}

describe('InboxPage', () => {
  beforeEach(() => {
    useDesktopStore.setState({
      cards: [],
      selectedCardId: null,
      duplicateCardIds: [],
      isLoading: false,
      error: null,
      errorOperation: null,
    })
  })

  it('exposes two source types and groups legacy web cards under documents', async () => {
    const user = userEvent.setup()
    renderInbox(createService({
      listCards: vi.fn().mockResolvedValue([personalCard, documentCard, legacyWebCard]),
    }))
    const cardList = within(screen.getByLabelText('卡片列表'))

    expect(await cardList.findByText('产品验证问题')).toBeInTheDocument()
    expect(cardList.getByText('需求方案摘录')).toBeInTheDocument()
    expect(cardList.getByText('历史网页摘录')).toBeInTheDocument()
    for (const label of ['个人记录', '文档资料']) {
      expect(screen.getByRole('button', { name: label })).toBeInTheDocument()
    }
    expect(screen.queryByRole('button', { name: '网页文章' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '图片材料' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '音视频' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '文档资料' }))

    expect(cardList.queryByText('产品验证问题')).not.toBeInTheDocument()
    expect(cardList.getByText('需求方案摘录')).toBeInTheDocument()
    expect(cardList.getByText('历史网页摘录')).toBeInTheDocument()
    expect(screen.queryByText('标签')).not.toBeInTheDocument()
  })

  it('keeps accessible region names while removing redundant chrome labels and refresh', async () => {
    render(<App service={createService()} />)

    await within(screen.getByLabelText('卡片列表')).findByText('产品验证问题')
    expect(screen.getByRole('navigation', { name: '工作区' })).toBeInTheDocument()
    expect(screen.getByRole('region', { name: '来源' })).toBeInTheDocument()
    expect(screen.queryByText('工作区')).not.toBeInTheDocument()
    expect(screen.queryByText('统一收集')).not.toBeInTheDocument()
    expect(screen.queryByText('归档内容')).not.toBeInTheDocument()
    expect(screen.queryByText('本地工作区')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '刷新卡片' })).not.toBeInTheDocument()
  })

  it('switches between inbox and archive workspaces with separate counts and actions', async () => {
    const user = userEvent.setup()
    const service = createService({
      listCards: vi.fn().mockResolvedValue([personalCard, documentCard, archivedCard]),
    })
    render(<App service={service} />)

    const navigation = screen.getByRole('navigation', { name: '工作区' })
    expect(await within(navigation).findByRole('button', { name: '收集箱 2' })).toHaveAttribute(
      'aria-current',
      'page',
    )
    expect(within(navigation).getByRole('button', { name: '已归档 1' })).toBeInTheDocument()
    const cardList = within(screen.getByLabelText('卡片列表'))
    expect(cardList.getByText('产品验证问题')).toBeInTheDocument()
    expect(cardList.queryByText('已归档的旧想法')).not.toBeInTheDocument()

    await user.click(within(navigation).getByRole('button', { name: '已归档 1' }))

    expect(screen.getByRole('heading', { name: '已归档' })).toBeInTheDocument()
    expect(cardList.getByText('已归档的旧想法')).toBeInTheDocument()
    expect(cardList.queryByText('产品验证问题')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '快速记录' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '更多捕获方式' })).not.toBeInTheDocument()
    expect(screen.queryByText('待整理')).not.toBeInTheDocument()
    expect(screen.queryByText('已整理')).not.toBeInTheDocument()
    expect(within(navigation).getByRole('button', { name: '已归档 1' })).toHaveAttribute(
      'aria-current',
      'page',
    )
  })

  it('opens the concept collection from the workspace navigation', async () => {
    const user = userEvent.setup()
    const concept = {
      id: 'concept-1',
      name: '费曼学习法',
      explanation: '通过讲解检验理解。',
      mentionCount: 1,
      updatedAt: '2026-07-19T10:00:00Z',
    }
    const service = createService({
      listConcepts: vi.fn().mockResolvedValue([concept]),
      getConcept: vi.fn().mockResolvedValue({
        ...concept,
        mentions: [{
          mentionId: 'mention-1',
          cardId: personalCard.id,
          cardTitle: personalCard.title,
          selectedText: '费曼学习法',
          cardStatus: 'unprocessed',
          createdAt: '2026-07-19T10:00:00Z',
        }],
      }),
    })
    render(<App service={service} />)

    const navigation = screen.getByRole('navigation', { name: '工作区' })
    await user.click(await within(navigation).findByRole('button', { name: '概念集合' }))

    expect(await screen.findByRole('heading', { name: '概念集合' })).toBeInTheDocument()
    expect((await screen.findAllByText('费曼学习法')).length).toBeGreaterThan(0)
    expect(service.listConcepts).toHaveBeenCalled()
  })

  it('shows workspace-specific empty guidance', async () => {
    renderInbox(createService({ listCards: vi.fn().mockResolvedValue([]) }))

    expect(await screen.findByText('还没有卡片，先快速记录一个想法。')).toBeInTheDocument()
  })

  it('archives the selected inbox card and moves it out of the workspace', async () => {
    const user = userEvent.setup()
    const service = createService({
      listCards: vi.fn().mockResolvedValue([personalCard, documentCard]),
      archiveCard: vi.fn().mockResolvedValue({
        ...personalCard,
        status: 'archived',
        archivedAt: '2026-07-19T10:00:00Z',
      }),
    })
    render(<App service={service} />)
    const cardList = within(screen.getByLabelText('卡片列表'))
    expect(await cardList.findByText('产品验证问题')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '归档' }))

    await waitFor(() => expect(service.archiveCard).toHaveBeenCalledWith('card-1'))
    expect(cardList.queryByText('产品验证问题')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '收集箱 1' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '已归档 1' })).toBeInTheDocument()
  })

  it('restores an archived card back to the inbox', async () => {
    const user = userEvent.setup()
    const service = createService({
      listCards: vi.fn().mockResolvedValue([personalCard, archivedCard]),
      restoreCard: vi.fn().mockResolvedValue({
        ...archivedCard,
        status: 'annotated',
        archivedAt: null,
      }),
    })
    render(<App service={service} />)
    await screen.findByRole('button', { name: '已归档 1' })
    await user.click(screen.getByRole('button', { name: '已归档 1' }))
    expect(await screen.findByRole('heading', { name: '已归档的旧想法' })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '恢复' }))

    await waitFor(() => expect(service.restoreCard).toHaveBeenCalledWith('card-archived'))
    expect(screen.getByText('还没有归档卡片。')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '收集箱 2' })).toBeInTheDocument()
  })

  it('deletes an archived card only after confirmation', async () => {
    const user = userEvent.setup()
    const service = createService({
      listCards: vi.fn().mockResolvedValue([personalCard, archivedCard]),
    })
    render(<App service={service} />)
    await screen.findByRole('button', { name: '已归档 1' })
    await user.click(screen.getByRole('button', { name: '已归档 1' }))
    await screen.findByRole('heading', { name: '已归档的旧想法' })

    await user.click(screen.getByRole('button', { name: '永久删除' }))

    const dialog = screen.getByRole('dialog', { name: '永久删除卡片' })
    expect(service.deleteCard).not.toHaveBeenCalled()
    await user.click(within(dialog).getByRole('button', { name: '永久删除' }))

    await waitFor(() => expect(service.deleteCard).toHaveBeenCalledWith('card-archived'))
    expect(screen.queryByRole('dialog', { name: '永久删除卡片' })).not.toBeInTheDocument()
    expect(screen.getByText('还没有归档卡片。')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '已归档 0' })).toBeInTheDocument()
  })

  it('deletes an inbox card only after confirmation', async () => {
    const user = userEvent.setup()
    const service = createService({
      listCards: vi.fn().mockResolvedValue([personalCard, documentCard]),
    })
    render(<App service={service} />)
    const cardList = within(screen.getByLabelText('卡片列表'))
    await cardList.findByText('产品验证问题')

    await user.click(screen.getByRole('button', { name: '删除' }))

    const dialog = screen.getByRole('dialog', { name: '永久删除卡片' })
    expect(service.deleteCard).not.toHaveBeenCalled()
    await user.click(within(dialog).getByRole('button', { name: '永久删除' }))

    await waitFor(() => expect(service.deleteCard).toHaveBeenCalledWith('card-1'))
    expect(screen.queryByRole('dialog', { name: '永久删除卡片' })).not.toBeInTheDocument()
    expect(cardList.queryByText('产品验证问题')).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: '需求方案摘录' })).toBeInTheDocument()
  })

  it('shows only automatic organization statuses and has no status editor', async () => {
    const legacyCards: Card[] = [
      personalCard,
      { ...personalCard, id: 'card-annotated', title: '已整理卡片', status: 'annotated' },
      {
        ...personalCard,
        id: 'card-pending',
        title: '历史待分析卡片',
        status: 'pending_analysis',
      },
      {
        ...personalCard,
        id: 'card-concluded',
        title: '历史结论卡片',
        status: 'concluded',
      },
    ]
    renderInbox(createService({ listCards: vi.fn().mockResolvedValue(legacyCards) }))
    const cardList = within(screen.getByLabelText('卡片列表'))

    expect(await cardList.findAllByText('待整理')).toHaveLength(1)
    expect(cardList.getAllByText('已整理')).toHaveLength(3)
    expect(screen.queryByLabelText('状态')).not.toBeInTheDocument()
    expect(screen.queryByText('待分析')).not.toBeInTheDocument()
    expect(screen.queryByText('已生成结论')).not.toBeInTheDocument()
    expect(screen.queryByText('已归档')).not.toBeInTheDocument()
  })

  it('keeps capture actions out of source filters and opens them from the toolbar', async () => {
    const user = userEvent.setup()
    const service = createService({
      readClipboardDraft: vi.fn().mockResolvedValue({
        kind: 'clipboard',
        title: '剪贴板内容',
        content: '需要保存的临时材料',
        note: '',
        sourceType: 'personal',
        sourceName: '剪贴板',
        sourceUri: '',
        temporaryAttachmentIds: [],
      }),
    })
    renderInbox(service)
    await within(screen.getByLabelText('卡片列表')).findByText('产品验证问题')
    const sourceFilter = within(screen.getByRole('region', { name: '来源' }))

    expect(sourceFilter.queryByRole('button', { name: '剪贴板' })).not.toBeInTheDocument()
    expect(sourceFilter.queryByRole('button', { name: '导入文件' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '更多捕获方式' }))
    expect(screen.getByRole('menuitem', { name: '导入本地文件' })).toBeInTheDocument()
    await user.click(screen.getByRole('menuitem', { name: '从剪贴板捕获' }))

    expect(service.readClipboardDraft).toHaveBeenCalledTimes(1)
    expect(await screen.findByRole('dialog', { name: '确认捕获' })).toBeInTheDocument()
  })

  it('creates a quick card with an optional note and selects it', async () => {
    const user = userEvent.setup()
    const createdCard: Card = {
      ...personalCard,
      id: 'card-created',
      title: '先验证收集流程',
      originalContent: '先验证收集流程',
      note: '观察一周',
      updatedAt: '2026-07-14T12:00:00Z',
    }
    const service = createService({
      createRichCard: vi.fn().mockResolvedValue({ card: createdCard, duplicateCardIds: [] }),
    })
    renderInbox(service)
    await within(screen.getByLabelText('卡片列表')).findByText('产品验证问题')

    await user.click(screen.getByRole('button', { name: '快速记录' }))
    const dialog = screen.getByRole('dialog', { name: '快速记录' })
    fireEvent.input(within(dialog).getByLabelText('记录内容'), {
      target: { textContent: '先验证收集流程' },
    })
    fireEvent.change(within(dialog).getByLabelText('我的想法'), {
      target: { value: '观察一周' },
    })
    await waitFor(() => {
      expect(within(dialog).getByRole('button', { name: '保存' })).toBeEnabled()
    })
    await user.click(within(dialog).getByRole('button', { name: '保存' }))

    expect(service.createRichCard).toHaveBeenCalledTimes(1)
    expect(service.createRichCard).toHaveBeenCalledWith({
      title: '',
      document: documentFromPlainText('先验证收集流程'),
      note: '观察一周',
      sourceType: 'personal',
      captureMethod: 'quick',
      sourceName: '',
      sourceUri: '',
    })
    expect(await screen.findByRole('heading', { name: '先验证收集流程' })).toBeInTheDocument()
    expect(screen.queryByRole('dialog', { name: '快速记录' })).not.toBeInTheDocument()
  })

  it('edits note and source information from detail', async () => {
    const user = userEvent.setup()
    const service = createService()
    renderInbox(service)
    const cardList = within(screen.getByLabelText('卡片列表'))
    await cardList.findByText('产品验证问题')

    await user.click(cardList.getByRole('button', { name: /产品验证问题/ }))
    await user.type(screen.getByLabelText('我的想法'), '补充判断')
    await user.selectOptions(screen.getByLabelText('来源类型'), 'document')
    await user.clear(screen.getByLabelText('来源名称'))
    await user.type(screen.getByLabelText('来源名称'), '方案.pdf')
    expect(screen.queryByRole('button', { name: '保存修改' })).not.toBeInTheDocument()

    await waitFor(() => {
      expect(service.updateCard).toHaveBeenCalledWith(
        'card-1',
        expect.objectContaining({
          note: '补充判断',
          sourceType: 'document',
          sourceName: '方案.pdf',
        }),
      )
    }, { timeout: 2_500 })
    expect(vi.mocked(service.updateCard).mock.calls[0]?.[1]).not.toHaveProperty('status')
  })

  it('refreshes the detail note when selecting another card', async () => {
    const user = userEvent.setup()
    const screenshotCard: Card = {
      ...documentCard,
      id: 'screenshot-card',
      title: '截图 2026-07-28 11:16:19',
      note: '截图时记录的想法',
      captureMethod: 'screenshot',
      sourceType: 'personal',
      sourceName: '区域截图',
    }
    const service = createService({
      listCards: vi.fn().mockResolvedValue([personalCard, screenshotCard]),
    })
    renderInbox(service)
    const cardList = within(screen.getByLabelText('卡片列表'))

    await cardList.findByText('产品验证问题')
    expect(screen.getByLabelText('我的想法')).toHaveValue('')

    await user.type(screen.getByLabelText('我的想法'), '切换前保存')
    await user.click(cardList.getByRole('button', { name: /截图 2026-07-28 11:16:19/ }))

    await waitFor(() => {
      expect(service.updateCard).toHaveBeenCalledWith(
        'card-1',
        expect.objectContaining({ note: '切换前保存' }),
      )
    })
    expect(screen.getByLabelText('我的想法')).toHaveValue('截图时记录的想法')
  })

  it('retries the initial load after a failure without exposing a permanent refresh action', async () => {
    const user = userEvent.setup()
    const service = createService({
      listCards: vi
        .fn()
        .mockRejectedValueOnce({
          code: 'database_operation_failed',
          message: '数据库操作失败',
          recovery: '重新打开应用',
        })
        .mockResolvedValueOnce([personalCard]),
    })
    renderInbox(service)

    expect(await screen.findByRole('alert')).toHaveTextContent('数据库操作失败')
    expect(screen.getByRole('alert')).toHaveTextContent('重新打开应用')
    await user.click(screen.getByRole('button', { name: '重试' }))

    expect(await within(screen.getByLabelText('卡片列表')).findByText('产品验证问题'))
      .toBeInTheDocument()
    expect(service.listCards).toHaveBeenCalledTimes(2)
    expect(screen.queryByRole('button', { name: '刷新卡片' })).not.toBeInTheDocument()
  })

  it('shows create failures only inside the quick capture dialog', async () => {
    const user = userEvent.setup()
    const service = createService({
      createRichCard: vi.fn().mockRejectedValue({
        code: 'database_operation_failed',
        message: '保存失败',
        recovery: '重新打开应用',
      }),
    })
    renderInbox(service)
    await within(screen.getByLabelText('卡片列表')).findByText('产品验证问题')
    await user.click(screen.getByRole('button', { name: '快速记录' }))
    const dialog = screen.getByRole('dialog', { name: '快速记录' })
    fireEvent.input(within(dialog).getByLabelText('记录内容'), {
      target: { textContent: '保留这段草稿' },
    })
    await waitFor(() => {
      expect(within(dialog).getByRole('button', { name: '保存' })).toBeEnabled()
    })

    await user.click(within(dialog).getByRole('button', { name: '保存' }))

    expect(await within(dialog).findByRole('alert')).toHaveTextContent('保存失败')
    expect(screen.getAllByRole('alert')).toHaveLength(1)
  })
})
