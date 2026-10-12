import { AlertCircle, RefreshCcw, ScanText, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { FormEvent } from 'react'
import type { Card, NewCardInput } from '../../entities/card/model'
import { Button } from '../../shared/ui/Button'
import {
  type CaptureDraft,
  type CaptureKind,
  createEmptyCaptureDraft,
  isDuplicateCard,
  simulateCapture,
} from './captureAdapter'

interface CaptureDialogProps {
  kind: CaptureKind
  cards: Card[]
  onCreateCard: (input: NewCardInput) => void
  onClose: () => void
}

const dialogTitles = {
  quick: '快速记录',
  clipboard: '剪贴板捕获',
  screenshot: '截图捕获',
} as const

const loadingMessages = {
  clipboard: '正在读取模拟剪贴板...',
  screenshot: '正在识别模拟截图...',
} as const

export function CaptureDialog({
  kind,
  cards,
  onCreateCard,
  onClose,
}: CaptureDialogProps) {
  const [draft, setDraft] = useState<CaptureDraft>(() =>
    createEmptyCaptureDraft(kind),
  )
  const [isLoading, setIsLoading] = useState(kind === 'clipboard')
  const [error, setError] = useState('')
  const [duplicateVisible, setDuplicateVisible] = useState(false)
  const contentRef = useRef<HTMLTextAreaElement | null>(null)
  const isMountedRef = useRef(true)
  const captureRunRef = useRef(0)
  const titleId = `capture-dialog-title-${kind}`

  useEffect(() => {
    return () => {
      isMountedRef.current = false
      captureRunRef.current += 1
    }
  }, [])

  useEffect(() => {
    let active = true

    if (kind !== 'clipboard') {
      return () => {
        active = false
      }
    }

    const runId = captureRunRef.current + 1
    captureRunRef.current = runId

    simulateCapture('clipboard', 'success')
      .then((nextDraft) => {
        if (active && isCurrentCaptureRun(runId)) {
          setDraft(nextDraft)
        }
      })
      .catch((captureError: unknown) => {
        if (active && isCurrentCaptureRun(runId)) {
          setError(
            captureError instanceof Error
              ? captureError.message
              : '读取失败',
          )
        }
      })
      .finally(() => {
        if (active && isCurrentCaptureRun(runId)) {
          setIsLoading(false)
        }
      })

    return () => {
      active = false
    }
  }, [kind])

  function isCurrentCaptureRun(runId: number) {
    return isMountedRef.current && captureRunRef.current === runId
  }

  useEffect(() => {
    if (kind === 'quick' && !isLoading) {
      contentRef.current?.focus()
    }
  }, [isLoading, kind])

  function updateDraft(patch: Partial<CaptureDraft>) {
    setDraft((currentDraft) => ({ ...currentDraft, ...patch }))
    setDuplicateVisible(false)
  }

  function recognizeScreenshot(outcome: 'success' | 'failure') {
    const runId = captureRunRef.current + 1
    captureRunRef.current = runId

    setError('')
    setDuplicateVisible(false)
    setIsLoading(true)

    simulateCapture('screenshot', outcome)
      .then((nextDraft) => {
        if (isCurrentCaptureRun(runId)) {
          setDraft(nextDraft)
        }
      })
      .catch((captureError: unknown) => {
        if (isCurrentCaptureRun(runId)) {
          setError(
            captureError instanceof Error
              ? captureError.message
              : '识别失败',
          )
        }
      })
      .finally(() => {
        if (isCurrentCaptureRun(runId)) {
          setIsLoading(false)
        }
      })
  }

  function buildCardInput(): NewCardInput {
    const content = draft.content.trim()
    const title = draft.title.trim() || content.slice(0, 20)

    return {
      ...draft,
      title,
      content,
      note: draft.note.trim(),
      sourceName: draft.sourceName.trim(),
      sourceUrl: draft.sourceUrl.trim(),
    }
  }

  function saveDraft(input: NewCardInput) {
    onCreateCard(input)
    onClose()
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const input = buildCardInput()

    if (!input.content) {
      return
    }

    if (isDuplicateCard(cards, input)) {
      setDuplicateVisible(true)
      return
    }

    saveDraft(input)
  }

  const canSave = !isLoading && Boolean(draft.content.trim())

  return (
    <div className="capture-dialog-backdrop">
      <section
        aria-labelledby={titleId}
        aria-modal="true"
        className="capture-dialog"
        role="dialog"
      >
        <header className="capture-dialog-header">
          <div>
            <p className="capture-dialog-kicker">模拟捕获</p>
            <h2 id={titleId}>{dialogTitles[kind]}</h2>
          </div>
          <button
            aria-label="关闭"
            className="capture-dialog-close"
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" size={17} />
          </button>
        </header>

        {kind === 'screenshot' ? (
          <div className="capture-demo-actions">
            <Button
              disabled={isLoading}
              variant="secondary"
              onClick={() => recognizeScreenshot('success')}
            >
              <ScanText aria-hidden="true" size={15} />
              正常识别
            </Button>
            <Button
              disabled={isLoading}
              variant="ghost"
              onClick={() => recognizeScreenshot('failure')}
            >
              演示识别失败
            </Button>
          </div>
        ) : null}

        {isLoading ? (
          <p className="capture-loading">
            {kind === 'screenshot'
              ? loadingMessages.screenshot
              : loadingMessages.clipboard}
          </p>
        ) : null}

        {error ? (
          <div className="capture-alert" role="alert">
            <AlertCircle aria-hidden="true" size={16} />
            <span>{error}</span>
            {kind === 'screenshot' ? (
              <Button
                size="small"
                variant="secondary"
                onClick={() => recognizeScreenshot('success')}
              >
                <RefreshCcw aria-hidden="true" size={14} />
                重新识别
              </Button>
            ) : null}
          </div>
        ) : null}

        <form className="capture-form" onSubmit={handleSubmit}>
          <label className="capture-field">
            <span>标题</span>
            <input
              type="text"
              value={draft.title}
              onChange={(event) =>
                updateDraft({ title: event.target.value })
              }
            />
          </label>

          <label className="capture-field">
            <span>记录内容</span>
            <textarea
              ref={contentRef}
              rows={7}
              value={draft.content}
              onChange={(event) =>
                updateDraft({ content: event.target.value })
              }
            />
          </label>

          <label className="capture-field">
            <span>我的想法</span>
            <textarea
              rows={4}
              value={draft.note}
              onChange={(event) =>
                updateDraft({ note: event.target.value })
              }
            />
          </label>

          <div className="capture-source">
            <span>来源</span>
            <strong>{draft.sourceName || '未设置'}</strong>
          </div>

          {duplicateVisible ? (
            <div className="capture-alert capture-alert-warning" role="alert">
              <AlertCircle aria-hidden="true" size={16} />
              <span>可能重复：已有卡片使用相同来源或内容。</span>
              <Button
                size="small"
                variant="secondary"
                onClick={() => setDuplicateVisible(false)}
              >
                返回修改
              </Button>
              <Button
                size="small"
                onClick={() => saveDraft(buildCardInput())}
              >
                仍然保存
              </Button>
            </div>
          ) : null}

          <div className="capture-dialog-actions">
            <Button variant="ghost" onClick={onClose}>
              取消
            </Button>
            <Button disabled={!canSave} type="submit">
              保存
            </Button>
          </div>
        </form>
      </section>
    </div>
  )
}
