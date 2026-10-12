import { AlertCircle, Archive, ExternalLink, RotateCcw, Sparkles, Trash2, X } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import {
  getCardOrganizationStatus,
  sourceTypeOptions,
  visibleSourceType,
  type Card,
  type CardWorkspace,
} from '../../entities/card/model'
import {
  documentFromPlainText,
  documentPlainText,
  type RichDocument,
} from '../../entities/document/model'
import {
  settingsSupportWebSearch,
  type AiMessage,
  type AiSettings,
} from '../../entities/ai/model'
import type {
  CardConcept,
  SaveConceptAnnotationInput,
  SavedConceptAnnotation,
} from '../../entities/concept/model'
import { DesktopError } from '../../shared/errors/DesktopError'
import { useDesktopStore } from '../../store/useDesktopStore'
import type {
  CardDocument,
  CardPatch,
  AiStreamDelta,
  ConceptExplanation,
  InlineImageData,
  RegenerateCardAiInput,
  SavedCardDocument,
  SendCardAiInput,
} from '../../services/desktop/contracts'
import { AiConversation } from '../ai/AiConversation'
import { ConceptExplainDialog } from '../concepts/ConceptExplainDialog'
import {
  RichTextEditor,
  type ConceptAnnotationRequest,
  type EditorSelectionSnapshot,
} from '../editor/RichTextEditor'
import { NativeTextField } from '../native-text-field/NativeTextField'
import { NativeTextArea } from '../native-text-field/NativeTextArea'

interface CardDetailProps {
  card: Card | null
  workspace: CardWorkspace
  onGenerateTitle(content: string): Promise<string>
  onSave(patch: CardPatch): Promise<unknown>
  onArchive(): Promise<unknown>
  onRestore(): Promise<unknown>
  onDeleteRequest(): void
  onRevealAttachment(id: string): Promise<void>
  onLoadDocument(cardId: string): Promise<CardDocument>
  onSaveDocument(cardId: string, document: RichDocument): Promise<SavedCardDocument>
  onLoadInlineImage(attachmentId: string): Promise<InlineImageData>
  onLoadConcepts(cardId: string): Promise<CardConcept[]>
  onExplainConcept(input: {
    cardId: string
    selectedText: string
    webSearchEnabled: boolean
    requestId: string
  }, onDelta?: (delta: string) => void): Promise<ConceptExplanation>
  onSaveConceptAnnotation(input: SaveConceptAnnotationInput): Promise<SavedConceptAnnotation>
  onOpenConcept(conceptId: string): void
  onLoadAiSettings(): Promise<AiSettings>
  onLoadAiMessages(cardId: string): Promise<AiMessage[]>
  onSendAiMessage(input: SendCardAiInput, onDelta?: (delta: AiStreamDelta) => void): Promise<AiMessage[]>
  onRegenerateAiMessage(
    input: RegenerateCardAiInput,
    onDelta?: (delta: AiStreamDelta) => void,
  ): Promise<AiMessage[]>
  onCopyText(content: string): Promise<void>
  onCancelAiRequest(requestId: string): Promise<void>
  onOpenAiSettings(): void
  onOpenExternalUrl(url: string): Promise<void>
  targetMentionId?: string | null
}

const AUTO_SAVE_DELAY_MS = 1_200

type AutoSaveState = 'saved' | 'pending' | 'saving' | 'error'

interface SaveSnapshot {
  patch: CardPatch
  document: RichDocument
  serializedDocument: string
  documentChanged: boolean
  hasChanges: boolean
  revision: number
  signature: string
}

function formatSaveTime(value: Date | string) {
  const date = value instanceof Date ? value : new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(date)
}

export function CardDetail(props: CardDetailProps) {
  const { card } = props
  if (!card) {
    return (
      <div className="card-detail-empty">
        <p>选择一张卡片开始整理</p>
      </div>
    )
  }

  return <CardDetailForm key={card.id} {...props} card={card} />
}

