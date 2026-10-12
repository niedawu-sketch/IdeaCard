import { useCallback } from 'react'
import { DEFAULT_CONCEPT_SYSTEM_PROMPT, type ConceptAssistantSettings } from '../../entities/ai/model'
import { PromptSettingsDialog } from './PromptSettingsDialog'

interface ConceptAssistantSettingsDialogProps {
  settings: ConceptAssistantSettings
  onClose(): void
  onSave(input: ConceptAssistantSettings): Promise<ConceptAssistantSettings>
}

export function ConceptAssistantSettingsDialog({ settings, onClose, onSave }: ConceptAssistantSettingsDialogProps) {
  const onLoad = useCallback(async () => settings, [settings])
  return <PromptSettingsDialog title="AI 概念助手"
    description="解释选中概念时使用，自动附加卡片标题和概念附近的原文。"
    defaultPrompt={DEFAULT_CONCEPT_SYSTEM_PROMPT} onLoad={onLoad} onSave={onSave} onClose={onClose} />
}
