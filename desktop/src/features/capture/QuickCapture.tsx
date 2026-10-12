import { AlertCircle, X } from 'lucide-react'
import { useState, type FormEvent } from 'react'

import {
  documentPlainText,
  emptyDocument,
  hasDocumentContent,
  type RichDocument,
} from '../../entities/document/model'
import { RichTextEditor } from '../editor/RichTextEditor'
import { NativeTextArea } from '../native-text-field/NativeTextArea'
import { NativeTextField } from '../native-text-field/NativeTextField'
import { DesktopError } from '../../shared/errors/DesktopError'

export interface RichQuickCardInput {
  title: string
  originalContent: string
  document: RichDocument
  note: string
}

interface QuickCaptureProps {
  onClose(): void
  onSave(input: RichQuickCardInput): Promise<unknown>
}

export function QuickCapture({ onClose, onSave }: QuickCaptureProps) {
  const [title, setTitle] = useState('')
  const [document, setDocument] = useState(emptyDocument)
  const [originalContent, setOriginalContent] = useState('')
  const [note, setNote] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [error, setError] = useState<DesktopError | null>(null)

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!hasDocumentContent(document) || isSubmitting) {
      return
    }

    setError(null)
    setIsSubmitting(true)
    try {
      await onSave({
        title: title.trim() || (originalContent ? '' : '图片记录'),
        originalContent,
        document,
        note,
      })
      onClose()
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div className="dialog-backdrop">
      <section
        aria-labelledby="quick-capture-title"
        aria-modal="true"
        className="quick-capture"
        role="dialog"
      >
        <header className="dialog-header">
          <div>
            <p>随手记下</p>
            <h2 id="quick-capture-title">快速记录</h2>
          </div>
          <button
            aria-label="关闭快速记录"
            className="icon-button"
            title="关闭"
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" size={17} />
          </button>
        </header>

        <form className="capture-form" onSubmit={handleSubmit}>
          {error ? (
            <div className="inline-alert is-error" role="alert">
              <AlertCircle aria-hidden="true" size={16} />
              <div>
                <strong>{error.message}</strong>
                {error.recovery ? <span>{error.recovery}</span> : null}
              </div>
            </div>
          ) : null}

          <label className="form-field">
            <span>标题</span>
            <NativeTextField
              fieldId="quick-capture-title-field"
              placeholder="留空时自动取正文首行"
              value={title}
              onChange={setTitle}
            />
          </label>

          <div className="form-field">
            <span>记录内容</span>
            <RichTextEditor
              autoFocus
              label="记录内容"
              value={document}
              onChange={(value) => {
                setDocument(value.document)
                setOriginalContent(documentPlainText(value.document))
              }}
            />
          </div>

          <label className="form-field">
            <span>我的想法</span>
            <NativeTextArea
              fieldId="quick-capture-note-field"
              rows={4}
              value={note}
              onChange={setNote}
            />
          </label>

          <footer className="dialog-actions">
            <button className="button is-ghost" type="button" onClick={onClose}>
              取消
            </button>
            <button
              className="button is-primary"
              disabled={!hasDocumentContent(document) || isSubmitting}
              type="submit"
            >
              {isSubmitting ? '保存中...' : '保存'}
            </button>
          </footer>
        </form>
      </section>
    </div>
  )
}
