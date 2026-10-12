import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { Card } from '../entities/card/model'
import { documentFromPlainText } from '../entities/document/model'
import { DesktopError } from '../shared/errors/DesktopError'
import type { DesktopService } from '../services/desktop/contracts'
import { useDesktopStore } from './useDesktopStore'

const firstCard: Card = {
  id: 'card-1',
  title: '第一张卡片',
  originalContent: '第一张卡片正文',
  note: '',
  sourceType: 'personal',
  captureMethod: 'quick',
  sourceName: '',
  sourceUri: '',
  status: 'unprocessed',
  attachments: [],
  createdAt: '2026-07-14T10:00:00Z',
  updatedAt: '2026-07-14T10:00:00Z',
  archivedAt: null,
}

const secondCard: Card = {
  ...firstCard,
  id: 'card-2',
  title: '第二张卡片',
  originalContent: '第二张卡片正文',
  updatedAt: '2026-07-14T11:00:00Z',
}

function createService(overrides: Partial<DesktopService> = {}): DesktopService {
  return {
    listCards: vi.fn().mockResolvedValue([]),
    getCard: vi.fn().mockResolvedValue(firstCard),
    createCard: vi.fn().mockResolvedValue({ card: firstCard, duplicateCardIds: [] }),
    createRichCard: vi.fn().mockResolvedValue({ card: firstCard, duplicateCardIds: [] }),
    getCardDocument: vi.fn(),
    saveCardDocument: vi.fn(),
    loadInlineImage: vi.fn(),
    listConcepts: vi.fn(),
    getConcept: vi.fn(),
    deleteConcept: vi.fn(),
    listCardConcepts: vi.fn(),
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
    updateCard: vi.fn().mockResolvedValue(firstCard),
    archiveCard: vi.fn().mockResolvedValue({ ...firstCard, status: 'archived' }),
    restoreCard: vi.fn().mockResolvedValue(firstCard),
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

describe('useDesktopStore', () => {
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

  it('loads persisted cards and selects the first card', async () => {
    const service = createService({
      listCards: vi.fn().mockResolvedValue([firstCard, secondCard]),
    })

    await useDesktopStore.getState().loadCards(service)

    expect(service.listCards).toHaveBeenCalledWith(true)
    expect(useDesktopStore.getState()).toMatchObject({
      cards: [firstCard, secondCard],
      selectedCardId: 'card-1',
      isLoading: false,
      error: null,
    })
  })

  it('keeps the current selection when that card still exists after refresh', async () => {
    const service = createService({
      listCards: vi.fn().mockResolvedValue([secondCard, firstCard]),
    })
    useDesktopStore.setState({ cards: [firstCard], selectedCardId: 'card-1' })

    await useDesktopStore.getState().loadCards(service)

    expect(useDesktopStore.getState().selectedCardId).toBe('card-1')
  })

  it('creates a quick card, selects it, and retains duplicate matches', async () => {
    const created = { ...secondCard, title: '临时问题', originalContent: '临时问题' }
    const service = createService({
      createRichCard: vi.fn().mockResolvedValue({
        card: created,
        duplicateCardIds: ['card-existing'],
      }),
    })

    const result = await useDesktopStore.getState().createQuickCard(service, {
      title: '',
      originalContent: '临时问题',
      document: documentFromPlainText('临时问题'),
      note: '等待讨论',
    })

    expect(service.createRichCard).toHaveBeenCalledWith({
      title: '',
      note: '等待讨论',
      sourceType: 'personal',
      captureMethod: 'quick',
      sourceName: '',
      sourceUri: '',
      document: documentFromPlainText('临时问题'),
    })
    expect(result).toEqual({ card: created, duplicateCardIds: ['card-existing'] })
    expect(useDesktopStore.getState()).toMatchObject({
      cards: [created],
      selectedCardId: 'card-2',
      duplicateCardIds: ['card-existing'],
      error: null,
    })
  })

  it('replaces the matching card after a persisted update', async () => {
    const updated = { ...firstCard, note: '补充判断', status: 'annotated' as const }
    const service = createService({
      updateCard: vi.fn().mockResolvedValue(updated),
    })
    useDesktopStore.setState({ cards: [firstCard, secondCard], selectedCardId: 'card-1' })

    await useDesktopStore.getState().updateCard(service, 'card-1', { note: '补充判断' })

    expect(service.updateCard).toHaveBeenCalledWith('card-1', { note: '补充判断' })
    expect(useDesktopStore.getState().cards).toEqual([updated, secondCard])
  })

  it('archives and restores a card without removing it from the local collection', async () => {
    const archived = {
      ...firstCard,
      status: 'archived' as const,
      archivedAt: '2026-07-19T00:00:00Z',
    }
    const service = createService({
      archiveCard: vi.fn().mockResolvedValue(archived),
      restoreCard: vi.fn().mockResolvedValue(firstCard),
    })
    useDesktopStore.setState({ cards: [firstCard, secondCard], selectedCardId: firstCard.id })

    await useDesktopStore.getState().archiveCard(service, firstCard.id)
    expect(useDesktopStore.getState().cards).toEqual([archived, secondCard])

    await useDesktopStore.getState().restoreCard(service, firstCard.id)
    expect(useDesktopStore.getState().cards).toEqual([firstCard, secondCard])
  })

  it('permanently deletes a card and selects the next archived sibling', async () => {
    const archivedFirst = {
      ...firstCard,
      status: 'archived' as const,
      archivedAt: '2026-07-19T00:00:00Z',
    }
    const archivedSecond = {
      ...secondCard,
      status: 'archived' as const,
      archivedAt: '2026-07-19T00:01:00Z',
    }
    const service = createService()
    useDesktopStore.setState({
      cards: [archivedFirst, archivedSecond],
      selectedCardId: archivedFirst.id,
    })

    await useDesktopStore.getState().deleteCard(service, archivedFirst.id)

    expect(service.deleteCard).toHaveBeenCalledWith(archivedFirst.id)
    expect(useDesktopStore.getState().cards).toEqual([archivedSecond])
    expect(useDesktopStore.getState().selectedCardId).toBe(archivedSecond.id)
  })

  it('deletes an inbox card and selects the next inbox sibling', async () => {
    const service = createService()
    useDesktopStore.setState({
      cards: [firstCard, secondCard],
      selectedCardId: firstCard.id,
    })

    await useDesktopStore.getState().deleteCard(service, firstCard.id)

    expect(service.deleteCard).toHaveBeenCalledWith(firstCard.id)
    expect(useDesktopStore.getState().cards).toEqual([secondCard])
    expect(useDesktopStore.getState().selectedCardId).toBe(secondCard.id)
  })

  it('keeps the card when a lifecycle operation fails', async () => {
    const service = createService({
      archiveCard: vi.fn().mockRejectedValue({
        code: 'database_operation_failed',
        message: '归档失败',
        recovery: '稍后重试',
      }),
    })
    useDesktopStore.setState({ cards: [firstCard], selectedCardId: firstCard.id })

    await expect(
      useDesktopStore.getState().archiveCard(service, firstCard.id),
    ).rejects.toMatchObject({ code: 'database_operation_failed' })

    expect(useDesktopStore.getState().cards).toEqual([firstCard])
    expect(useDesktopStore.getState().errorOperation).toBe('archive')
  })

  it('retains existing cards when loading fails', async () => {
    const service = createService({
      listCards: vi.fn().mockRejectedValue({
        code: 'database_operation_failed',
        message: '数据库操作失败',
        recovery: '重新打开应用',
      }),
    })
    useDesktopStore.setState({ cards: [firstCard], selectedCardId: 'card-1' })

    await useDesktopStore.getState().loadCards(service)

    expect(useDesktopStore.getState().cards).toEqual([firstCard])
    expect(useDesktopStore.getState().error).toEqual(
      new DesktopError('database_operation_failed', '数据库操作失败', '重新打开应用'),
    )
    expect(useDesktopStore.getState().isLoading).toBe(false)
    expect(useDesktopStore.getState().errorOperation).toBe('load')
  })

  it('retains cards but clears stale duplicate matches when creation fails', async () => {
    const service = createService({
      createRichCard: vi.fn().mockRejectedValue({
        code: 'database_operation_failed',
        message: '数据库操作失败',
        recovery: '重新打开应用',
      }),
    })
    useDesktopStore.setState({
      cards: [firstCard],
      selectedCardId: 'card-1',
      duplicateCardIds: ['old-duplicate'],
    })

    await expect(
      useDesktopStore.getState().createQuickCard(service, {
        title: '',
        originalContent: '未保存的想法',
        document: documentFromPlainText('未保存的想法'),
        note: '',
      }),
    ).rejects.toMatchObject({ code: 'database_operation_failed' })

    expect(useDesktopStore.getState()).toMatchObject({
      cards: [firstCard],
      selectedCardId: 'card-1',
      duplicateCardIds: [],
      error: new DesktopError('database_operation_failed', '数据库操作失败', '重新打开应用'),
      errorOperation: 'create',
    })
  })

  it('clears duplicate matches after the notice is dismissed', () => {
    useDesktopStore.setState({ duplicateCardIds: ['card-existing'] })

    useDesktopStore.getState().clearDuplicateCardIds()

    expect(useDesktopStore.getState().duplicateCardIds).toEqual([])
  })
})
