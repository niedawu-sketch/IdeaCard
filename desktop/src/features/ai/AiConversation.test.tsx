import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { AiMessage } from '../../entities/ai/model'
import type { AiStreamDelta } from '../../services/desktop/contracts'
import { AiConversation } from './AiConversation'

const assistantMessage: AiMessage = {
  id: 'message-2',
  role: 'assistant',
  content: '可以先从反例验证这个判断。',
  reasoningContent: '',
  webSearchEnabled: false,
  citations: [{
    url: 'https://example.com/source',
    title: '参考来源',
    position: 0,
  }],
  createdAt: '2026-07-19T08:00:00Z',
}

describe('AiConversation', () => {
  it('starts compact and grows the composer with its content', () => {
    render(
      <AiConversation
        configured
        messages={[]}
        readOnly={false}
        webSearchAvailable={false}
        onCancel={vi.fn()}
        onCopy={vi.fn()}
        onOpenCitation={vi.fn()}
        onOpenSettings={vi.fn()}
        onRegenerate={vi.fn()}
        onSend={vi.fn()}
      />,
    )

    const input = screen.getByLabelText('和 AI 讨论')
    expect(input).toHaveAttribute('rows', '1')
    expect(input).not.toHaveAttribute('placeholder')
    Object.defineProperty(input, 'scrollHeight', { configurable: true, value: 96 })

    fireEvent.change(input, { target: { value: '第一行\n第二行\n第三行' } })

    expect(input).toHaveStyle({ height: '96px', overflowY: 'hidden' })
  })

  it('keeps the latest turn visible inside the conversation scroller', () => {
    const { rerender } = render(
      <AiConversation
        configured
        messages={[]}
        readOnly={false}
        webSearchAvailable={false}
        onCancel={vi.fn()}
        onCopy={vi.fn()}
        onOpenCitation={vi.fn()}
        onOpenSettings={vi.fn()}
        onRegenerate={vi.fn()}
        onSend={vi.fn()}
      />,
    )
    const messageList = screen.getByLabelText('AI 对话记录')
    Object.defineProperty(messageList, 'scrollHeight', { configurable: true, value: 680 })

    rerender(
      <AiConversation
        configured
        messages={[assistantMessage]}
        readOnly={false}
        webSearchAvailable={false}
        onCancel={vi.fn()}
        onCopy={vi.fn()}
        onOpenCitation={vi.fn()}
        onOpenSettings={vi.fn()}
        onRegenerate={vi.fn()}
        onSend={vi.fn()}
      />,
    )

    expect(messageList.scrollTop).toBe(680)
  })

  it('sends a question with the web-search choice and renders clickable citations', async () => {
    const user = userEvent.setup()
    const onSend = vi.fn().mockResolvedValue([
      {
        id: 'message-1',
        role: 'user',
        content: '这个想法还有什么反例？',
        reasoningContent: '',
        webSearchEnabled: true,
        citations: [],
        createdAt: '2026-07-19T07:59:00Z',
      },
      assistantMessage,
    ])
    const onOpenCitation = vi.fn()
    render(
      <AiConversation
        configured
        messages={[]}
        readOnly={false}
        webSearchAvailable
        onCancel={vi.fn()}
        onCopy={vi.fn()}
        onOpenCitation={onOpenCitation}
        onOpenSettings={vi.fn()}
        onRegenerate={vi.fn()}
        onSend={onSend}
      />,
    )

    await user.type(screen.getByLabelText('和 AI 讨论'), '这个想法还有什么反例？')
    await user.click(screen.getByLabelText('联网查询'))
    await user.click(screen.getByRole('button', { name: '发送' }))

    expect(onSend).toHaveBeenCalledWith({
      content: '这个想法还有什么反例？',
      webSearchEnabled: true,
    }, expect.any(Function))
    expect(await screen.findByText('可以先从反例验证这个判断。')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '参考来源' }))
    expect(onOpenCitation).toHaveBeenCalledWith('https://example.com/source')
    expect(screen.getByLabelText('和 AI 讨论')).toHaveValue('')
  })

  it('sends with Enter and keeps Shift+Enter available for multiline input', async () => {
    const onSend = vi.fn().mockResolvedValue([])
    render(
      <AiConversation
        configured
        messages={[]}
        readOnly={false}
        webSearchAvailable={false}
        onCancel={vi.fn()}
        onCopy={vi.fn()}
        onOpenCitation={vi.fn()}
        onOpenSettings={vi.fn()}
        onRegenerate={vi.fn()}
        onSend={onSend}
      />,
    )

    const input = screen.getByLabelText('和 AI 讨论')
    fireEvent.change(input, { target: { value: '先保留换行' } })
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: true })
    expect(onSend).not.toHaveBeenCalled()

    fireEvent.change(input, { target: { value: '直接发送' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    await waitFor(() => {
      expect(onSend).toHaveBeenCalledWith({
        content: '直接发送',
        webSearchEnabled: false,
      }, expect.any(Function))
    })
  })

  it('shows the user turn immediately, waits for the first token, and streams the reply', async () => {
    const user = userEvent.setup()
    let pushDelta!: (delta: AiStreamDelta) => void
    let resolveSend!: (messages: AiMessage[]) => void
    const onSend = vi.fn().mockImplementation((
      _input: unknown,
      onDelta: (delta: AiStreamDelta) => void,
    ) => {
      pushDelta = onDelta
      return new Promise<AiMessage[]>((resolve) => {
        resolveSend = resolve
      })
    })
    render(
      <AiConversation
        configured
        messages={[]}
        readOnly={false}
        webSearchAvailable={false}
        onCancel={vi.fn()}
        onCopy={vi.fn()}
        onOpenCitation={vi.fn()}
        onOpenSettings={vi.fn()}
        onRegenerate={vi.fn()}
        onSend={onSend}
      />,
    )

    const input = screen.getByLabelText('和 AI 讨论')
    await user.type(input, '立即显示这条消息')
    await user.click(screen.getByRole('button', { name: '发送' }))

    expect(screen.getByLabelText('我的消息')).toHaveTextContent('立即显示这条消息')
    expect(screen.getByRole('status', { name: 'AI 正在思考' })).toBeInTheDocument()
    expect(input).toHaveValue('')

    pushDelta({ kind: 'reasoning', delta: '先查证资料' })
    const reasoning = await screen.findByText('先查证资料')
    expect((reasoning.closest('details') as HTMLDetailsElement).open).toBe(true)

    pushDelta({ kind: 'answer', delta: '第一段' })
    expect(await screen.findByText('第一段')).toBeInTheDocument()
    expect((reasoning.closest('details') as HTMLDetailsElement).open).toBe(false)
    expect(screen.queryByRole('status', { name: 'AI 正在思考' })).not.toBeInTheDocument()

    pushDelta({ kind: 'answer', delta: '，继续出现' })
    expect(await screen.findByText('第一段，继续出现')).toBeInTheDocument()

    await act(async () => {
      resolveSend([
        {
          id: 'message-stream-user',
          role: 'user',
          content: '立即显示这条消息',
          reasoningContent: '',
          webSearchEnabled: false,
          citations: [],
          createdAt: '2026-07-31T00:00:00Z',
        },
        { ...assistantMessage, content: '第一段，继续出现' },
      ])
    })
    expect(screen.getByText('第一段，继续出现')).toBeInTheDocument()
  })

  it('renders safe Markdown and keeps reasoning secondary and collapsible', async () => {
    const user = userEvent.setup()
    const onOpenCitation = vi.fn()
    render(
      <AiConversation
        configured
        messages={[{
          ...assistantMessage,
          content: [
            '## 结论',
            '',
            '**重点判断**',
            '',
            '- 第一项',
            '- 第二项',
            '',
            '[官方文档](https://example.com/docs)',
            '',
            '[不安全链接](javascript:alert(1))',
            '',
            '<script>window.__unsafe = true</script>',
          ].join('\n'),
          reasoningContent: '先检索，再交叉验证。',
        }]}
        readOnly={false}
        webSearchAvailable={false}
        onCancel={vi.fn()}
        onCopy={vi.fn()}
        onOpenCitation={onOpenCitation}
        onOpenSettings={vi.fn()}
        onRegenerate={vi.fn()}
        onSend={vi.fn()}
      />,
    )

    expect(screen.getByRole('heading', { level: 2, name: '结论' })).toBeInTheDocument()
    expect(screen.getByText('重点判断').tagName).toBe('STRONG')
    expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual([
      '第一项',
      '第二项',
    ])
    const reasoning = screen.getByText('先检索，再交叉验证。')
    expect((reasoning.closest('details') as HTMLDetailsElement).open).toBe(false)

    await user.click(screen.getByText('思考过程'))
    expect((reasoning.closest('details') as HTMLDetailsElement).open).toBe(true)
    await user.click(screen.getByRole('link', { name: /官方文档/ }))
    expect(onOpenCitation).toHaveBeenCalledWith('https://example.com/docs')
    expect(screen.getByText('不安全链接').closest('a')).toBeNull()
    expect(document.querySelector('script')).toBeNull()
  })

  it('copies the final Markdown and regenerates the latest answer in place', async () => {
    const user = userEvent.setup()
    const question: AiMessage = {
      id: 'message-question',
      role: 'user',
      content: '重新回答这个问题',
      reasoningContent: '',
      webSearchEnabled: true,
      citations: [],
      createdAt: '2026-08-12T00:00:00Z',
    }
    const markdownAnswer = '**旧回答**'
    const answer = { ...assistantMessage, content: markdownAnswer }
    const onCopy = vi.fn().mockResolvedValue(undefined)
    const onRegenerate = vi.fn().mockImplementation(async (
      _messageId: string,
      onDelta: (delta: AiStreamDelta) => void,
    ) => {
      onDelta({ kind: 'reasoning', delta: '重新检查' })
      onDelta({ kind: 'answer', delta: '新的回答' })
      return [question, {
        ...answer,
        content: '新的回答',
        reasoningContent: '重新检查',
      }]
    })
    render(
      <AiConversation
        configured
        messages={[question, answer]}
        readOnly={false}
        webSearchAvailable
        onCancel={vi.fn()}
        onCopy={onCopy}
        onOpenCitation={vi.fn()}
        onOpenSettings={vi.fn()}
        onRegenerate={onRegenerate}
        onSend={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('button', { name: '复制回答' }))
    expect(onCopy).toHaveBeenCalledWith(markdownAnswer)
    expect(screen.getByText('已复制')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '重新生成回答' }))
    expect(onRegenerate).toHaveBeenCalledWith('message-2', expect.any(Function))
    expect(await screen.findByText('新的回答')).toBeInTheDocument()
    expect(screen.queryByText('旧回答')).not.toBeInTheDocument()
    expect(screen.getAllByLabelText('我的消息')).toHaveLength(1)
    expect(
      (screen.getByText('重新检查').closest('details') as HTMLDetailsElement).open,
    ).toBe(false)
  })

  it('keeps the question after a recoverable failure', async () => {
    const user = userEvent.setup()
    render(
      <AiConversation
        configured
        messages={[]}
        readOnly={false}
        webSearchAvailable={false}
        onCancel={vi.fn()}
        onCopy={vi.fn()}
        onOpenCitation={vi.fn()}
        onOpenSettings={vi.fn()}
        onRegenerate={vi.fn()}
        onSend={vi.fn().mockImplementation(async (
          _input: unknown,
          onDelta: (delta: AiStreamDelta) => void,
        ) => {
          onDelta({ kind: 'answer', delta: '未完成的回答' })
          throw {
            code: 'ai_request_failed',
            message: 'AI 请求失败',
            recovery: '请检查模型设置后重试',
          }
        })}
      />,
    )

    const input = screen.getByLabelText('和 AI 讨论')
    await user.type(input, '保留这个问题')
    await user.click(screen.getByRole('button', { name: '发送' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('AI 请求失败')
    expect(input).toHaveValue('保留这个问题')
    expect(screen.queryByText('未完成的回答')).not.toBeInTheDocument()
  })

  it('shows setup guidance when unconfigured and hides the composer for archived cards', async () => {
    const user = userEvent.setup()
    const onOpenSettings = vi.fn()
    const { rerender } = render(
      <AiConversation
        configured={false}
        messages={[]}
        readOnly={false}
        webSearchAvailable={false}
        onCancel={vi.fn()}
        onCopy={vi.fn()}
        onOpenCitation={vi.fn()}
        onOpenSettings={onOpenSettings}
        onRegenerate={vi.fn()}
        onSend={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('button', { name: '打开模型设置' }))
    expect(onOpenSettings).toHaveBeenCalled()

    rerender(
      <AiConversation
        configured
        messages={[assistantMessage]}
        readOnly
        webSearchAvailable
        onCancel={vi.fn()}
        onCopy={vi.fn()}
        onOpenCitation={vi.fn()}
        onOpenSettings={vi.fn()}
        onRegenerate={vi.fn()}
        onSend={vi.fn()}
      />,
    )
    expect(within(screen.getByLabelText('AI 对话记录')).getByText(assistantMessage.content))
      .toBeInTheDocument()
    expect(screen.queryByLabelText('和 AI 讨论')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '复制回答' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '重新生成回答' })).toBeDisabled()
  })

  it('stops the sending state and ignores a response that arrives after cancellation', async () => {
    const user = userEvent.setup()
    let resolveSend!: (messages: AiMessage[]) => void
    let pushDelta!: (delta: AiStreamDelta) => void
    const onSend = vi.fn().mockImplementation((
      _input: unknown,
      onDelta: (delta: AiStreamDelta) => void,
    ) => {
      pushDelta = onDelta
      return new Promise<AiMessage[]>((resolve) => {
        resolveSend = resolve
      })
    })
    const onCancel = vi.fn()
    render(
      <AiConversation
        configured
        messages={[]}
        readOnly={false}
        webSearchAvailable={false}
        onCancel={onCancel}
        onCopy={vi.fn()}
        onOpenCitation={vi.fn()}
        onOpenSettings={vi.fn()}
        onRegenerate={vi.fn()}
        onSend={onSend}
      />,
    )

    await user.type(screen.getByLabelText('和 AI 讨论'), '停止这个请求')
    await user.click(screen.getByRole('button', { name: '发送' }))
    act(() => pushDelta({ kind: 'answer', delta: '取消前的半段内容' }))
    expect(screen.getByText('取消前的半段内容')).toBeInTheDocument()
    await user.click(screen.getByTitle('停止生成'))

    expect(onCancel).toHaveBeenCalled()
    expect(screen.getByRole('button', { name: '发送' })).toBeInTheDocument()
    expect(screen.queryByText('取消前的半段内容')).not.toBeInTheDocument()

    resolveSend([assistantMessage])
    await waitFor(() => {
      expect(screen.queryByText(assistantMessage.content)).not.toBeInTheDocument()
    })
    expect(screen.getByLabelText('和 AI 讨论')).toHaveValue('停止这个请求')
  })
})
