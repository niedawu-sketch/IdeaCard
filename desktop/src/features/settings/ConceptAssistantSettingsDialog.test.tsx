import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { DEFAULT_CONCEPT_SYSTEM_PROMPT } from '../../entities/ai/model'
import { ConceptAssistantSettingsDialog } from './ConceptAssistantSettingsDialog'

describe('ConceptAssistantSettingsDialog', () => {
  it('loads, restores, and saves the concept system prompt', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue({
      systemPrompt: DEFAULT_CONCEPT_SYSTEM_PROMPT,
    })
    render(
      <ConceptAssistantSettingsDialog
        settings={{ systemPrompt: '自定义规则' }}
        onClose={vi.fn()}
        onSave={onSave}
      />,
    )

    const prompt = screen.getByLabelText('System Prompt')
    await waitFor(() => expect(prompt).toHaveValue('自定义规则'))

    await user.click(screen.getByRole('button', { name: '恢复默认' }))
    expect(prompt).toHaveValue(DEFAULT_CONCEPT_SYSTEM_PROMPT)

    await user.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() => expect(onSave).toHaveBeenCalledWith({
      systemPrompt: DEFAULT_CONCEPT_SYSTEM_PROMPT,
    }))
  })

  it('does not submit an empty prompt', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn()
    render(
      <ConceptAssistantSettingsDialog
        settings={{ systemPrompt: '原规则' }}
        onClose={vi.fn()}
        onSave={onSave}
      />,
    )

    await waitFor(() => expect(screen.getByLabelText('System Prompt')).toHaveValue('原规则'))
    await user.clear(screen.getByLabelText('System Prompt'))
    await user.click(screen.getByRole('button', { name: '保存' }))

    expect(onSave).not.toHaveBeenCalled()
  })
})
