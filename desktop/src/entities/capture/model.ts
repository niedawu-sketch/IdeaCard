import { sourceTypeLabel, type SourceType } from '../card/model'

export type CaptureKind = 'quick' | 'clipboard' | 'screenshot' | 'file'

export interface CaptureDraft {
  kind: CaptureKind
  title: string
  content: string
  note: string
  sourceType: SourceType
  sourceName: string
  sourceUri: string
  temporaryAttachmentIds: string[]
}

export type CaptureState = 'loading' | 'ready' | 'saving' | 'failed'

interface CaptureSaveInput {
  content: string
  attachmentCount: number
}

export function canSaveCaptureDraft({ content, attachmentCount }: CaptureSaveInput) {
  return content.trim().length > 0 || attachmentCount > 0
}

export function captureSourceLabel(sourceType: SourceType) {
  return sourceTypeLabel(sourceType)
}
