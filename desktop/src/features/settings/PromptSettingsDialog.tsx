import { AlertCircle, X } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import type { ConceptAssistantSettings } from '../../entities/ai/model'
import { DesktopError } from '../../shared/errors/DesktopError'

interface PromptSettingsDialogProps {
  title: string
  description: string
  defaultPrompt: string
  onLoad(): Promise<ConceptAssistantSettings>
  onSave(input: ConceptAssistantSettings): Promise<ConceptAssistantSettings>
  onClose(): void
}

export function PromptSettingsDialog({
  title, description, defaultPrompt, onLoad, onSave, onClose,
}: PromptSettingsDialogProps) {
  const [systemPrompt, setSystemPrompt] = useState('')
  const [isLoading, setIsLoading] = useState(true)
  const [loaded, setLoaded] = useState(false)
  const [isSaving, setIsSaving] = useState(false)
  const [saved, setSaved] = useState(false)
  const [error, setError] = useState<DesktopError | null>(null)
  const [loadAttempt, setLoadAttempt] = useState(0)
  const dialogRef = useRef<HTMLElement>(null)

  useEffect(() => {
    let active = true
    void onLoad().then((settings) => {
      if (!active) return
      setSystemPrompt(settings.systemPrompt)
      setLoaded(true)
      setIsLoading(false)
    }).catch((nextError) => {
      if (!active) return
      setError(DesktopError.fromUnknown(nextError))
      setIsLoading(false)
    })
    return () => { active = false }
  }, [onLoad, loadAttempt])

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null
    dialogRef.current?.focus()
    return () => { previous?.focus() }
  }, [])

  async function handleSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!loaded || isSaving || !systemPrompt.trim()) return
    setIsSaving(true)
    setError(null)
    setSaved(false)
    try {
      const next = await onSave({ systemPrompt: systemPrompt.trim() })
      setSystemPrompt(next.systemPrompt)
      setSaved(true)
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <div className="dialog-backdrop">
      <section
        ref={dialogRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby="prompt-settings-title"
        aria-describedby="prompt-settings-description"
        className="settings-dialog concept-assistant-settings-dialog"
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.stopPropagation()
            onClose()
          }
          if (event.key !== 'Tab') return
          const elements = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>(
            'button:not(:disabled), textarea:not(:disabled)',
          ) ?? [])
          const first = elements[0]
          const last = elements.at(-1)
          if (event.shiftKey && (document.activeElement === first || document.activeElement === dialogRef.current)) {
            event.preventDefault()
            last?.focus()
          } else if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault()
            first?.focus()
          }
        }}
      >
        <header className="dialog-header">
          <div>
            <p>提示词配置</p>
            <h2 id="prompt-settings-title">{title}</h2>
          </div>
          <button aria-label={`关闭${title}`} className="icon-button" type="button" onClick={onClose}>
            <X aria-hidden="true" size={17} />
          </button>
        </header>
        <form className="settings-form" onSubmit={handleSave}>
          <p id="prompt-settings-description" className="prompt-settings-description">{description}</p>
          {error ? (
            <div className="inline-alert is-error" role="alert">
              <AlertCircle aria-hidden="true" size={16} />
              <div>
                <strong>{error.message}</strong>
                {error.recovery ? <span>{error.recovery}</span> : null}
                {!loaded ? <button className="button is-ghost" type="button" onClick={() => {
                  setError(null)
                  setIsLoading(true)
                  setLoadAttempt((attempt) => attempt + 1)
                }}>重新加载</button> : null}
              </div>
            </div>
          ) : null}
          <label className="form-field">
            <span>System Prompt</span>
            <textarea required maxLength={8_000} rows={12} value={systemPrompt}
              disabled={!loaded || isSaving} aria-busy={isLoading}
              placeholder={isLoading ? '正在读取提示词…' : undefined}
              onChange={(event) => { setSystemPrompt(event.target.value); setSaved(false) }} />
          </label>
          {saved ? <p className="settings-success" role="status">已保存，下次使用时生效</p> : null}
          <footer className="dialog-actions">
            <button className="button is-ghost settings-restore-default" type="button"
              disabled={!loaded || isSaving}
              onClick={() => { setSystemPrompt(defaultPrompt); setSaved(false); setError(null) }}>
              恢复默认
            </button>
            <button className="button is-primary" type="submit" disabled={!loaded || isSaving || !systemPrompt.trim()}>
              {isSaving ? '保存中...' : '保存'}
            </button>
          </footer>
        </form>
      </section>
    </div>
  )
}
