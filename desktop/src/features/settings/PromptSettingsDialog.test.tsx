import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { PromptSettingsDialog } from './PromptSettingsDialog'

const props = {
  title: '标题生成', description: '仅影响新卡片', defaultPrompt: '默认标题提示词',
  onClose: vi.fn(),
}

describe('PromptSettingsDialog', () => {
  it('keeps failed loads read-only and permits retry without overwriting settings', async () => {
    const user = userEvent.setup()
    const onLoad = vi.fn().mockRejectedValueOnce(new Error('读取失败')).mockResolvedValue({ systemPrompt: '已有规则' })
    const onSave = vi.fn()
    render(<PromptSettingsDialog {...props} onLoad={onLoad} onSave={onSave} />)
    await screen.findByRole('alert')
    expect(screen.getByRole('button', { name: '保存' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '恢复默认' })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: '重新加载' }))
    await waitFor(() => expect(screen.getByLabelText('System Prompt')).toHaveValue('已有规则'))
    expect(onSave).not.toHaveBeenCalled()
  })

  it('restores only the draft, preserves failed edits, and saves before confirming success', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockRejectedValueOnce(new Error('保存失败')).mockResolvedValue({ systemPrompt: props.defaultPrompt })
    render(<PromptSettingsDialog {...props} onLoad={vi.fn().mockResolvedValue({ systemPrompt: '当前规则' })} onSave={onSave} />)
    const input = screen.getByLabelText('System Prompt')
    await waitFor(() => expect(input).toHaveValue('当前规则'))
    await user.click(screen.getByRole('button', { name: '恢复默认' }))
    expect(input).toHaveValue(props.defaultPrompt)
    expect(onSave).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: '保存' }))
    await screen.findByRole('alert')
    expect(input).toHaveValue(props.defaultPrompt)
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '保存' }))
    await screen.findByRole('status')
    expect(onSave).toHaveBeenLastCalledWith({ systemPrompt: props.defaultPrompt })
  })
})
