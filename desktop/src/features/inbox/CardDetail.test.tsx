import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { Card } from '../../entities/card/model'
import { documentFromPlainText } from '../../entities/document/model'
import { CardDetail } from './CardDetail'

const activeCard: Card = {
  id: 'card-1',
  title: '待整理想法',
  originalContent: '需要继续整理的原始内容',
  note: '',
  sourceType: 'personal',
  captureMethod: 'quick',
  sourceName: '快速记录',
  sourceUri: '',
  status: 'unprocessed',
  attachments: [],
  createdAt: '2026-07-19T08:00:00Z',
  updatedAt: '2026-07-19T08:00:00Z',
  archivedAt: null,
}

const archivedCard: Card = {
  ...activeCard,
  id: 'card-archived',
  title: '已归档想法',
  status: 'archived',
  archivedAt: '2026-07-19T09:00:00Z',
}

function renderDetail(
  card: Card,
  overrides: Partial<Parameters<typeof CardDetail>[0]> = {},
) {
  const props = {
    card,
    workspace: card.status === 'archived' ? 'archived' as const : 'inbox' as const,
    onGenerateTitle: vi.fn().mockResolvedValue("AI 标题"),
    onSave: vi.fn().mockResolvedValue(undefined),
    onArchive: vi.fn().mockResolvedValue(undefined),
    onRestore: vi.fn().mockResolvedValue(undefined),
    onDeleteRequest: vi.fn(),
    onRevealAttachment: vi.fn().mockResolvedValue(undefined),
    onLoadDocument: vi.fn().mockResolvedValue({
      cardId: card.id,
      document: documentFromPlainText(card.originalContent),
      schemaVersion: 1,
      updatedAt: card.updatedAt,
    }),
    onSaveDocument: vi.fn(),
    onLoadInlineImage: vi.fn(),
    onLoadConcepts: vi.fn().mockResolvedValue([]),
    onExplainConcept: vi.fn(),
    onSaveConceptAnnotation: vi.fn(),
    onOpenConcept: vi.fn(),
    onLoadAiSettings: vi.fn().mockResolvedValue({
      provider: 'openAi',
      baseUrl: 'https://api.openai.com/v1',
      model: 'gpt-5.6',
      webSearchEnabled: true,
      hasApiKey: false,
    }),
    onLoadAiMessages: vi.fn().mockResolvedValue([]),
    onSendAiMessage: vi.fn(),
    onRegenerateAiMessage: vi.fn(),
    onCancelAiRequest: vi.fn(),
    onCopyText: vi.fn(),
    onOpenAiSettings: vi.fn(),
    onOpenExternalUrl: vi.fn(),
    ...overrides,
  }
  render(<CardDetail {...props} />)
  return props
}

