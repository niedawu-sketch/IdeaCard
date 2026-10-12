import { AlertCircle, X } from 'lucide-react'
import { useEffect, useState, type FormEvent } from 'react'

import { canSaveCaptureDraft, captureSourceLabel, type CaptureDraft } from '../../entities/capture/model'
import { NativeTextArea } from '../native-text-field/NativeTextArea'
import { NativeTextField } from '../native-text-field/NativeTextField'
import { DesktopError } from '../../shared/errors/DesktopError'

interface CaptureDialogProps {
  draft: CaptureDraft
  onClose(): void
  onSave(draft: CaptureDraft): Promise<unknown>
}

export function CaptureDialog({ draft, onClose, onSave }: CaptureDialogProps) {
  const [value, setValue] = useState(draft)
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<DesktopError | null>(null)
  const canSave = canSaveCaptureDraft({
    content: value.content,
    attachmentCount: value.temporaryAttachmentIds.length,
  })

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') onClose()
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onClose])

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!canSave || isSaving) return
    setError(null)
    setIsSaving(true)
    try {
      await onSave(value)
      onClose()
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <div className="dialog-backdrop">
      <section aria-labelledby="capture-dialog-title" aria-modal="true" className="quick-capture" role="dialog">
        <header className="dialog-header">
          <div>
            <p>检查后收进卡片</p>
            <h2 id="capture-dialog-title">确认捕获</h2>
          </div>
          <button aria-label="关闭确认捕获" className="icon-button" title="关闭" type="button" onClick={onClose}>
            <X aria-hidden="true" size={17} />
          </button>
        </header>
        <form className="capture-form" onSubmit={handleSubmit}>
          <div className="capture-source-row">
            <span>{captureSourceLabel(value.sourceType)}</span>
          </div>
          {error ? (
            <div className="inline-alert is-error" role="alert">
              <AlertCircle aria-hidden="true" size={16} />
              <div><strong>{error.message}</strong>{error.recovery ? <span>{error.recovery}</span> : null}</div>
            </div>
          ) : null}
          <label className="form-field">
            <span>标题</span>
            <NativeTextField
              fieldId="capture-dialog-title-field"
              value={value.title}
              onChange={(title) => setValue((current) => ({ ...current, title }))}
            />
          </label>
          <label className="form-field">
            <span>记录内容</span>
            <NativeTextArea
              autoFocus
              fieldId="capture-dialog-content-field"
              rows={7}
              value={value.content}
              onChange={(content) => setValue((current) => ({ ...current, content }))}
            />
          </label>
          <label className="form-field">
            <span>我的想法</span>
            <NativeTextArea
              fieldId="capture-dialog-note-field"
              rows={4}
              value={value.note}
              onChange={(note) => setValue((current) => ({ ...current, note }))}
            />
          </label>
          <footer className="dialog-actions">
            <button className="button is-ghost" type="button" onClick={onClose}>取消</button>
            <button className="button is-primary" disabled={!canSave || isSaving} type="submit">
              {isSaving ? '保存中...' : '保存'}
            </button>
          </footer>
        </form>
      </section>
    </div>
  )
}
