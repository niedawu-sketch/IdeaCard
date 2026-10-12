import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'

import type { Card, NewCardInput } from '../../entities/card/model'
import type { CaptureDraft } from '../../entities/capture/model'
import type { RichDocument } from '../../entities/document/model'
import type {
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
import { DesktopError } from '../../shared/errors/DesktopError'
import type {
  CaptureSystemStatus,
  CardDocument,
  CardPatch,
  CreateCardResult,
  CreateRichCardInput,
  DesktopService,
  FileImportResult,
  InlineImageData,
  SavedCardDocument,
  ConceptExplanation,
  ExplainConceptInput,
  AiStreamDelta,
  RegenerateCardAiInput,
  SendCardAiInput,
} from './contracts'

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return args === undefined ? await invoke<T>(command) : await invoke<T>(command, args)
  } catch (error) {
    throw DesktopError.fromUnknown(error)
  }
}

const AI_TEXT_DELTA_EVENT = 'ai://text-delta'

interface AiTextDelta extends AiStreamDelta {
  requestId: string
}

async function invokeAiStream<T>(
  command: string,
  input: SendCardAiInput | RegenerateCardAiInput | ExplainConceptInput,
  onDelta?: (delta: AiStreamDelta) => void,
) {
  let unlisten: UnlistenFn | null = null
  try {
    if (onDelta) {
      unlisten = await listen<AiTextDelta>(AI_TEXT_DELTA_EVENT, (event) => {
        if (event.payload.requestId === input.requestId) {
          onDelta({ kind: event.payload.kind, delta: event.payload.delta })
        }
      })
    }
    return await invokeDesktop<T>(command, { input })
  } finally {
    unlisten?.()
  }
}

