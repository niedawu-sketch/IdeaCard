import type { Card, CardStatus, NewCardInput, SourceType } from '../../entities/card/model'
import type { CaptureDraft } from '../../entities/capture/model'
import type { RichDocument } from '../../entities/document/model'
import type {
  AiCitation,
  AiMessage,
  AiProvider,
  AiSettings,
  AiSettingsInput,
  ConceptAssistantSettings,
  TitleGenerationSettings,
} from '../../entities/ai/model'
import type {
  CardConcept,
  ConceptDetail,
  ConceptSummary,
  SaveConceptAnnotationInput,
  SavedConceptAnnotation,
} from '../../entities/concept/model'

export interface CardPatch {
  title?: string
  originalContent?: string
  note?: string
  sourceType?: SourceType
  sourceName?: string
  sourceUri?: string
  status?: CardStatus
}

export interface CreateCardResult {
  card: Card
  duplicateCardIds: string[]
}

export interface CardDocument {
  cardId: string
  document: RichDocument
  schemaVersion: number
  updatedAt: string
}

export interface SavedCardDocument {
  cardId: string
  document: RichDocument
  originalContent: string
  status: CardStatus
  updatedAt: string
}

export interface InlineImageData {
  mimeType: string
  dataUrl: string
}

export interface CreateRichCardInput {
  title: string
  note: string
  sourceType: SourceType
  captureMethod: 'quick' | 'clipboard' | 'screenshot' | 'file'
  sourceName: string
  sourceUri: string
  document: RichDocument
}

export interface SendCardAiInput {
  cardId: string
  content: string
  webSearchEnabled: boolean
  requestId: string
}

export interface RegenerateCardAiInput {
  cardId: string
  messageId: string
  requestId: string
}

export interface AiStreamDelta {
  kind: 'reasoning' | 'answer'
  delta: string
}

export interface NativeTextFieldInput {
  fieldId: string
  value: string
  placeholder: string
  readOnly: boolean
  visible: boolean
  x: number
  y: number
  width: number
  height: number
}

export interface NativeTextAreaInput {
  fieldId: string
  value: string
  readOnly: boolean
  submitOnEnter: boolean
  visible: boolean
  x: number
  y: number
  width: number
  height: number
}

export interface ExplainConceptInput {
  cardId: string
  selectedText: string
  webSearchEnabled: boolean
  requestId: string
}

export interface ConceptExplanation {
  selectedText: string
  explanation: string
  citations: Omit<AiCitation, 'position'>[]
}

export type FileImportStatus = 'saved' | 'saved_with_extraction_failure' | 'failed'

export interface FileImportResult {
  originalName: string
  status: FileImportStatus
  card: Card | null
  errorCode: string | null
}

export interface CaptureSystemStatus {
  screenPermission: 'granted' | 'not_granted'
  shortcuts: Array<{
    shortcut: string
    action: 'quick' | 'clipboard' | 'screenshot'
    registered: boolean
  }>
}

export interface DesktopService {
  listCards(includeArchived?: boolean): Promise<Card[]>
  getCard(id: string): Promise<Card>
  createCard(input: NewCardInput): Promise<CreateCardResult>
  createRichCard(input: CreateRichCardInput): Promise<CreateCardResult>
  getCardDocument(cardId: string): Promise<CardDocument>
  saveCardDocument(cardId: string, document: RichDocument): Promise<SavedCardDocument>
  loadInlineImage(attachmentId: string): Promise<InlineImageData>
  listConcepts(): Promise<ConceptSummary[]>
  getConcept(id: string): Promise<ConceptDetail>
  deleteConcept(id: string): Promise<void>
  listCardConcepts(cardId: string): Promise<CardConcept[]>
  saveConceptAnnotation(input: SaveConceptAnnotationInput): Promise<SavedConceptAnnotation>
  getAiSettings(): Promise<AiSettings>
  saveAiSettings(input: AiSettingsInput): Promise<AiSettings>
  generateCardTitle(cardId: string, originalContent: string): Promise<string>
  getTitleGenerationSettings(): Promise<TitleGenerationSettings>
  saveTitleGenerationSettings(input: TitleGenerationSettings): Promise<TitleGenerationSettings>
  getConceptAssistantSettings(): Promise<ConceptAssistantSettings>
  saveConceptAssistantSettings(input: ConceptAssistantSettings): Promise<ConceptAssistantSettings>
  removeAiKey(provider: AiProvider): Promise<AiSettings>
  testAiConnection(input: AiSettingsInput): Promise<{ status: 'connected' }>
  listCardAiMessages(cardId: string): Promise<AiMessage[]>
  sendCardAiMessage(
    input: SendCardAiInput,
    onDelta?: (delta: AiStreamDelta) => void,
  ): Promise<AiMessage[]>
  regenerateCardAiMessage(
    input: RegenerateCardAiInput,
    onDelta?: (delta: AiStreamDelta) => void,
  ): Promise<AiMessage[]>
  explainCardConcept(
    input: ExplainConceptInput,
    onDelta?: (delta: string) => void,
  ): Promise<ConceptExplanation>
  cancelAiRequest(requestId: string): Promise<void>
  openExternalUrl(url: string): Promise<void>
  updateCard(id: string, patch: CardPatch): Promise<Card>
  archiveCard(id: string): Promise<Card>
  restoreCard(id: string): Promise<Card>
  deleteCard(id: string): Promise<void>
  readClipboardDraft(): Promise<CaptureDraft>
  saveCaptureDraft(draft: CaptureDraft): Promise<Card>
  selectAndImportFiles(): Promise<FileImportResult[]>
  captureScreenshot(): Promise<{ status: 'saved'; card: Card } | { status: 'cancelled' }>
  getCaptureStatus(): Promise<CaptureSystemStatus>
  revealAttachment(id: string): Promise<void>
  readClipboardText(): Promise<string | null>
  writeClipboardText(text: string): Promise<void>
  syncNativeTextArea?(input: NativeTextAreaInput): Promise<void>
  removeNativeTextArea?(fieldId: string): Promise<void>
  focusNativeTextArea?(fieldId: string): Promise<void>
  syncNativeTextField?(input: NativeTextFieldInput): Promise<void>
  removeNativeTextField?(fieldId: string): Promise<void>
  focusNativeTextField?(fieldId: string): Promise<void>
  setVoiceInputTarget?(
    targetToken: string | null,
    targetRect?: {
      x: number
      y: number
      width: number
      height: number
    } | null,
  ): Promise<void>
  finishVoiceInputSession?(sessionId: number): Promise<void>
}
