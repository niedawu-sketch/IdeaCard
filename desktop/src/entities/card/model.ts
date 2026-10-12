export type SourceType = 'personal' | 'web' | 'document' | 'image' | 'media'
export type VisibleSourceType = Exclude<SourceType, 'web' | 'image' | 'media'>
export type CaptureMethod = 'quick' | 'clipboard' | 'screenshot' | 'file'
export type CardStatus = 'unprocessed' | 'annotated' | 'pending_analysis' | 'concluded' | 'archived'
export type CardOrganizationStatus = 'unprocessed' | 'organized'
export type CardWorkspace = 'inbox' | 'archived'
export type ExtractionStatus = 'pending' | 'completed' | 'empty' | 'failed' | 'not_applicable'

export interface Attachment {
  id: string
  cardId: string
  kind: 'document' | 'image' | 'audio' | 'video'
  originalName: string
  relativePath: string
  mimeType: string
  byteSize: number
  extractionStatus: ExtractionStatus
  extractionError: string
  extractedText: string
  createdAt: string
}

export interface Card {
  id: string
  title: string
  originalContent: string
  note: string
  sourceType: SourceType
  captureMethod: CaptureMethod
  sourceName: string
  sourceUri: string
  status: CardStatus
  attachments: Attachment[]
  createdAt: string
  updatedAt: string
  archivedAt: string | null
}

export interface NewCardInput {
  title: string
  originalContent: string
  note: string
  sourceType: SourceType
  captureMethod: CaptureMethod
  sourceName: string
  sourceUri: string
}

export const sourceTypeOptions = [
  { value: 'personal', label: '个人记录' },
  { value: 'document', label: '文档资料' },
] as const satisfies ReadonlyArray<{ value: VisibleSourceType; label: string }>

export function visibleSourceType(sourceType: SourceType): VisibleSourceType {
  if (sourceType === 'image') return 'personal'
  if (sourceType === 'web' || sourceType === 'media') return 'document'
  return sourceType
}

export function sourceTypeLabel(sourceType: SourceType) {
  const visibleType = visibleSourceType(sourceType)
  return sourceTypeOptions.find((option) => option.value === visibleType)?.label ?? ''
}

export function getCardOrganizationStatus(status: CardStatus): {
  value: CardOrganizationStatus
  label: '待整理' | '已整理'
} {
  return status === 'unprocessed'
    ? { value: 'unprocessed', label: '待整理' }
    : { value: 'organized', label: '已整理' }
}

const titleGraphemeSegmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' })
const titleWhitespacePattern = /[\s\u0085]+/g

export function createFallbackTitle(content: string) {
  const firstNormalizedLine = content.split(/\r?\n/, 1)[0].replace(titleWhitespacePattern, ' ').trim()

  return Array.from(titleGraphemeSegmenter.segment(firstNormalizedLine), ({ segment }) => segment)
    .slice(0, 30)
    .join('')
}

export function totalContextCharacters(cards: Array<Pick<Card, 'title' | 'originalContent' | 'note'>>) {
  return cards.reduce(
    (total, card) =>
      total +
      Array.from(card.title).length +
      Array.from(card.originalContent).length +
      Array.from(card.note).length,
    0,
  )
}
