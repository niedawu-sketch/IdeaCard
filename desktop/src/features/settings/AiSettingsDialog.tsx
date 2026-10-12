import { AlertCircle, KeyRound, X } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'

import {
  defaultAiSettings,
  settingsSupportWebSearch,
  type AiProvider,
  type AiSettings,
  type AiSettingsInput,
} from '../../entities/ai/model'
import { DesktopError } from '../../shared/errors/DesktopError'

interface AiSettingsDialogProps {
  settings: AiSettings
  onClose(): void
  onSave(input: AiSettingsInput): Promise<AiSettings>
  onTest(input: AiSettingsInput): Promise<unknown>
  onRemoveKey(): Promise<AiSettings> | void
}

export function AiSettingsDialog({
  settings,
  onClose,
  onSave,
  onTest,
  onRemoveKey,
}: AiSettingsDialogProps) {
  const [value, setValue] = useState(settings)
  const [isSaving, setIsSaving] = useState(false)
  const [isTesting, setIsTesting] = useState(false)
  const [connectionPassed, setConnectionPassed] = useState(false)
  const [error, setError] = useState<DesktopError | null>(null)
  const apiKeyRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    setValue(settings)
  }, [settings])

  function currentInput(): AiSettingsInput {
    const apiKey = apiKeyRef.current?.value.trim()
    return {
      provider: value.provider,
      baseUrl: value.baseUrl.trim(),
      model: value.model.trim(),
      webSearchEnabled: settingsSupportWebSearch(value.provider, value.model)
        ? value.webSearchEnabled
        : false,
      ...(apiKey ? { apiKey } : {}),
    }
  }

  function changeProvider(provider: AiProvider) {
    setConnectionPassed(false)
    setError(null)
    setValue(defaultAiSettings(provider))
    if (apiKeyRef.current) apiKeyRef.current.value = ''
  }

  async function handleSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (isSaving) return
    setError(null)
    setIsSaving(true)
    try {
      const saved = await onSave(currentInput())
      setValue(saved)
      if (apiKeyRef.current) apiKeyRef.current.value = ''
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      setIsSaving(false)
    }
  }

  async function testConnection() {
    if (isTesting) return
    setError(null)
    setConnectionPassed(false)
    setIsTesting(true)
    try {
      await onTest(currentInput())
      setConnectionPassed(true)
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      setIsTesting(false)
    }
  }

  const webSearchAvailable = settingsSupportWebSearch(value.provider, value.model)
  const showDeepSeekSearchLimit = value.provider === 'deepSeek' && !webSearchAvailable

  return (
    <div className="dialog-backdrop">
      <section
        aria-labelledby="ai-settings-title"
        aria-modal="true"
        className="settings-dialog"
        role="dialog"
      >
        <header className="dialog-header">
          <div>
            <p>本机模型接入</p>
            <h2 id="ai-settings-title">模型设置</h2>
          </div>
          <button
            aria-label="关闭模型设置"
            className="icon-button"
            title="关闭"
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" size={17} />
          </button>
        </header>

        <form className="settings-form" onSubmit={handleSave}>
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
            <span>接入方式</span>
            <select
              value={value.provider}
              onChange={(event) => changeProvider(event.target.value as AiProvider)}
            >
              <option value="openAi">OpenAI</option>
              <option value="deepSeek">DeepSeek</option>
              <option value="openAiCompatible">OpenAI 兼容接口</option>
              <option value="anthropic">Anthropic Claude</option>
            </select>
          </label>
          <label className="form-field">
            <span>接口地址</span>
            <input
              required
              type="url"
              value={value.baseUrl}
              onChange={(event) => setValue({ ...value, baseUrl: event.target.value })}
            />
          </label>
          <label className="form-field">
            <span>模型</span>
            <input
              required
              type="text"
              value={value.model}
              onChange={(event) => {
                const model = event.target.value
                setValue({
                  ...value,
                  model,
                  webSearchEnabled: settingsSupportWebSearch(value.provider, model)
                    ? value.webSearchEnabled
                    : false,
                })
              }}
            />
          </label>
          <label className="form-field">
            <span className="settings-key-label">
              <span>API Key</span>
              {value.hasApiKey ? <em><KeyRound aria-hidden="true" size={12} />已保存</em> : null}
            </span>
            <input
              aria-label="API Key"
              autoComplete="off"
              name="ideacard-api-key"
              placeholder={value.hasApiKey ? '留空则继续使用已保存密钥' : '输入后仅保存到 macOS 钥匙串'}
              ref={apiKeyRef}
              type="password"
            />
          </label>

          <label className="settings-toggle">
            <input
              checked={value.webSearchEnabled}
              disabled={!webSearchAvailable}
              aria-describedby={showDeepSeekSearchLimit ? 'deepseek-search-limit' : undefined}
              type="checkbox"
              onChange={(event) => setValue({
                ...value,
                webSearchEnabled: event.target.checked,
              })}
            />
            <span>允许联网查询</span>
          </label>
          {showDeepSeekSearchLimit ? (
            <p className="settings-capability-note" id="deepseek-search-limit">
              DeepSeek 联网查询目前仅支持 deepseek-v4-flash
            </p>
          ) : null}

          {connectionPassed ? <p className="settings-success" role="status">连接成功</p> : null}

          <footer className="dialog-actions">
            {value.hasApiKey ? (
              <button
                className="button is-ghost settings-remove-key"
                type="button"
                onClick={() => void onRemoveKey()}
              >
                移除密钥
              </button>
            ) : null}
            <button
              className="button is-secondary"
              disabled={isTesting}
              type="button"
              onClick={() => void testConnection()}
            >
              {isTesting ? '测试中...' : '测试连接'}
            </button>
            <button className="button is-primary" disabled={isSaving} type="submit">
              {isSaving ? '保存中...' : '保存设置'}
            </button>
          </footer>
        </form>
      </section>
    </div>
  )
}
