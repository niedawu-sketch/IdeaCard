import { describe, expect, it } from 'vitest'

import {
  createFallbackTitle,
  getCardOrganizationStatus,
  sourceTypeLabel,
  sourceTypeOptions,
  totalContextCharacters,
  visibleSourceType,
} from './model'

describe('card model', () => {
  it('creates a 30-character title from the first normalized line', () => {
    expect(createFallbackTitle('  第一行   有空格\n第二行  ')).toBe('第一行 有空格')
    expect(createFallbackTitle('一'.repeat(35))).toBe('一'.repeat(30))
  })

  it('exposes only the two user-facing source types', () => {
    expect(sourceTypeOptions.map((item) => item.value)).toEqual(['personal', 'document'])
  })

  it('maps legacy format-based sources into the visible source taxonomy', () => {
    expect(visibleSourceType('web')).toBe('document')
    expect(sourceTypeLabel('web')).toBe('文档资料')
    expect(visibleSourceType('image')).toBe('personal')
    expect(sourceTypeLabel('image')).toBe('个人记录')
    expect(visibleSourceType('media')).toBe('document')
    expect(sourceTypeLabel('media')).toBe('文档资料')
  })

  it.each([
    ['unprocessed', { value: 'unprocessed', label: '待整理' }],
    ['annotated', { value: 'organized', label: '已整理' }],
    ['pending_analysis', { value: 'organized', label: '已整理' }],
    ['concluded', { value: 'organized', label: '已整理' }],
    ['archived', { value: 'organized', label: '已整理' }],
  ] as const)('maps %s to its organization status', (status, expected) => {
    expect(getCardOrganizationStatus(status)).toEqual(expected)
  })

  it('counts title, original content, and note for AI context', () => {
    expect(totalContextCharacters([{ title: '题', originalContent: '正文', note: '想法' }])).toBe(5)
  })

  it('counts emoji as one character and never splits one in a fallback title', () => {
    expect(createFallbackTitle('😀'.repeat(31))).toBe('😀'.repeat(30))
    expect(totalContextCharacters([{ title: '😀', originalContent: '📚', note: '💡' }])).toBe(3)
  })

  it.each([
    ['family emoji', '👨‍👩‍👧‍👦'],
    ['flag emoji', '🇨🇳'],
    ['skin-tone emoji', '👍🏽'],
    ['combining character', 'e\u0301'],
  ])('truncates %s by grapheme cluster', (_, grapheme) => {
    expect(createFallbackTitle(grapheme.repeat(31))).toBe(grapheme.repeat(30))
  })

  it('normalizes BOM and NEL whitespace consistently', () => {
    expect(createFallbackTitle('\uFEFF第一\uFEFF\uFEFF第二')).toBe('第一 第二')
    expect(createFallbackTitle('\u0085第一\u0085\u0085第二')).toBe('第一 第二')
  })
})