export const tauriDesktopService: DesktopService = {
  listCards(includeArchived = false) {
    return invokeDesktop<Card[]>('list_cards', { includeArchived })
  },

  getCard(id) {
    return invokeDesktop<Card>('get_card', { id })
  },

  createCard(input: NewCardInput) {
    return invokeDesktop<CreateCardResult>('create_card', { input })
  },

  createRichCard(input: CreateRichCardInput) {
    return invokeDesktop<CreateCardResult>('create_rich_card', { input })
  },

  getCardDocument(cardId) {
    return invokeDesktop<CardDocument>('get_card_document', { cardId })
  },

  saveCardDocument(cardId: string, document: RichDocument) {
    return invokeDesktop<SavedCardDocument>('save_card_document', { cardId, document })
  },

  loadInlineImage(attachmentId) {
    return invokeDesktop<InlineImageData>('load_inline_image', { attachmentId })
  },

  listConcepts() {
    return invokeDesktop<ConceptSummary[]>('list_concepts')
  },

  getConcept(id) {
    return invokeDesktop<ConceptDetail>('get_concept', { id })
  },

  deleteConcept(id) {
    return invokeDesktop<void>('delete_concept', { id })
  },

  listCardConcepts(cardId) {
    return invokeDesktop<CardConcept[]>('list_card_concepts', { cardId })
  },

  saveConceptAnnotation(input: SaveConceptAnnotationInput) {
    return invokeDesktop<SavedConceptAnnotation>('save_concept_annotation', { input })
  },

  getAiSettings() {
    return invokeDesktop<AiSettings>('get_ai_settings')
  },

  saveAiSettings(input: AiSettingsInput) {
    return invokeDesktop<AiSettings>('save_ai_settings', { input })
  },

  getConceptAssistantSettings() {
    return invokeDesktop<ConceptAssistantSettings>('get_concept_assistant_settings')
  },

  saveConceptAssistantSettings(input: ConceptAssistantSettings) {
    return invokeDesktop<ConceptAssistantSettings>('save_concept_assistant_settings', { input })
  },

  generateCardTitle(cardId: string, originalContent: string) {
    return invokeDesktop<string>('generate_card_title', { cardId, originalContent })
  },

  getTitleGenerationSettings() {
    return invokeDesktop<TitleGenerationSettings>('get_title_generation_settings')
  },

  saveTitleGenerationSettings(input: TitleGenerationSettings) {
    return invokeDesktop<TitleGenerationSettings>('save_title_generation_settings', { input })
  },

  removeAiKey(provider: AiProvider) {
    return invokeDesktop<AiSettings>('remove_ai_key', { provider })
  },

  testAiConnection(input: AiSettingsInput) {
    return invokeDesktop<{ status: 'connected' }>('test_ai_connection', { input })
  },

  listCardAiMessages(cardId) {
    return invokeDesktop<AiMessage[]>('list_card_ai_messages', { cardId })
  },

  sendCardAiMessage(input: SendCardAiInput, onDelta?: (delta: AiStreamDelta) => void) {
    return invokeAiStream<AiMessage[]>('send_card_ai_message', input, onDelta)
  },

  regenerateCardAiMessage(input, onDelta) {
    return invokeAiStream<AiMessage[]>('regenerate_card_ai_message', input, onDelta)
  },

  explainCardConcept(input: ExplainConceptInput, onDelta?: (delta: string) => void) {
    return invokeAiStream<ConceptExplanation>(
      'explain_card_concept',
      input,
      onDelta ? (event) => {
        if (event.kind === 'answer') onDelta(event.delta)
      } : undefined,
    )
  },

  cancelAiRequest(requestId) {
    return invokeDesktop<void>('cancel_ai_request', { requestId })
  },

  openExternalUrl(url) {
    return invokeDesktop<void>('open_external_url', { url })
  },

  updateCard(id: string, patch: CardPatch) {
    return invokeDesktop<Card>('update_card', { id, patch })
  },

  archiveCard(id) {
    return invokeDesktop<Card>('archive_card', { id })
  },

  restoreCard(id) {
    return invokeDesktop<Card>('restore_card', { id })
  },

  deleteCard(id) {
    return invokeDesktop<void>('delete_card', { id })
  },

  readClipboardDraft() {
    return invokeDesktop<CaptureDraft>('read_clipboard_draft')
  },

  saveCaptureDraft(draft) {
    return invokeDesktop<Card>('save_capture_draft', { draft })
  },

  async selectAndImportFiles() {
    const selected = await open({
      multiple: true,
      directory: false,
      filters: [{
        name: '支持的材料',
        extensions: ['pdf', 'docx', 'pptx', 'xlsx', 'md', 'markdown', 'txt', 'png', 'jpg', 'jpeg', 'heic', 'mp3', 'm4a', 'wav', 'aac', 'flac', 'mp4', 'mov', 'm4v', 'webm'],
      }],
    })
    if (!selected) return []
    const paths = Array.isArray(selected) ? selected.slice(0, 20) : [selected]
    return invokeDesktop<FileImportResult[]>('import_files', { paths })
  },

  captureScreenshot() {
    return invokeDesktop<{ status: 'saved'; card: Card } | { status: 'cancelled' }>('capture_screenshot')
  },

  getCaptureStatus() {
    return invokeDesktop<CaptureSystemStatus>('get_capture_status')
  },

  revealAttachment(id) {
    return invokeDesktop<void>('reveal_attachment', { id })
  },

  readClipboardText() {
    return invokeDesktop<string | null>('read_clipboard_text')
  },

  writeClipboardText(text) {
    return invokeDesktop<void>('write_clipboard_text', { text })
  },

  syncNativeTextArea(input) {
    return invokeDesktop<void>('sync_native_text_area', { input })
  },

  removeNativeTextArea(fieldId) {
    return invokeDesktop<void>('remove_native_text_area', { fieldId })
  },

  focusNativeTextArea(fieldId) {
    return invokeDesktop<void>('focus_native_text_area', { fieldId })
  },

  syncNativeTextField(input) {
    return invokeDesktop<void>('sync_native_text_field', { input })
  },

  removeNativeTextField(fieldId) {
    return invokeDesktop<void>('remove_native_text_field', { fieldId })
  },

  focusNativeTextField(fieldId) {
    return invokeDesktop<void>('focus_native_text_field', { fieldId })
  },

  setVoiceInputTarget(targetToken, targetRect) {
    return invokeDesktop<void>(
      'set_voice_input_target',
      targetRect ? { targetToken, targetRect } : { targetToken },
    )
  },

  finishVoiceInputSession(sessionId) {
    return invokeDesktop<void>('finish_voice_input_session', { sessionId })
  },
}
