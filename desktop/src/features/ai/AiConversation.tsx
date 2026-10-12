import { AlertCircle, Check, Copy, ExternalLink, RefreshCw, Send, Square } from 'lucide-react'
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
} from 'react'

import type { AiMessage } from '../../entities/ai/model'
import type { AiStreamDelta } from '../../services/desktop/contracts'
import { NativeTextArea } from '../native-text-field/NativeTextArea'
import { DesktopError } from '../../shared/errors/DesktopError'
import { AiMarkdown } from './AiMarkdown'

interface AiConversationProps {
  autoFocus?: boolean
  configured: boolean
  messages: AiMessage[]
  readOnly: boolean
  webSearchAvailable: boolean
  onActivate?(): void
  onSend(
    input: { content: string; webSearchEnabled: boolean },
    onDelta: (delta: AiStreamDelta) => void,
  ): Promise<AiMessage[]>
  onRegenerate(messageId: string, onDelta: (delta: AiStreamDelta) => void): Promise<AiMessage[]>
  onCopy(content: string): Promise<void>
  onCancel(): void
  onOpenSettings(): void
  onOpenCitation(url: string): void
}

export function AiConversation({
  autoFocus = false,
  configured,
  messages,
  readOnly,
  webSearchAvailable,
  onActivate,
  onSend,
  onRegenerate,
  onCopy,
  onCancel,
  onOpenSettings,
  onOpenCitation,
}: AiConversationProps) {
  const [thread, setThread] = useState(messages)
  const [content, setContent] = useState('')
  const [webSearchEnabled, setWebSearchEnabled] = useState(false)
  const [isSending, setIsSending] = useState(false)
  const [pendingTurn, setPendingTurn] = useState<{
    request: number
    question: string
    answer: string
    reasoning: string
    origin: 'send' | 'regenerate'
    replaceMessageId: string | null
  } | null>(null)
  const [copiedMessageId, setCopiedMessageId] = useState<string | null>(null)
  const [error, setError] = useState<DesktopError | null>(null)
  const activeRequest = useRef(0)
  const copyResetTimer = useRef<number | null>(null)
  const composerInput = useRef<HTMLTextAreaElement>(null)
  const composerForm = useRef<HTMLFormElement>(null)
  const messageList = useRef<HTMLDivElement>(null)

  useEffect(() => setThread(messages), [messages])
  useEffect(() => () => {
    if (copyResetTimer.current !== null) window.clearTimeout(copyResetTimer.current)
  }, [])
  useLayoutEffect(() => {
    const list = messageList.current
    if (list) list.scrollTop = list.scrollHeight
  }, [pendingTurn, thread])
  useLayoutEffect(() => {
    const input = composerInput.current
    if (!input) return
    input.style.height = 'auto'
    const contentHeight = Math.max(44, Math.min(input.scrollHeight, 160))
    input.style.height = `${contentHeight}px`
    input.style.overflowY = input.scrollHeight > 160 ? 'auto' : 'hidden'
  }, [content])

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const question = content.trim()
    if (!question || isSending) return
    setError(null)
    setIsSending(true)
    const request = activeRequest.current + 1
    activeRequest.current = request
    setContent('')
    setPendingTurn({
      request,
      question,
      answer: '',
      reasoning: '',
      origin: 'send',
      replaceMessageId: null,
    })
    try {
      const nextThread = await onSend(
        {
          content: question,
          webSearchEnabled: webSearchAvailable && webSearchEnabled,
        },
        (event) => {
          if (activeRequest.current !== request || !event.delta) return
          setPendingTurn((current) => (
            current?.request === request
              ? {
                  ...current,
                  [event.kind]: current[event.kind] + event.delta,
                }
              : current
          ))
        },
      )
      if (activeRequest.current !== request) return
      setThread(nextThread)
      setPendingTurn(null)
    } catch (nextError) {
      if (activeRequest.current !== request) return
      setPendingTurn(null)
      setContent(question)
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      if (activeRequest.current === request) setIsSending(false)
    }
  }

  async function handleRegenerate(message: AiMessage) {
    if (isSending || readOnly || message.role !== 'assistant') return
    const messageIndex = thread.findIndex((item) => item.id === message.id)
    const question = messageIndex > 0 && thread[messageIndex - 1].role === 'user'
      ? thread[messageIndex - 1].content
      : ''
    if (!question) return
    const request = activeRequest.current + 1
    activeRequest.current = request
    setError(null)
    setIsSending(true)
    setPendingTurn({
      request,
      question,
      answer: '',
      reasoning: '',
      origin: 'regenerate',
      replaceMessageId: messageIndex + 1 === thread.length ? message.id : null,
    })
    try {
      const nextThread = await onRegenerate(message.id, (event) => {
        if (activeRequest.current !== request || !event.delta) return
        setPendingTurn((current) => (
          current?.request === request
            ? {
                ...current,
                [event.kind]: current[event.kind] + event.delta,
              }
            : current
        ))
      })
      if (activeRequest.current !== request) return
      setThread(nextThread)
      setPendingTurn(null)
    } catch (nextError) {
      if (activeRequest.current !== request) return
      setPendingTurn(null)
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      if (activeRequest.current === request) setIsSending(false)
    }
  }

  async function handleCopy(message: AiMessage) {
    try {
      await onCopy(message.content)
      setCopiedMessageId(message.id)
      if (copyResetTimer.current !== null) window.clearTimeout(copyResetTimer.current)
      copyResetTimer.current = window.setTimeout(() => {
        setCopiedMessageId((current) => current === message.id ? null : current)
      }, 1_500)
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
    }
  }

  function cancelRequest() {
    const question = pendingTurn?.origin === 'send' ? pendingTurn.question : ''
    activeRequest.current += 1
    setIsSending(false)
    setPendingTurn(null)
    if (question) setContent(question)
    onCancel()
  }

  function handleComposerKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (
      event.key !== 'Enter'
      || event.shiftKey
      || event.nativeEvent.isComposing
      || isSending
    ) {
      return
    }
    event.preventDefault()
    event.currentTarget.form?.requestSubmit()
  }

  if (!configured && !readOnly) {
    return (
      <div className="ai-setup-empty">
        <span>尚未连接模型</span>
        <button className="button is-secondary" type="button" onClick={onOpenSettings}>
          打开模型设置
        </button>
      </div>
    )
  }

  return (
    <div className="ai-conversation">
      <div aria-label="AI 对话记录" className="ai-message-list" ref={messageList}>
        {thread.length === 0 && !pendingTurn
          ? <p className="ai-conversation-empty">还没有 AI 讨论记录</p>
          : null}
        {thread.map((message) => {
          const replacing = message.role === 'assistant'
            && pendingTurn?.replaceMessageId === message.id
          return (
            <article
              aria-label={message.role === 'user' ? '我的消息' : 'AI 回复'}
              aria-live={replacing ? 'polite' : undefined}
              className={`ai-message is-${message.role}${replacing ? ' is-streaming' : ''}`}
              key={message.id}
            >
              {message.role === 'user' ? <p>{message.content}</p> : (
                <AssistantResponse
                  answer={replacing ? pendingTurn.answer : message.content}
                  citations={replacing ? [] : message.citations}
                  copied={copiedMessageId === message.id}
                  isStreaming={replacing}
                  reasoning={replacing ? pendingTurn.reasoning : message.reasoningContent}
                  regenerateDisabled={readOnly}
                  showActions={!replacing}
                  onCopy={() => void handleCopy(message)}
                  onOpenCitation={onOpenCitation}
                  onRegenerate={() => void handleRegenerate(message)}
                />
              )}
            </article>
          )
        })}
        {pendingTurn && pendingTurn.replaceMessageId === null ? (
          <>
            <article aria-label="我的消息" className="ai-message is-user is-pending">
              <p>{pendingTurn.question}</p>
            </article>
            <article
              aria-label="AI 回复"
              aria-live="polite"
              className="ai-message is-assistant is-streaming"
            >
              <AssistantResponse
                answer={pendingTurn.answer}
                citations={[]}
                copied={false}
                isStreaming
                reasoning={pendingTurn.reasoning}
                regenerateDisabled
                showActions={false}
                onCopy={() => undefined}
                onOpenCitation={onOpenCitation}
                onRegenerate={() => undefined}
              />
            </article>
          </>
        ) : null}
      </div>

      {error ? (
        <div className="inline-alert is-error" role="alert">
          <AlertCircle aria-hidden="true" size={15} />
          <div>
            <strong>{error.message}</strong>
            {error.recovery ? <span>{error.recovery}</span> : null}
          </div>
        </div>
      ) : null}

      {!readOnly ? (
        <form className="ai-composer" ref={composerForm} onSubmit={handleSubmit}>
          <label className="sr-only" htmlFor="ai-question">和 AI 讨论</label>
          <NativeTextArea
            autoFocus={autoFocus && configured}
            className="ai-composer-input"
            fieldId="ai-question-field"
            id="ai-question"
            nativeInsets={{ top: 5, right: 4, left: 4 }}
            ref={composerInput}
            rows={1}
            submitOnEnter={!isSending}
            value={content}
            onChange={setContent}
            onFocus={onActivate}
            onKeyDown={handleComposerKeyDown}
            onSubmit={() => composerForm.current?.requestSubmit()}
          />
          <div>
            <label className="settings-toggle">
              <input
                checked={webSearchEnabled}
                disabled={!webSearchAvailable}
                type="checkbox"
                onChange={(event) => setWebSearchEnabled(event.target.checked)}
              />
              <span>联网查询</span>
            </label>
            {isSending ? (
              <button className="icon-button" title="停止生成" type="button" onClick={cancelRequest}>
                <Square aria-hidden="true" size={14} />
              </button>
            ) : (
              <button
                aria-label="发送"
                className="icon-button is-primary"
                disabled={!content.trim()}
                title="发送"
                type="submit"
              >
                <Send aria-hidden="true" size={15} />
              </button>
            )}
          </div>
        </form>
      ) : null}
    </div>
  )
}

