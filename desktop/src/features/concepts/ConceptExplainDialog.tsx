import { AiMarkdown } from '../ai/AiMarkdown'
import { AlertCircle, AlertTriangle, RefreshCw, X } from 'lucide-react'
import { useState, type FormEvent } from 'react'

import { NativeTextField } from '../native-text-field/NativeTextField'
import { DesktopError } from '../../shared/errors/DesktopError'

interface ConceptExplainDialogProps {
  onOpenLink(url: string): void
  selectedText: string
  explanation: string
  generationStatus: 'waiting' | 'streaming' | 'complete'
  overwriteConceptName?: string
  onCancel(): void
  onRegenerate(): void
  onSave(value: { name: string; explanation: string }): Promise<void>
}

export function ConceptExplainDialog({
  onOpenLink,
  selectedText,
  explanation,
  generationStatus,
  overwriteConceptName,
  onCancel,
  onRegenerate,
  onSave,
}: ConceptExplainDialogProps) {
  const [name, setName] = useState(overwriteConceptName ?? selectedText)
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<DesktopError | null>(null)
  const isGenerating = generationStatus !== 'complete'

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!name.trim() || isSaving || isGenerating) return
    setError(null)
    setIsSaving(true)
    try {
      await onSave({ name: name.trim(), explanation })
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <div className="dialog-backdrop">
      <section
        aria-labelledby="concept-explain-title"
        aria-modal="true"
        className="concept-explain-dialog"
        role="dialog"
      >
        <header className="dialog-header">
          <div>
            <p>AI 解释</p>
            <h2 id="concept-explain-title">解释概念</h2>
          </div>
          <button
            aria-label="关闭概念解释"
            className="icon-button"
            title="关闭"
            type="button"
            onClick={onCancel}
          >
            <X aria-hidden="true" size={17} />
          </button>
        </header>
        <form className="concept-explain-form" onSubmit={handleSubmit}>
          {overwriteConceptName ? (
            <div className="inline-alert is-warning" role="note">
              <AlertTriangle aria-hidden="true" size={15} />
              <div>
                <strong>将覆盖原有解释</strong>
                <span>保存后，所有引用「{overwriteConceptName}」的卡片都会显示这次的新解释。</span>
              </div>
            </div>
          ) : null}
          {error ? (
            <div className="inline-alert is-error" role="alert">
              <AlertCircle aria-hidden="true" size={15} />
              <div>
                <strong>{error.message}</strong>
                {error.recovery ? <span>{error.recovery}</span> : null}
              </div>
            </div>
          ) : null}
          <label className="form-field">
            <span>概念名称</span>
            <NativeTextField
              fieldId="concept-explain-name-field"
              readOnly={Boolean(overwriteConceptName)}
              required
              value={name}
              onChange={setName}
            />
          </label>
          <div className="concept-explain-preview">
            <span>解释</span>
            {generationStatus === 'waiting' ? (
              <span
                aria-label="AI 正在思考"
                className="ai-stream-waiting"
                role="status"
              >
                <i />
                <i />
                <i />
              </span>
            ) : (
              <div aria-live="polite"><AiMarkdown content={explanation} onOpenLink={onOpenLink} /></div>
            )}
          </div>
          <footer className="dialog-actions">
            <button className="button is-ghost" type="button" onClick={onCancel}>
              取消
            </button>
            <button
              className="button is-secondary"
              disabled={isGenerating}
              type="button"
              onClick={onRegenerate}
            >
              <RefreshCw aria-hidden="true" size={14} />
              重新生成
            </button>
            <button
              className="button is-primary"
              disabled={isSaving || isGenerating}
              type="submit"
            >
              {isSaving
                ? '保存中...'
                : isGenerating
                  ? '生成中...'
                  : overwriteConceptName
                    ? '覆盖原解释'
                    : '保存概念'}
            </button>
          </footer>
        </form>
      </section>
    </div>
  )
}
