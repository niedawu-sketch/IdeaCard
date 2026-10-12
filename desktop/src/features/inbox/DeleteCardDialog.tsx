import { AlertCircle, Trash2, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

import type { Card } from '../../entities/card/model'
import { DesktopError } from '../../shared/errors/DesktopError'

interface DeleteCardDialogProps {
  card: Card
  onClose(): void
  onConfirm(): Promise<void>
}

export function DeleteCardDialog({ card, onClose, onConfirm }: DeleteCardDialogProps) {
  const cancelButtonRef = useRef<HTMLButtonElement>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  const [error, setError] = useState<DesktopError | null>(null)

  useEffect(() => {
    cancelButtonRef.current?.focus()
  }, [])

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape' && !isDeleting) onClose()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isDeleting, onClose])

  async function handleDelete() {
    if (isDeleting) return
    setError(null)
    setIsDeleting(true)
    try {
      await onConfirm()
      onClose()
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
      setIsDeleting(false)
    }
  }

  return (
    <div className="dialog-backdrop">
      <section
        aria-labelledby="delete-card-dialog-title"
        aria-modal="true"
        className="quick-capture delete-card-dialog"
        role="dialog"
      >
        <header className="dialog-header">
          <div>
            <p>此操作不可撤销</p>
            <h2 id="delete-card-dialog-title">永久删除卡片</h2>
          </div>
          <button
            aria-label="关闭"
            className="icon-button subtle"
            disabled={isDeleting}
            title="关闭"
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" size={17} />
          </button>
        </header>
        <div className="delete-card-dialog-body">
          <strong>{card.title}</strong>
          <p>删除后无法恢复，卡片内容和本地附件将一并删除。</p>
          {error ? (
            <div className="inline-alert is-error" role="alert">
              <AlertCircle aria-hidden="true" size={16} />
              <div>
                <strong>{error.message}</strong>
                {error.recovery ? <span>{error.recovery}</span> : null}
              </div>
            </div>
          ) : null}
        </div>
        <footer className="dialog-actions">
          <button
            ref={cancelButtonRef}
            className="button is-secondary"
            disabled={isDeleting}
            type="button"
            onClick={onClose}
          >
            取消
          </button>
          <button
            className="button is-danger"
            disabled={isDeleting}
            type="button"
            onClick={() => void handleDelete()}
          >
            <Trash2 aria-hidden="true" size={15} />
            {isDeleting ? '删除中...' : '永久删除'}
          </button>
        </footer>
      </section>
    </div>
  )
}