describe('CardDetail', () => {
  it('automatically saves a burst of edits and keeps the status quiet', async () => {
    const user = userEvent.setup()
    const props = renderDetail(activeCard)

    expect(screen.queryByRole('button', { name: '保存修改' })).not.toBeInTheDocument()
    expect(screen.getByText(/^已保存 \d{2}:\d{2}$/)).toBeInTheDocument()

    await user.type(screen.getByLabelText('我的想法'), '补充判断')

    expect(screen.getByText('等待保存')).toBeInTheDocument()
    expect(props.onSave).not.toHaveBeenCalled()
    await waitFor(() => {
      expect(props.onSave).toHaveBeenCalledWith({ note: '补充判断' })
    }, { timeout: 2_500 })
    expect(props.onSave).toHaveBeenCalledTimes(1)
    expect(screen.getByText(/^已保存 \d{2}:\d{2}$/)).toBeInTheDocument()
  })

  it('queues edits made while an automatic save is still running', async () => {
    const user = userEvent.setup()
    let finishFirstSave: (() => void) | undefined
    const onSave = vi.fn()
      .mockImplementationOnce(() => new Promise<void>((resolve) => {
        finishFirstSave = resolve
      }))
      .mockResolvedValue(undefined)
    renderDetail(activeCard, { onSave })

    await user.type(screen.getByLabelText('我的想法'), '第一段')
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1), { timeout: 2_500 })

    await user.type(screen.getByLabelText('我的想法'), '第二段')
    finishFirstSave?.()

    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(2), { timeout: 2_500 })
    expect(onSave).toHaveBeenLastCalledWith({ note: '第一段第二段' })
  })

  it('archives only when the active card has no unsaved changes', async () => {
    const user = userEvent.setup()
    const props = renderDetail(activeCard)
    const archiveButton = screen.getByRole('button', { name: '归档' })

    expect(archiveButton).toBeEnabled()
    await user.type(screen.getByLabelText('我的想法'), '补充内容')
    expect(archiveButton).toBeDisabled()

    await user.clear(screen.getByLabelText('我的想法'))
    expect(archiveButton).toBeEnabled()
    await user.click(archiveButton)

    expect(props.onArchive).toHaveBeenCalledTimes(1)
  })

  it('exposes deletion for an active card', async () => {
    const user = userEvent.setup()
    const props = renderDetail(activeCard)

    await user.click(screen.getByRole('button', { name: '删除' }))

    expect(props.onDeleteRequest).toHaveBeenCalledTimes(1)
  })

  it('keeps archived cards read-only and exposes restore and permanent delete', async () => {
    const user = userEvent.setup()
    const props = renderDetail(archivedCard)

    expect(screen.getByLabelText('标题')).toHaveAttribute('readonly')
    expect(screen.getByLabelText('原始内容')).toHaveAttribute('contenteditable', 'false')
    expect(screen.getByLabelText('我的想法')).toHaveAttribute('readonly')
    expect(screen.getByLabelText('来源类型')).toBeDisabled()
    expect(screen.queryByText('待整理')).not.toBeInTheDocument()
    expect(screen.queryByText('已整理')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '保存修改' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '恢复' }))
    expect(props.onRestore).toHaveBeenCalledTimes(1)

    await user.click(screen.getByRole('button', { name: '永久删除' }))
    expect(props.onDeleteRequest).toHaveBeenCalledTimes(1)
  })

  it('prioritizes the working content and collapses source metadata by default', async () => {
    renderDetail(activeCard)

    const title = screen.getByLabelText('标题')
    const original = await screen.findByLabelText('原始内容')
    const note = screen.getByLabelText('我的想法')
    const metadata = screen.getByText('来源与附件')

    expect(title.compareDocumentPosition(original) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(original.compareDocumentPosition(note) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(note.compareDocumentPosition(metadata) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(metadata.closest('details')).not.toHaveAttribute('open')
    expect(screen.queryByRole('dialog', { name: 'AI 延伸' })).toBeNull()
  })

  it('shows a legacy web source as document without offering web article', async () => {
    const user = userEvent.setup()
    renderDetail({ ...activeCard, sourceType: 'web' })

    await user.click(screen.getByText('来源与附件'))
    const sourceType = screen.getByLabelText('来源类型')
    expect(sourceType).toHaveValue('document')
    expect(within(sourceType).getByRole('option', { name: '文档资料' })).toBeInTheDocument()
    expect(within(sourceType).queryByRole('option', { name: '网页文章' })).not.toBeInTheDocument()
  })

  it('opens and closes the AI drawer from the header', async () => {
    const user = userEvent.setup()
    renderDetail(activeCard, {
      onLoadAiSettings: vi.fn().mockResolvedValue({
        provider: 'openAi' as const,
        baseUrl: 'https://api.openai.com/v1',
        model: 'gpt-5.6',
        webSearchEnabled: true,
        hasApiKey: true,
      }),
    })

    expect(screen.queryByRole('dialog', { name: 'AI 延伸' })).toBeNull()
    await user.click(screen.getByRole('button', { name: 'AI 延伸' }))
    expect(await screen.findByRole('dialog', { name: 'AI 延伸' }))
      .toHaveTextContent('还没有 AI 讨论记录')
    await user.click(screen.getByRole('button', { name: '关闭 AI 延伸' }))
    expect(screen.queryByRole('dialog', { name: 'AI 延伸' })).toBeNull()
  })

  it('keeps per-question web search disabled when the global setting is off', async () => {
    const user = userEvent.setup()
    renderDetail(activeCard, {
      onLoadAiSettings: vi.fn().mockResolvedValue({
        provider: 'deepSeek' as const,
        baseUrl: 'https://api.deepseek.com',
        model: 'deepseek-v4-flash',
        webSearchEnabled: false,
        hasApiKey: true,
      }),
    })

    await user.click(screen.getByRole('button', { name: 'AI 延伸' }))

    expect(await screen.findByLabelText('联网查询')).toBeDisabled()
  })

  it('does not inspect the Keychain until the user activates AI', async () => {
    const onLoadAiSettings = vi.fn().mockResolvedValue({
      provider: 'openAi' as const,
      baseUrl: 'https://api.openai.com/v1',
      model: 'gpt-5.6',
      webSearchEnabled: true,
      hasApiKey: true,
    })
    renderDetail(activeCard, { onLoadAiSettings })

    await screen.findByLabelText('原始内容')
    expect(onLoadAiSettings).not.toHaveBeenCalled()

    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'AI 延伸' }))
    await screen.findByLabelText('和 AI 讨论')

    await waitFor(() => expect(onLoadAiSettings).toHaveBeenCalledTimes(1))
  })

  it('keeps lifecycle failures in the detail panel', async () => {
    const user = userEvent.setup()
    renderDetail(activeCard, {
      onArchive: vi.fn().mockRejectedValue({
        code: 'database_operation_failed',
        message: '归档失败',
        recovery: '请重试',
      }),
    })

    await user.click(screen.getByRole('button', { name: '归档' }))

    const detail = screen.getByRole('form', { name: '卡片详情' })
    expect(await within(detail).findByRole('alert')).toHaveTextContent('归档失败')
    expect(within(detail).getByRole('alert')).toHaveTextContent('请重试')
  })

  it('explains a selected term and saves its mark into the original document', async () => {
    const user = userEvent.setup()
    const explanationResult = {
      selectedText: '继续',
      explanation: '表示在已有内容基础上向前推进。',
      citations: [],
    }
    let pushConceptDelta!: (delta: string) => void
    let resolveConcept!: (value: typeof explanationResult) => void
    const onExplainConcept = vi.fn().mockImplementation((
      _input: unknown,
      onDelta: (delta: string) => void,
    ) => {
      pushConceptDelta = onDelta
      return new Promise<typeof explanationResult>((resolve) => {
        resolveConcept = resolve
      })
    })
    const onSaveConceptAnnotation = vi.fn().mockImplementation(async (input) => ({
      concept: {
        id: input.conceptId,
        name: input.name,
        explanation: input.explanation,
        mentionCount: 1,
        updatedAt: activeCard.updatedAt,
      },
      mention: {
        id: input.conceptId,
        name: input.name,
        explanation: input.explanation,
        mentionId: input.mentionId,
      },
      document: input.document,
      originalContent: activeCard.originalContent,
      updatedAt: activeCard.updatedAt,
    }))
    const onLoadAiSettings = vi.fn().mockResolvedValue({
      provider: 'openAi' as const,
      baseUrl: 'https://api.openai.com/v1',
      model: 'gpt-5.6',
      webSearchEnabled: true,
      hasApiKey: true,
    })
    const props = renderDetail(activeCard, {
      onExplainConcept,
      onSaveConceptAnnotation,
      onLoadAiSettings,
    })

    await user.click(screen.getByRole('button', { name: 'AI 延伸' }))
    await screen.findByLabelText('和 AI 讨论')
    await waitFor(() => expect(props.onLoadConcepts).toHaveBeenCalledTimes(1))
    const editor = await screen.findByRole('textbox', { name: '原始内容' })
    const text = editor.querySelector('p')?.firstChild
    const range = document.createRange()
    range.setStart(text!, 2)
    range.setEnd(text!, 4)
    window.getSelection()?.removeAllRanges()
    window.getSelection()?.addRange(range)
    fireEvent.mouseUp(editor)
    fireEvent.contextMenu(editor)
    editor.parentElement?.parentElement?.dispatchEvent(
      new CustomEvent('ideacard:explain-concept'),
    )

    await waitFor(() => expect(onLoadAiSettings).toHaveBeenCalledTimes(1))
    await waitFor(() => expect(onExplainConcept).toHaveBeenCalled())
    const dialog = await screen.findByRole('dialog', { name: '解释概念' })
    expect(within(dialog).getByRole('status', { name: 'AI 正在思考' })).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: '生成中...' })).toBeDisabled()

    act(() => pushConceptDelta('表示在已有内容'))
    expect(await within(dialog).findByText('表示在已有内容')).toBeInTheDocument()
    act(() => pushConceptDelta('基础上向前推进。'))
    expect(await within(dialog).findByText('表示在已有内容基础上向前推进。'))
      .toBeInTheDocument()
    await act(async () => resolveConcept(explanationResult))
    expect(within(dialog).getByRole('button', { name: '保存概念' })).toBeEnabled()
    await user.click(screen.getByRole('button', { name: '保存概念' }))

    await waitFor(() => {
      expect(onSaveConceptAnnotation).toHaveBeenCalledWith(expect.objectContaining({
        cardId: activeCard.id,
        selectedText: '继续',
        name: '继续',
        explanation: '表示在已有内容基础上向前推进。',
        overwriteExisting: false,
        document: expect.objectContaining({ type: 'doc' }),
      }))
    })
    expect(screen.queryByRole('dialog', { name: '解释概念' })).not.toBeInTheDocument()
  })

  it('overwrites a saved concept explanation without replacing its existing mark', async () => {
    const user = userEvent.setup()
    const card = { ...activeCard, originalContent: 'OPC' }
    const markedDocument = {
      type: 'doc' as const,
      content: [{
        type: 'paragraph' as const,
        content: [{
          type: 'text' as const,
          text: 'OPC',
          marks: [{
            type: 'concept' as const,
            attrs: { conceptId: 'concept-1', mentionId: 'mention-1' },
          }],
        }],
      }],
    }
    const onSaveConceptAnnotation = vi.fn().mockImplementation(async (input) => ({
      concept: {
        id: 'concept-1',
        name: 'OPC',
        explanation: input.explanation,
        mentionCount: 1,
        updatedAt: activeCard.updatedAt,
      },
      mention: {
        id: 'concept-1',
        name: 'OPC',
        explanation: input.explanation,
        mentionId: 'mention-1',
      },
      document: input.document,
      originalContent: 'OPC',
      updatedAt: activeCard.updatedAt,
    }))
    const props = renderDetail(card, {
      onLoadDocument: vi.fn().mockResolvedValue({
        cardId: card.id,
        document: markedDocument,
        schemaVersion: 1,
        updatedAt: card.updatedAt,
      }),
      onLoadConcepts: vi.fn().mockResolvedValue([{
        id: 'concept-1',
        name: 'OPC',
        explanation: '原解释。',
        mentionId: 'mention-1',
      }]),
      onLoadAiSettings: vi.fn().mockResolvedValue({
        provider: 'openAi' as const,
        baseUrl: 'https://api.openai.com/v1',
        model: 'gpt-5.6',
        webSearchEnabled: true,
        hasApiKey: true,
      }),
      onExplainConcept: vi.fn().mockResolvedValue({
        selectedText: 'OPC',
        explanation: '新的 OPC 解释。',
        citations: [],
      }),
      onSaveConceptAnnotation,
    })

    await waitFor(() => expect(props.onLoadConcepts).toHaveBeenCalledTimes(1))
    await waitFor(() => expect(screen.getByText('OPC')).toHaveAttribute('data-concept-name', 'OPC'))
    const mark = screen.getByText('OPC')
    const editor = screen.getByRole('textbox', { name: '原始内容' })
    const range = document.createRange()
    range.selectNodeContents(mark)
    window.getSelection()?.removeAllRanges()
    window.getSelection()?.addRange(range)
    fireEvent.mouseUp(editor)
    fireEvent.contextMenu(editor)
    editor.parentElement?.parentElement?.dispatchEvent(
      new CustomEvent('ideacard:explain-concept'),
    )

    const dialog = await screen.findByRole('dialog', { name: '解释概念' })
    expect(within(dialog).getByRole('note')).toHaveTextContent('将覆盖原有解释')
    expect(within(dialog).getByLabelText('概念名称')).toHaveAttribute('readonly')
    await user.click(within(dialog).getByRole('button', { name: '覆盖原解释' }))

    await waitFor(() => {
      expect(onSaveConceptAnnotation).toHaveBeenCalledWith({
        cardId: card.id,
        conceptId: 'concept-1',
        mentionId: 'mention-1',
        selectedText: 'OPC',
        name: 'OPC',
        explanation: '新的 OPC 解释。',
        overwriteExisting: true,
        document: markedDocument,
      })
    })
    expect(screen.queryByRole('dialog', { name: '解释概念' })).not.toBeInTheDocument()
    expect(screen.getByText('OPC')).toHaveAttribute('data-mention-id', 'mention-1')
  })
})