interface AssistantResponseProps {
  answer: string
  citations: AiMessage['citations']
  copied: boolean
  isStreaming: boolean
  reasoning: string
  regenerateDisabled: boolean
  showActions: boolean
  onCopy(): void
  onOpenCitation(url: string): void
  onRegenerate(): void
}

function AssistantResponse({
  answer,
  citations,
  copied,
  isStreaming,
  reasoning,
  regenerateDisabled,
  showActions,
  onCopy,
  onOpenCitation,
  onRegenerate,
}: AssistantResponseProps) {
  const hasOutput = Boolean(answer || reasoning)
  return (
    <>
      {reasoning ? (
        <details className={`ai-reasoning${isStreaming ? ' is-streaming' : ''}`} open={isStreaming && !answer}>
          <summary>{isStreaming && !answer ? '正在思考' : '思考过程'}</summary>
          <div>{reasoning}</div>
        </details>
      ) : null}
      {answer ? <AiMarkdown content={answer} onOpenLink={onOpenCitation} /> : null}
      {!hasOutput && isStreaming ? <StreamingIndicator /> : null}
      {citations.length > 0 ? (
        <div aria-label="回答来源" className="ai-citations">
          <span>来源</span>
          {citations.map((citation) => (
            <button
              key={`${citation.position}:${citation.url}`}
              type="button"
              onClick={() => onOpenCitation(citation.url)}
            >
              <ExternalLink aria-hidden="true" size={12} />
              {citation.title || citation.url}
            </button>
          ))}
        </div>
      ) : null}
      {showActions ? (
        <div className="ai-message-actions">
          <button aria-label="复制回答" title="复制回答" type="button" onClick={onCopy}>
            {copied ? <Check aria-hidden="true" size={12} /> : <Copy aria-hidden="true" size={12} />}
            <span>{copied ? '已复制' : '复制'}</span>
          </button>
          <button
            aria-label="重新生成回答"
            disabled={regenerateDisabled}
            title={regenerateDisabled ? '已归档的卡片不能重新生成' : '重新生成回答'}
            type="button"
            onClick={onRegenerate}
          >
            <RefreshCw aria-hidden="true" size={12} />
            <span>重新生成</span>
          </button>
        </div>
      ) : null}
    </>
  )
}

function StreamingIndicator() {
  return (
    <span aria-label="AI 正在思考" className="ai-stream-waiting" role="status">
      <i />
      <i />
      <i />
    </span>
  )
}
