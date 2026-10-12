import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { defaultAiSettings } from '../../entities/ai/model'
import { AiSettingsDialog } from './AiSettingsDialog'

describe('AiSettingsDialog', () => {
  it('never echoes a saved key and clears a newly saved key from the DOM', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue({
      ...defaultAiSettings('openAi'),
      hasApiKey: true,
    })
    render(
      <AiSettingsDialog
        settings={{ ...defaultAiSettings('openAi'), hasApiKey: true }}
        onClose={vi.fn()}
        onRemoveKey={vi.fn()}
        onSave={onSave}
        onTest={vi.fn()}
      />,
    )
    const dialog = screen.getByRole('dialog', { name: '模型设置' })
    const keyInput = within(dialog).getByLabelText('API Key')

    expect(keyInput).toHaveValue('')
    expect(within(dialog).getByText('已保存')).toBeInTheDocument()

    await user.type(keyInput, 'test-secret-value')
    await user.click(within(dialog).getByRole('button', { name: '保存设置' }))

    await waitFor(() => expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      apiKey: 'test-secret-value',
    })))
    expect(keyInput).toHaveValue('')
  })

  it('changes provider defaults and disables unsupported web search', async () => {
    const user = userEvent.setup()
    render(
      <AiSettingsDialog
        settings={defaultAiSettings('openAi')}
        onClose={vi.fn()}
        onRemoveKey={vi.fn()}
        onSave={vi.fn()}
        onTest={vi.fn()}
      />,
    )

    await user.selectOptions(screen.getByLabelText('接入方式'), 'anthropic')

    expect(screen.getByLabelText('接口地址')).toHaveValue('https://api.anthropic.com/v1')
    expect(screen.getByLabelText('模型')).toHaveValue('claude-sonnet-4-6')
    expect(screen.getByLabelText('允许联网查询')).toBeDisabled()
    expect(screen.getByLabelText('允许联网查询')).not.toBeChecked()
  })

  it('enables native search only for DeepSeek V4 Flash', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue({
      ...defaultAiSettings('deepSeek'),
      model: 'deepseek-v4-pro',
      webSearchEnabled: false,
    })
    render(
      <AiSettingsDialog
        settings={defaultAiSettings('openAi')}
        onClose={vi.fn()}
        onRemoveKey={vi.fn()}
        onSave={onSave}
        onTest={vi.fn()}
      />,
    )

    await user.selectOptions(screen.getByLabelText('接入方式'), 'deepSeek')

    expect(screen.getByLabelText('接口地址')).toHaveValue('https://api.deepseek.com')
    expect(screen.getByLabelText('模型')).toHaveValue('deepseek-v4-flash')
    expect(screen.getByLabelText('允许联网查询')).toBeEnabled()
    expect(screen.getByLabelText('允许联网查询')).toBeChecked()

    await user.clear(screen.getByLabelText('模型'))
    await user.type(screen.getByLabelText('模型'), 'deepseek-v4-pro')

    expect(screen.getByLabelText('允许联网查询')).toBeDisabled()
    expect(screen.getByLabelText('允许联网查询')).not.toBeChecked()
    expect(screen.getByText('DeepSeek 联网查询目前仅支持 deepseek-v4-flash'))
      .toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '保存设置' }))
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      provider: 'deepSeek',
      model: 'deepseek-v4-pro',
      webSearchEnabled: false,
    }))
  })

  it('tests the current unsaved connection settings without closing', async () => {
    const user = userEvent.setup()
    const onTest = vi.fn().mockResolvedValue(undefined)
    render(
      <AiSettingsDialog
        settings={defaultAiSettings('openAiCompatible')}
        onClose={vi.fn()}
        onRemoveKey={vi.fn()}
        onSave={vi.fn()}
        onTest={onTest}
      />,
    )

    await user.type(screen.getByLabelText('接口地址'), 'https://example.test/v1')
    await user.type(screen.getByLabelText('模型'), 'example-chat')
    await user.type(screen.getByLabelText('API Key'), 'temporary-key')
    await user.click(screen.getByRole('button', { name: '测试连接' }))

    expect(onTest).toHaveBeenCalledWith(expect.objectContaining({
      baseUrl: 'https://example.test/v1',
      model: 'example-chat',
      apiKey: 'temporary-key',
    }))
    expect(await screen.findByRole('status')).toHaveTextContent('连接成功')
  })

  it('reflects a key removal returned by the parent', async () => {
    const initial = { ...defaultAiSettings('openAi'), hasApiKey: true }
    const { rerender } = render(
      <AiSettingsDialog
        settings={initial}
        onClose={vi.fn()}
        onRemoveKey={vi.fn()}
        onSave={vi.fn()}
        onTest={vi.fn()}
      />,
    )
    expect(screen.getByText('已保存')).toBeInTheDocument()

    rerender(
      <AiSettingsDialog
        settings={{ ...initial, hasApiKey: false }}
        onClose={vi.fn()}
        onRemoveKey={vi.fn()}
        onSave={vi.fn()}
        onTest={vi.fn()}
      />,
    )

    expect(screen.queryByText('已保存')).not.toBeInTheDocument()
  })
})