describe('manual AI title generation', () => {
  it('can generate repeatedly and uses the current original content', async () => {
    const user = userEvent.setup()
    const generate = vi.fn().mockResolvedValueOnce('第一版标题').mockResolvedValueOnce('第二版标题')
    renderDetail(activeCard, { onGenerateTitle: generate })
    const button = screen.getByRole('button', { name: 'AI 生成' })
    await waitFor(() => expect(button).toBeEnabled())
    await user.click(button)
    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('第一版标题'))
    await user.click(button)
    await waitFor(() => expect(screen.getByLabelText('标题')).toHaveValue('第二版标题'))
    expect(generate).toHaveBeenCalledTimes(2)
    expect(generate).toHaveBeenLastCalledWith(activeCard.originalContent)
  })

  it('preserves edits made while generation is pending', async () => {
    const user = userEvent.setup()
    let resolve!: (value: string) => void
    const generate = vi.fn().mockImplementation(() => new Promise<string>((done) => { resolve = done }))
    renderDetail(activeCard, { onGenerateTitle: generate })
    const button = screen.getByRole('button', { name: 'AI 生成' })
    await waitFor(() => expect(button).toBeEnabled())
    await user.click(button)
    expect(screen.getByRole('button', { name: '生成中…' })).toBeDisabled()
    await user.clear(screen.getByLabelText('标题'))
    await user.type(screen.getByLabelText('标题'), '手动修改')
    await act(async () => resolve('迟到的标题'))
    expect(screen.getByLabelText('标题')).toHaveValue('手动修改')
  })

  it('retains the title and allows retry after a model failure', async () => {
    const user = userEvent.setup()
    renderDetail(activeCard, { onGenerateTitle: vi.fn().mockRejectedValue(new Error('生成失败')) })
    const button = screen.getByRole('button', { name: 'AI 生成' })
    await waitFor(() => expect(button).toBeEnabled())
    await user.click(button)
    await screen.findByRole('alert')
    expect(screen.getByLabelText('标题')).toHaveValue(activeCard.title)
    expect(button).toBeEnabled()
  })
})
