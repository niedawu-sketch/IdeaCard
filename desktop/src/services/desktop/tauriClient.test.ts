import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { Card, NewCardInput } from '../../entities/card/model'
import { documentFromPlainText } from '../../entities/document/model'
import { DesktopError } from '../../shared/errors/DesktopError'
import type { CardPatch } from './contracts'
import { tauriDesktopService } from './tauriClient'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

const invokeMock = vi.mocked(invoke)
const listenMock = vi.mocked(listen)

const card: Card = {
  id: 'card-1',
  title: '临时问题',
  originalContent: '临时问题',
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

const input: NewCardInput = {
  title: '临时问题',
  originalContent: '临时问题',
  note: '',
  sourceType: 'personal',
  captureMethod: 'quick',
  sourceName: '',
  sourceUri: '',
}

describe('tauriDesktopService', () => {
  beforeEach(() => {
    invokeMock.mockReset()
    listenMock.mockReset()
  })

  it('maps card reads to their Tauri commands', async () => {
    invokeMock.mockResolvedValueOnce([card]).mockResolvedValueOnce(card)

    await expect(tauriDesktopService.listCards()).resolves.toEqual([card])
    await expect(tauriDesktopService.getCard('card-1')).resolves.toEqual(card)

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'list_cards', { includeArchived: false })
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'get_card', { id: 'card-1' })
  })

  it('maps full card lifecycle operations to Tauri commands', async () => {
    const archived = { ...card, status: 'archived' as const }
    invokeMock
      .mockResolvedValueOnce([card, archived])
      .mockResolvedValueOnce(archived)
      .mockResolvedValueOnce(card)
      .mockResolvedValueOnce(undefined)

    await tauriDesktopService.listCards(true)
    await tauriDesktopService.archiveCard('card-1')
    await tauriDesktopService.restoreCard('card-1')
    await tauriDesktopService.deleteCard('card-1')

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'list_cards', { includeArchived: true })
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'archive_card', { id: 'card-1' })
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'restore_card', { id: 'card-1' })
    expect(invokeMock).toHaveBeenNthCalledWith(4, 'delete_card', { id: 'card-1' })
  })

  it('preserves duplicate IDs returned when creating a card', async () => {
    const result = { card, duplicateCardIds: ['card-existing'] }
    invokeMock.mockResolvedValue(result)

    await expect(tauriDesktopService.createCard(input)).resolves.toEqual(result)

    expect(invokeMock).toHaveBeenCalledWith('create_card', { input })
  })

  it('maps structured document creation, loading, saving, and image reads', async () => {
    const document = documentFromPlainText('结构化正文')
    const result = { card, duplicateCardIds: [] }
    invokeMock
      .mockResolvedValueOnce(result)
      .mockResolvedValueOnce({
        cardId: 'card-1',
        document,
        schemaVersion: 1,
        updatedAt: card.updatedAt,
      })
      .mockResolvedValueOnce({
        cardId: 'card-1',
        document,
        originalContent: '结构化正文',
        status: 'unprocessed',
        updatedAt: card.updatedAt,
      })
      .mockResolvedValueOnce({
        mimeType: 'image/png',
        dataUrl: 'data:image/png;base64,dGVzdA==',
      })

    const richInput = {
      title: '',
      note: '',
      sourceType: 'personal' as const,
      captureMethod: 'quick' as const,
      sourceName: '',
      sourceUri: '',
      document,
    }
    await tauriDesktopService.createRichCard(richInput)
    await tauriDesktopService.getCardDocument('card-1')
    await tauriDesktopService.saveCardDocument('card-1', document)
    await tauriDesktopService.loadInlineImage('attachment-1')

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'create_rich_card', { input: richInput })
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'get_card_document', { cardId: 'card-1' })
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'save_card_document', {
      cardId: 'card-1',
      document,
    })
    expect(invokeMock).toHaveBeenNthCalledWith(4, 'load_inline_image', {
      attachmentId: 'attachment-1',
    })
  })

  it('maps concept queries and annotation saving to Tauri commands', async () => {
    const document = documentFromPlainText('费曼学习法')
    const annotation = {
      cardId: 'card-1',
      conceptId: 'concept-1',
      mentionId: 'mention-1',
      selectedText: '费曼学习法',
      name: '费曼学习法',
      explanation: '通过讲解检验理解。',
      overwriteExisting: false,
      document,
    }
    invokeMock
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce({ id: 'concept-1', mentions: [] })
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce({ document })

    await tauriDesktopService.listConcepts()
    await tauriDesktopService.getConcept('concept-1')
    await tauriDesktopService.deleteConcept('concept-1')
    await tauriDesktopService.listCardConcepts('card-1')
    await tauriDesktopService.saveConceptAnnotation(annotation)

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'list_concepts')
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'get_concept', { id: 'concept-1' })
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'delete_concept', { id: 'concept-1' })
    expect(invokeMock).toHaveBeenNthCalledWith(4, 'list_card_concepts', { cardId: 'card-1' })
    expect(invokeMock).toHaveBeenNthCalledWith(5, 'save_concept_annotation', {
      input: annotation,
    })
  })

  it('maps AI settings and card conversation commands without exposing a key', async () => {
    const settings = {
      provider: 'openAi' as const,
      baseUrl: 'https://api.openai.com/v1',
      model: 'gpt-5.6',
      webSearchEnabled: true,
      hasApiKey: true,
    }
    invokeMock
      .mockResolvedValueOnce(settings)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce(undefined)

    await tauriDesktopService.getAiSettings()
    await tauriDesktopService.listCardAiMessages('card-1')
    await tauriDesktopService.sendCardAiMessage({
      cardId: 'card-1',
      content: '继续延伸',
      webSearchEnabled: true,
      requestId: 'request-1',
    })
    await tauriDesktopService.cancelAiRequest('request-1')

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'get_ai_settings')
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'list_card_ai_messages', { cardId: 'card-1' })
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'send_card_ai_message', {
      input: {
        cardId: 'card-1',
        content: '继续延伸',
        webSearchEnabled: true,
        requestId: 'request-1',
      },
    })
    expect(invokeMock).toHaveBeenNthCalledWith(4, 'cancel_ai_request', {
      requestId: 'request-1',
    })
  })

  it('forwards only text deltas for the active AI request and removes the listener', async () => {
    const unlisten = vi.fn()
    const onDelta = vi.fn()
    listenMock.mockImplementation(async (_event, handler) => {
      invokeMock.mockImplementationOnce(async () => {
        handler({
          payload: { requestId: 'another-request', kind: 'answer', delta: '忽略' },
        } as never)
        handler({
          payload: { requestId: 'request-stream', kind: 'reasoning', delta: '第一段' },
        } as never)
        return []
      })
      return unlisten
    })

    await tauriDesktopService.sendCardAiMessage({
      cardId: 'card-1',
      content: '流式回答',
      webSearchEnabled: false,
      requestId: 'request-stream',
    }, onDelta)

    expect(listenMock).toHaveBeenCalledWith('ai://text-delta', expect.any(Function))
    expect(onDelta).toHaveBeenCalledTimes(1)
    expect(onDelta).toHaveBeenCalledWith({ kind: 'reasoning', delta: '第一段' })
    expect(unlisten).toHaveBeenCalledTimes(1)
  })

  it('maps concept assistant settings to independent commands', async () => {
    const settings = { systemPrompt: '自定义概念规则' }
    invokeMock.mockResolvedValue(settings)

    await tauriDesktopService.getConceptAssistantSettings()
    await tauriDesktopService.saveConceptAssistantSettings(settings)

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'get_concept_assistant_settings')
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'save_concept_assistant_settings', {
      input: settings,
    })
  })

  it('maps card updates without reshaping the patch', async () => {
    const patch: CardPatch = { note: '继续追问', sourceType: 'document' }
    invokeMock.mockResolvedValue({ ...card, ...patch })

    await tauriDesktopService.updateCard('card-1', patch)

    expect(invokeMock).toHaveBeenCalledWith('update_card', { id: 'card-1', patch })
  })

  it('maps the invisible voice input bridge commands without text content', async () => {
    invokeMock.mockResolvedValue(undefined)

    await tauriDesktopService.setVoiceInputTarget?.('voice-target-1')
    await tauriDesktopService.finishVoiceInputSession?.(7)

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'set_voice_input_target', {
      targetToken: 'voice-target-1',
    })
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'finish_voice_input_session', {
      sessionId: 7,
    })
  })

  it('maps native text field lifecycle commands without reshaping geometry', async () => {
    invokeMock.mockResolvedValue(undefined)
    const input = {
      fieldId: 'card-title-1',
      value: '测试标题',
      placeholder: '',
      readOnly: false,
      visible: true,
      x: 20,
      y: 40,
      width: 320,
      height: 35,
    }

    await tauriDesktopService.syncNativeTextField?.(input)
    await tauriDesktopService.focusNativeTextField?.('card-title-1')
    await tauriDesktopService.removeNativeTextField?.('card-title-1')

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'sync_native_text_field', { input })
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'focus_native_text_field', {
      fieldId: 'card-title-1',
    })
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'remove_native_text_field', {
      fieldId: 'card-title-1',
    })
  })

  it('maps native text area lifecycle commands without reshaping geometry', async () => {
    invokeMock.mockResolvedValue(undefined)
    const input = {
      fieldId: 'card-note-1',
      value: '第一行\n第二行',
      readOnly: false,
      submitOnEnter: false,
      visible: true,
      x: 20,
      y: 40,
      width: 320,
      height: 120,
    }

    await tauriDesktopService.syncNativeTextArea?.(input)
    await tauriDesktopService.focusNativeTextArea?.('card-note-1')
    await tauriDesktopService.removeNativeTextArea?.('card-note-1')

    expect(invokeMock).toHaveBeenNthCalledWith(1, 'sync_native_text_area', { input })
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'focus_native_text_area', {
      fieldId: 'card-note-1',
    })
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'remove_native_text_area', {
      fieldId: 'card-note-1',
    })
  })

  it('converts stable command failures to DesktopError', async () => {
    invokeMock.mockRejectedValue({
      code: 'database_operation_failed',
      message: '数据库操作失败',
      recovery: '重新打开应用',
    })

    await expect(tauriDesktopService.listCards()).rejects.toMatchObject({
      name: 'DesktopError',
      code: 'database_operation_failed',
      message: '数据库操作失败',
      recovery: '重新打开应用',
    })
  })

  it('does not expose native error details', async () => {
    invokeMock.mockRejectedValue(new Error('SQL /private/ideacard.sqlite3'))

    await expect(tauriDesktopService.listCards()).rejects.toEqual(
      new DesktopError('unexpected', '发生未知错误'),
    )
  })
})