function CardDetailForm({
  card,
  workspace,
  onGenerateTitle,
  onSave,
  onArchive,
  onRestore,
  onDeleteRequest,
  onRevealAttachment,
  onLoadDocument,
  onSaveDocument,
  onLoadInlineImage,
  onLoadConcepts,
  onExplainConcept,
  onSaveConceptAnnotation,
  onOpenConcept,
  onLoadAiSettings,
  onLoadAiMessages,
  onSendAiMessage,
  onRegenerateAiMessage,
  onCopyText,
  onCancelAiRequest,
  onOpenAiSettings,
  onOpenExternalUrl,
  targetMentionId,
}: CardDetailProps & { card: Card }) {
  const [title, setTitle] = useState(card.title)
  const [isGeneratingTitle, setIsGeneratingTitle] = useState(false)
  const titleRequestPending = useRef(false)
  const [document, setDocument] = useState(() => documentFromPlainText(card.originalContent))
  const [savedDocument, setSavedDocument] = useState(() => JSON.stringify(document))
  const [note, setNote] = useState(card.note)
  const [sourceType, setSourceType] = useState(card.sourceType)
  const [sourceName, setSourceName] = useState(card.sourceName)
  const [sourceUri, setSourceUri] = useState(card.sourceUri)
  const [isSaving, setIsSaving] = useState(false)
  const [isLifecycleRunning, setIsLifecycleRunning] = useState(false)
  const [autoSaveState, setAutoSaveState] = useState<AutoSaveState>('saved')
  const [lastSavedAt, setLastSavedAt] = useState(() => new Date(card.updatedAt))
  const [isDocumentLoading, setIsDocumentLoading] = useState(true)
  const [error, setError] = useState<DesktopError | null>(null)
  const [aiSettings, setAiSettings] = useState<AiSettings | null>(null)
  const [aiMessages, setAiMessages] = useState<AiMessage[]>([])
  const [aiDrawerOpen, setAiDrawerOpen] = useState(false)
  const [concepts, setConcepts] = useState<CardConcept[]>([])
  const [conceptSelection, setConceptSelection] = useState<EditorSelectionSnapshot | null>(null)
  const [conceptExplanation, setConceptExplanation] = useState<ConceptExplanation | null>(null)
  const [conceptGenerationStatus, setConceptGenerationStatus] = useState<
    'waiting' | 'streaming' | 'complete'
  >('complete')
  const [annotationRequest, setAnnotationRequest] = useState<ConceptAnnotationRequest | null>(null)
  const conversationRequestId = useRef<string | null>(null)
  const conceptRequestId = useRef<string | null>(null)
  const aiSettingsRequest = useRef<Promise<AiSettings> | null>(null)
  const annotationCompletion = useRef<{
    input: Omit<SaveConceptAnnotationInput, 'document'>
    resolve(): void
    reject(error: unknown): void
  } | null>(null)
  const mounted = useRef(true)
  const saveInFlight = useRef(false)
  const queuedUnmountSave = useRef<SaveSnapshot | null>(null)
  const failedSignature = useRef<string | null>(null)
  const lastSavedSignature = useRef<string | null>(null)
  const editRevision = useRef(0)
  const saveHandlers = useRef({ onSave, onSaveDocument })
  const organizationStatus = getCardOrganizationStatus(card.status)
  const isArchived = workspace === 'archived'
  const serializedDocument = useMemo(() => JSON.stringify(document), [document])
  const documentChanged = serializedDocument !== savedDocument
  const overwriteConcept = useMemo(() => {
    if (!conceptSelection?.conceptId || !conceptSelection.mentionId) return null
    return concepts.find((concept) => (
      concept.id === conceptSelection.conceptId
      && concept.mentionId === conceptSelection.mentionId
    )) ?? null
  }, [conceptSelection, concepts])

  saveHandlers.current = { onSave, onSaveDocument }

  useEffect(() => {
    let active = true
    void onLoadDocument(card.id)
      .then((loaded) => {
        if (!active) return
        setDocument(loaded.document)
        setSavedDocument(JSON.stringify(loaded.document))
        setIsDocumentLoading(false)
      })
      .catch((nextError) => {
        if (active) {
          setError(DesktopError.fromUnknown(nextError))
          setIsDocumentLoading(false)
        }
      })
    return () => {
      active = false
    }
  }, [card.id, onLoadDocument])

  useEffect(() => {
    let active = true
    void Promise.all([onLoadAiMessages(card.id), onLoadConcepts(card.id)])
      .then(([loadedMessages, loadedConcepts]) => {
        if (!active) return
        setAiMessages(loadedMessages)
        setConcepts(loadedConcepts)
      })
      .catch((nextError) => {
        if (active) setError(DesktopError.fromUnknown(nextError))
      })
    return () => {
      active = false
    }
  }, [card.id, onLoadAiMessages, onLoadConcepts])

  const patch = useMemo<CardPatch>(() => {
    const nextPatch: CardPatch = {}
    if (title !== card.title) nextPatch.title = title
    if (note !== card.note) nextPatch.note = note
    if (sourceType !== card.sourceType) nextPatch.sourceType = sourceType
    if (sourceName !== card.sourceName) nextPatch.sourceName = sourceName
    if (sourceUri !== card.sourceUri) nextPatch.sourceUri = sourceUri
    return nextPatch
  }, [card, note, sourceName, sourceType, sourceUri, title])
  const hasChanges = Object.keys(patch).length > 0 || documentChanged
  const changeSignature = useMemo(
    () => JSON.stringify({
      patch,
      document: documentChanged ? serializedDocument : null,
    }),
    [documentChanged, patch, serializedDocument],
  )
  const latestChanges = useRef<SaveSnapshot>({
    patch,
    document,
    serializedDocument,
    documentChanged,
    hasChanges,
    revision: editRevision.current,
    signature: changeSignature,
  })
  latestChanges.current = {
    patch,
    document,
    serializedDocument,
    documentChanged,
    hasChanges,
    revision: editRevision.current,
    signature: changeSignature,
  }

  function markEdited() {
    editRevision.current += 1
    failedSignature.current = null
  }

  useEffect(() => {
    if (!aiDrawerOpen) return
    function closeOnEscape(event: globalThis.KeyboardEvent) {
      if (event.key === 'Escape') setAiDrawerOpen(false)
    }
    window.addEventListener('keydown', closeOnEscape)
    return () => window.removeEventListener('keydown', closeOnEscape)
  }, [aiDrawerOpen])

  // Let the app shell react to the drawer: it collapses the left rail and
  // restores it on close, so the readable column keeps as much width as
  // possible while the drawer is open.
  useEffect(() => {
    useDesktopStore.getState().setAiDrawerOpen(aiDrawerOpen)
    return () => useDesktopStore.getState().setAiDrawerOpen(false)
  }, [aiDrawerOpen])

  async function loadAiSettings() {
    if (aiSettings) return aiSettings
    if (!aiSettingsRequest.current) {
      aiSettingsRequest.current = onLoadAiSettings()
    }
    try {
      const loaded = await aiSettingsRequest.current
      setAiSettings(loaded)
      return loaded
    } finally {
      aiSettingsRequest.current = null
    }
  }

  const persistSnapshot = useCallback(async (
    snapshot: SaveSnapshot,
    showStatus: boolean,
  ) => {
    if (isArchived || !snapshot.hasChanges) return
    if (saveInFlight.current) {
      queuedUnmountSave.current = snapshot
      return
    }

    saveInFlight.current = true
    if (mounted.current && showStatus) {
      setError(null)
      setIsSaving(true)
      setAutoSaveState('saving')
    }

    try {
      if (snapshot.documentChanged) {
        const saved = await saveHandlers.current.onSaveDocument(card.id, snapshot.document)
        if (mounted.current) {
          setDocument((current) => (
            JSON.stringify(current) === snapshot.serializedDocument
              ? saved.document
              : current
          ))
          setSavedDocument(JSON.stringify(saved.document))
        }
      }
      await saveHandlers.current.onSave(snapshot.patch)
      lastSavedSignature.current = snapshot.signature
      if (mounted.current) {
        setLastSavedAt(new Date())
        setAutoSaveState(
          editRevision.current === snapshot.revision ? 'saved' : 'pending',
        )
      }
    } catch (nextError) {
      failedSignature.current = snapshot.signature
      if (mounted.current) {
        setError(DesktopError.fromUnknown(nextError))
        setAutoSaveState('error')
      }
    } finally {
      saveInFlight.current = false
      if (mounted.current && showStatus) setIsSaving(false)

      const queued = queuedUnmountSave.current
      queuedUnmountSave.current = null
      if (queued && queued.signature !== snapshot.signature) {
        void persistSnapshot(queued, false)
      }
    }
  }, [card.id, isArchived])

  useEffect(() => {
    if (
      isArchived
      || isDocumentLoading
      || isLifecycleRunning
      || !hasChanges
      || saveInFlight.current
      || failedSignature.current === changeSignature
      || lastSavedSignature.current === changeSignature
    ) {
      return
    }

    setAutoSaveState('pending')
    const timeout = window.setTimeout(() => {
      void persistSnapshot(latestChanges.current, true)
    }, AUTO_SAVE_DELAY_MS)
    return () => window.clearTimeout(timeout)
  }, [
    changeSignature,
    hasChanges,
    isArchived,
    isDocumentLoading,
    isLifecycleRunning,
    isSaving,
    persistSnapshot,
  ])

  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
      const snapshot = latestChanges.current
      if (
        !isArchived
        && snapshot.hasChanges
        && snapshot.signature !== lastSavedSignature.current
      ) {
        if (saveInFlight.current) {
          queuedUnmountSave.current = snapshot
        } else {
          void persistSnapshot(snapshot, false)
        }
      }
    }
  }, [isArchived, persistSnapshot])

  async function explainSelection(selection: EditorSelectionSnapshot) {
    const leadingWhitespace = selection.text.match(/^\s*/u)?.[0] ?? ''
    const selectedText = selection.text.trim()
    if (!selectedText || Array.from(selectedText).length > 200) {
      setError(new DesktopError(
        'concept_selection_invalid',
        '请选择 1 至 200 个字符',
        '重新选择需要解释的概念',
      ))
      return
    }
    let settings: AiSettings
    try {
      settings = await loadAiSettings()
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
      return
    }
    if (!settings.hasApiKey) {
      onOpenAiSettings()
      return
    }
    const preservesCompleteConcept = selectedText === selection.text
    const trimmedSelection = {
      ...selection,
      from: selection.from + leadingWhitespace.length,
      to: selection.from + leadingWhitespace.length + selectedText.length,
      text: selectedText,
      conceptId: preservesCompleteConcept ? selection.conceptId : undefined,
      mentionId: preservesCompleteConcept ? selection.mentionId : undefined,
    }
    setError(null)
    setConceptSelection(trimmedSelection)
    const requestId = crypto.randomUUID()
    conceptRequestId.current = requestId
    setConceptGenerationStatus('waiting')
    setConceptExplanation({
      selectedText,
      explanation: '',
      citations: [],
    })
    try {
      const result = await onExplainConcept(
        {
          cardId: card.id,
          selectedText,
          webSearchEnabled: settings.webSearchEnabled,
          requestId,
        },
        (delta) => {
          if (conceptRequestId.current !== requestId || !delta) return
          setConceptGenerationStatus('streaming')
          setConceptExplanation((current) => (
            current?.selectedText === selectedText
              ? { ...current, explanation: current.explanation + delta }
              : current
          ))
        },
      )
      if (conceptRequestId.current === requestId) {
        setConceptExplanation(result)
        setConceptGenerationStatus('complete')
      }
    } catch (nextError) {
      if (conceptRequestId.current === requestId) {
        setConceptExplanation(null)
        setConceptSelection(null)
        setError(DesktopError.fromUnknown(nextError))
      }
    } finally {
      if (conceptRequestId.current === requestId) conceptRequestId.current = null
    }
  }

  function closeConceptExplanation() {
    const requestId = conceptRequestId.current
    conceptRequestId.current = null
    if (requestId) void onCancelAiRequest(requestId)
    setConceptExplanation(null)
    setConceptSelection(null)
    setConceptGenerationStatus('complete')
  }

  async function saveConcept(value: { name: string; explanation: string }) {
    if (!conceptSelection) {
      throw new DesktopError(
        'concept_selection_stale',
        '原文选区已变化',
        '请重新选择文字后再试',
      )
    }
    if (overwriteConcept) {
      const saved = await onSaveConceptAnnotation({
        cardId: card.id,
        conceptId: overwriteConcept.id,
        mentionId: overwriteConcept.mentionId,
        selectedText: conceptSelection.text,
        name: overwriteConcept.name,
        explanation: value.explanation,
        overwriteExisting: true,
        document,
      })
      setDocument(saved.document)
      setSavedDocument(JSON.stringify(saved.document))
      setConcepts((current) => current.map((concept) => (
        concept.id === saved.concept.id
          ? {
              ...concept,
              name: saved.concept.name,
              explanation: saved.concept.explanation,
            }
          : concept
      )))
      setConceptExplanation(null)
      setConceptSelection(null)
      return
    }
    const conceptId = crypto.randomUUID()
    const mentionId = crypto.randomUUID()
    return new Promise<void>((resolve, reject) => {
      annotationCompletion.current = {
        input: {
          cardId: card.id,
          conceptId,
          mentionId,
          selectedText: conceptSelection.text,
          name: value.name,
          explanation: value.explanation,
          overwriteExisting: false,
        },
        resolve,
        reject,
      }
      setAnnotationRequest({
        conceptId,
        mentionId,
        from: conceptSelection.from,
        to: conceptSelection.to,
        revision: conceptSelection.revision,
        nonce: crypto.randomUUID(),
      })
    })
  }

  function annotationFailed() {
    const completion = annotationCompletion.current
    if (!completion) return
    annotationCompletion.current = null
    setAnnotationRequest(null)
    completion.reject(new DesktopError(
      'concept_selection_stale',
      '原文选区已变化',
      '请重新选择文字后再试',
    ))
  }

  function annotationApplied(markedDocument: RichDocument) {
    const completion = annotationCompletion.current
    if (!completion) return
    void onSaveConceptAnnotation({
      ...completion.input,
      document: markedDocument,
    }).then(async (saved) => {
      setDocument(saved.document)
      setSavedDocument(JSON.stringify(saved.document))
      setConcepts((current) => [
        ...current.filter((concept) => concept.mentionId !== saved.mention.mentionId),
        saved.mention,
      ])
      await onSave({})
      annotationCompletion.current = null
      setAnnotationRequest(null)
      setConceptExplanation(null)
      setConceptSelection(null)
      completion.resolve()
    }).catch(async (nextError) => {
      await onLoadDocument(card.id).then((loaded) => {
        setDocument(loaded.document)
        setSavedDocument(JSON.stringify(loaded.document))
      }).catch(() => undefined)
      annotationCompletion.current = null
      setAnnotationRequest(null)
      completion.reject(nextError)
    })
  }

  async function generateTitle() {
    if (titleRequestPending.current || isArchived || isDocumentLoading) return
    const content = documentPlainText(document)
    if (!content.trim()) return
    const revision = editRevision.current
    titleRequestPending.current = true
    setIsGeneratingTitle(true)
    setError(null)
    try {
      const generated = await onGenerateTitle(content)
      if (!mounted.current || revision !== editRevision.current) return
      markEdited()
      setTitle(generated)
    } catch (nextError) {
      if (mounted.current) setError(DesktopError.fromUnknown(nextError))
    } finally {
      titleRequestPending.current = false
      if (mounted.current) setIsGeneratingTitle(false)
    }
  }

  async function runLifecycle(action: () => Promise<unknown>) {
    if (isSaving || isLifecycleRunning) return
    setError(null)
    setIsLifecycleRunning(true)
    try {
      await action()
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      setIsLifecycleRunning(false)
    }
  }

  const autoSaveLabel = autoSaveState === 'saving'
    ? '保存中…'
    : autoSaveState === 'pending'
      ? '等待保存'
      : autoSaveState === 'error'
        ? '自动保存失败'
        : `已保存 ${formatSaveTime(lastSavedAt)}`
  const isBusy = isSaving || isLifecycleRunning

  return (
    <div
      aria-label="卡片详情"
      className={aiDrawerOpen ? 'card-detail has-ai-drawer' : 'card-detail'}
      role="form"
    >
      <header className="card-detail-header">
        <div>
          <div className="card-detail-heading">
            <p>卡片详情</p>
            {!isArchived ? (
              <span className={`status-chip is-${organizationStatus.value}`}>
                {organizationStatus.label}
              </span>
            ) : null}
          </div>
          <h2>{card.title}</h2>
        </div>
        <div className="card-detail-header-side">
          <div className="card-detail-actions is-header">
            {isArchived ? (
              <>
                <button
                  className="button is-danger"
                  disabled={isBusy}
                  type="button"
                  onClick={onDeleteRequest}
                >
                  <Trash2 aria-hidden="true" size={15} />
                  永久删除
                </button>
                <button
                  className="button is-secondary"
                  disabled={isBusy}
                  type="button"
                  onClick={() => void runLifecycle(onRestore)}
                >
                  <RotateCcw aria-hidden="true" size={15} />
                  {isLifecycleRunning ? '恢复中...' : '恢复'}
                </button>
              </>
            ) : (
              <>
                <button
                  className="button is-danger"
                  disabled={isBusy}
                  type="button"
                  onClick={onDeleteRequest}
                >
                  <Trash2 aria-hidden="true" size={15} />
                  删除
                </button>
                <button
                  className="button is-secondary"
                  disabled={hasChanges || isBusy}
                  type="button"
                  onClick={() => void runLifecycle(onArchive)}
                >
                  <Archive aria-hidden="true" size={15} />
                  {isLifecycleRunning ? '归档中...' : '归档'}
                </button>
              </>
            )}
            <button
              aria-expanded={aiDrawerOpen}
              className="button is-secondary ai-drawer-toggle"
              type="button"
              onClick={() => setAiDrawerOpen((open) => !open)}
            >
              <Sparkles aria-hidden="true" size={15} />
              AI 延伸
            </button>
          </div>
          {!isArchived ? (
            <span
              aria-live="polite"
              className={`card-detail-save-status is-${autoSaveState}`}
            >
              {autoSaveLabel}
            </span>
          ) : null}
        </div>
      </header>

      {error ? (
        <div className="inline-alert is-error" role="alert">
          <AlertCircle aria-hidden="true" size={16} />
          <div>
            <strong>{error.message}</strong>
            {error.recovery ? <span>{error.recovery}</span> : null}
          </div>
        </div>
      ) : null}

      <section className="detail-section detail-grid">
        <div className="form-field detail-title-field">
          <div className="detail-title-label">
            <span>标题</span>
            {!isArchived ? <button className="button is-ghost" type="button"
              disabled={isGeneratingTitle || isDocumentLoading || isLifecycleRunning || !documentPlainText(document).trim()}
              onClick={() => void generateTitle()}>
              <Sparkles aria-hidden="true" size={14} />
              {isGeneratingTitle ? '生成中…' : 'AI 生成'}
            </button> : null}
          </div>
          <label>
            <span className="sr-only">标题</span>
          <NativeTextField
            fieldId={`card-title-${card.id}`}
            readOnly={isArchived}
            value={title}
            onChange={(value) => {
              markEdited()
              setTitle(value)
            }}
          />
          </label>
        </div>
      </section>

      <section className="detail-section">
        <div className="form-field">
          <span>原始内容</span>
          <RichTextEditor
            annotationRequest={annotationRequest}
            concepts={concepts}
            label="原始内容"
            loadImage={onLoadInlineImage}
            readOnly={isArchived || isDocumentLoading}
            targetMentionId={targetMentionId}
            value={document}
            onAnnotationApplied={annotationApplied}
            onAnnotationError={annotationFailed}
            onChange={(value) => {
              markEdited()
              setDocument(value.document)
            }}
            onExplainSelection={isArchived ? undefined : (selection) => void explainSelection(selection)}
            onOpenConcept={onOpenConcept}
          />
        </div>
      </section>

      <section className="detail-section">
        <label className="form-field">
          <span>我的想法</span>
          <NativeTextArea
            fieldId={`card-note-${card.id}`}
            readOnly={isArchived}
            rows={6}
            value={note}
            onChange={(value) => {
              markEdited()
              setNote(value)
            }}
          />
        </label>
      </section>

      <details className="detail-metadata">
        <summary>来源与附件</summary>
        <div className="detail-metadata-content">
          <div className="detail-grid">
            <label className="form-field">
              <span>来源类型</span>
              <select
                disabled={isArchived}
                value={visibleSourceType(sourceType)}
                onChange={(event) => {
                  markEdited()
                  setSourceType(event.target.value as Card['sourceType'])
                }}
              >
                {sourceTypeOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="form-field">
              <span>来源名称</span>
              <NativeTextField
                fieldId={`card-source-name-${card.id}`}
                readOnly={isArchived}
                value={sourceName}
                onChange={(value) => {
                  markEdited()
                  setSourceName(value)
                }}
              />
            </label>
            <label className="form-field detail-span-full">
              <span>来源地址</span>
              <NativeTextField
                fieldId={`card-source-uri-${card.id}`}
                readOnly={isArchived}
                type="url"
                value={sourceUri}
                onChange={(value) => {
                  markEdited()
                  setSourceUri(value)
                }}
              />
            </label>
          </div>

          {card.attachments.length > 0 ? (
            <div className="metadata-attachments">
              <h3>附件</h3>
              <ul className="attachment-list">
                {card.attachments.map((attachment) => (
                  <li key={attachment.id}>
                    <button
                      className="attachment-open"
                      title="使用默认应用打开"
                      type="button"
                      onClick={() => {
                        setError(null)
                        void onRevealAttachment(attachment.id).catch((nextError) => {
                          setError(DesktopError.fromUnknown(nextError))
                        })
                      }}
                    >
                      <ExternalLink aria-hidden="true" size={13} />
                      <span>{attachment.originalName}</span>
                    </button>
                    <small>{attachment.mimeType}</small>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </div>
      </details>

      {conceptExplanation ? (
        <ConceptExplainDialog
          onOpenLink={(url) => void onOpenExternalUrl(url)}
          explanation={conceptExplanation.explanation}
          generationStatus={conceptGenerationStatus}
          overwriteConceptName={overwriteConcept?.name}
          selectedText={conceptExplanation.selectedText}
          onCancel={closeConceptExplanation}
          onRegenerate={() => {
            if (conceptSelection) void explainSelection(conceptSelection)
          }}
          onSave={saveConcept}
        />
      ) : null}

      {aiDrawerOpen ? (
        <aside aria-label="AI 延伸" className="ai-drawer" role="dialog">
          <header className="ai-drawer-header">
            <h3>
              <Sparkles aria-hidden="true" size={15} />
              AI 延伸
            </h3>
            <button
              aria-label="关闭 AI 延伸"
              className="icon-button on-chrome"
              type="button"
              onClick={() => setAiDrawerOpen(false)}
            >
              <X aria-hidden="true" size={16} />
            </button>
          </header>
          <AiConversation
            autoFocus={!isArchived}
            configured={isArchived || aiSettings === null || aiSettings.hasApiKey}
            messages={aiMessages}
            readOnly={isArchived}
            webSearchAvailable={Boolean(
              aiSettings?.webSearchEnabled
              && settingsSupportWebSearch(aiSettings.provider, aiSettings.model),
            )}
            onActivate={() => {
              if (!isArchived) {
                void loadAiSettings().catch((nextError) => {
                  setError(DesktopError.fromUnknown(nextError))
                })
              }
            }}
            onCancel={() => {
              const requestId = conversationRequestId.current
              if (requestId) void onCancelAiRequest(requestId)
            }}
            onOpenCitation={(url) => void onOpenExternalUrl(url)}
            onOpenSettings={onOpenAiSettings}
            onCopy={onCopyText}
            onRegenerate={async (messageId, onDelta) => {
              const settings = await loadAiSettings()
              if (!settings.hasApiKey) {
                onOpenAiSettings()
                throw new DesktopError(
                  'ai_key_missing',
                  '尚未保存 API Key',
                  '先在模型设置中保存 API Key',
                )
              }
              const requestId = crypto.randomUUID()
              conversationRequestId.current = requestId
              try {
                const messages = await onRegenerateAiMessage({
                  cardId: card.id,
                  messageId,
                  requestId,
                }, onDelta)
                setAiMessages(messages)
                return messages
              } finally {
                if (conversationRequestId.current === requestId) {
                  conversationRequestId.current = null
                }
              }
            }}
            onSend={async (input, onDelta) => {
              const settings = await loadAiSettings()
              if (!settings.hasApiKey) {
                onOpenAiSettings()
                throw new DesktopError(
                  'ai_key_missing',
                  '尚未保存 API Key',
                  '先在模型设置中保存 API Key',
                )
              }
              const requestId = crypto.randomUUID()
              conversationRequestId.current = requestId
              try {
                const messages = await onSendAiMessage({
                  cardId: card.id,
                  content: input.content,
                  webSearchEnabled: input.webSearchEnabled,
                  requestId,
                }, onDelta)
                setAiMessages(messages)
                return messages
              } finally {
                if (conversationRequestId.current === requestId) {
                  conversationRequestId.current = null
                }
              }
            }}
          />
        </aside>
      ) : null}
    </div>
  )
}
