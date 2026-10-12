import { describe, expect, it } from 'vitest'

import { defaultAiSettings, settingsSupportWebSearch } from './model'

describe('AI model settings', () => {
  it('uses stable provider defaults and limits native web search by provider and model', () => {
    expect(defaultAiSettings('openAi')).toMatchObject({
      baseUrl: 'https://api.openai.com/v1',
      model: 'gpt-5.6',
      webSearchEnabled: true,
    })
    expect(defaultAiSettings('openAiCompatible')).toMatchObject({
      baseUrl: '',
      webSearchEnabled: false,
    })
    expect(defaultAiSettings('deepSeek')).toMatchObject({
      baseUrl: 'https://api.deepseek.com',
      model: 'deepseek-v4-flash',
      webSearchEnabled: true,
    })
    expect(defaultAiSettings('anthropic')).toMatchObject({
      baseUrl: 'https://api.anthropic.com/v1',
      webSearchEnabled: false,
    })
    expect(settingsSupportWebSearch('openAi', 'gpt-5.6')).toBe(true)
    expect(settingsSupportWebSearch('deepSeek', 'deepseek-v4-flash')).toBe(true)
    expect(settingsSupportWebSearch('deepSeek', ' DEEPSEEK-V4-FLASH ')).toBe(true)
    expect(settingsSupportWebSearch('deepSeek', 'deepseek-v4-pro')).toBe(false)
    expect(settingsSupportWebSearch('openAiCompatible', 'deepseek-v4-flash')).toBe(false)
    expect(settingsSupportWebSearch('anthropic', 'claude-sonnet-4-6')).toBe(false)
  })
})
