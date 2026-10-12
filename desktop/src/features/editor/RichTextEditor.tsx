import { EditorContent, useEditor } from '@tiptap/react'
import { StarterKit } from '@tiptap/starter-kit'
import { AlertCircle } from 'lucide-react'
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type MouseEvent,
} from 'react'

import {
  clipboardImageNode,
  documentPlainText,
  DocumentContentError,
  type RichDocument,
} from '../../entities/document/model'
import type { CardConcept } from '../../entities/concept/model'
import { registerTipTapVoiceInput } from '../voice-input/tiptapVoiceInput'
import { ConceptMark } from './extensions/ConceptMark'
import {
  createInlineImageExtension,
  type InlineImageLoader,
} from './extensions/InlineImage'

interface RichTextEditorChange {
  document: RichDocument
  plainText: string
}

export interface EditorSelectionSnapshot {
  from: number
  to: number
  text: string
  revision: number
  conceptId?: string
  mentionId?: string
}

export interface ConceptAnnotationRequest {
  conceptId: string
  mentionId: string
  from: number
  to: number
  revision: number
  nonce: string
}

interface RichTextEditorProps {
  value: RichDocument
  label: string
  autoFocus?: boolean
  readOnly?: boolean
  createId?: () => string
  loadImage?: InlineImageLoader
  concepts?: CardConcept[]
  annotationRequest?: ConceptAnnotationRequest | null
  targetMentionId?: string | null
  onChange(value: RichTextEditorChange): void
  onAnnotationApplied?(document: RichDocument): void
  onAnnotationError?(): void
  onExplainSelection?(selection: EditorSelectionSnapshot): void
  onOpenConcept?(conceptId: string): void
}

const textOnlyStarterKit = StarterKit.configure({
  blockquote: false,
  bold: false,
  bulletList: false,
  code: false,
  codeBlock: false,
  heading: false,
  horizontalRule: false,
  italic: false,
  link: false,
  listItem: false,
  listKeymap: false,
  orderedList: false,
  strike: false,
  trailingNode: false,
  underline: false,
})

export function RichTextEditor({
  value,
  label,
  autoFocus = false,
  readOnly = false,
  createId,
  loadImage,
  concepts = [],
  annotationRequest,
  targetMentionId,
  onChange,
  onAnnotationApplied,
  onAnnotationError,
  onExplainSelection,
  onOpenConcept,
}: RichTextEditorProps) {
  const [error, setError] = useState<string | null>(null)
  const wrapperRef = useRef<HTMLDivElement>(null)
  const emittedDocument = useRef<string | null>(null)
  const revision = useRef(0)
  const selectedText = useRef<EditorSelectionSnapshot | null>(null)
  const appliedAnnotation = useRef<string | null>(null)
  const extensions = useMemo(
    () => [textOnlyStarterKit, ConceptMark, createInlineImageExtension(loadImage)],
    [loadImage],
  )
  const editor = useEditor({
    content: value,
    editable: !readOnly,
    immediatelyRender: false,
    extensions,
    editorProps: {
      attributes: {
        'aria-label': label,
        class: 'rich-editor-content',
        role: 'textbox',
      },
    },
    onUpdate({ editor: currentEditor }) {
      revision.current += 1
      const document = currentEditor.getJSON() as RichDocument
      emittedDocument.current = JSON.stringify(document)
      onChange({
        document,
        plainText: documentPlainText(document),
      })
    },
  }, [extensions, label, readOnly])

  useEffect(() => {
    if (!editor || editor.isDestroyed) return
    const nextDocument = JSON.stringify(value)
    if (nextDocument === emittedDocument.current || nextDocument === JSON.stringify(editor.getJSON())) {
      return
    }
    editor.commands.setContent(value, {
      emitUpdate: false,
      errorOnInvalidContent: true,
    })
  }, [editor, value])

  useEffect(() => {
    if (autoFocus && editor && !editor.isDestroyed && !readOnly) {
      editor.commands.focus('start')
    }
  }, [autoFocus, editor, readOnly])

  useEffect(() => {
    if (!editor || editor.isDestroyed || readOnly) return
    return registerTipTapVoiceInput(editor)
  }, [editor, readOnly])

  useEffect(() => {
    if (
      !annotationRequest
      || !editor
      || editor.isDestroyed
      || readOnly
      || appliedAnnotation.current === annotationRequest.nonce
    ) {
      return
    }
    appliedAnnotation.current = annotationRequest.nonce
    if (annotationRequest.revision !== revision.current) {
      onAnnotationError?.()
      return
    }
    const applied = editor
      .chain()
      .focus()
      .setTextSelection({ from: annotationRequest.from, to: annotationRequest.to })
      .setMark('concept', {
        conceptId: annotationRequest.conceptId,
        mentionId: annotationRequest.mentionId,
      })
      .run()
    if (!applied) {
      onAnnotationError?.()
      return
    }
    onAnnotationApplied?.(editor.getJSON() as RichDocument)
  }, [annotationRequest, editor, onAnnotationApplied, onAnnotationError, readOnly])

  useEffect(() => {
    if (!targetMentionId || !wrapperRef.current) return
    const target = wrapperRef.current.querySelector<HTMLElement>(
      `[data-mention-id="${targetMentionId}"]`,
    )
    target?.scrollIntoView({ block: 'center' })
    target?.classList.add('is-targeted')
  }, [targetMentionId, value])

  useEffect(() => {
    const wrapper = wrapperRef.current
    if (!wrapper || !editor || editor.isDestroyed) return
    const currentWrapper = wrapper
    const conceptsById = new Map(concepts.map((concept) => [concept.id, concept]))
    function syncConceptMetadata() {
      for (const mark of currentWrapper.querySelectorAll<HTMLElement>('[data-concept-id]')) {
        const concept = conceptsById.get(mark.dataset.conceptId ?? '')
        if (concept) {
          mark.dataset.conceptName = concept.name
          mark.dataset.conceptExplanation = concept.explanation
        } else {
          delete mark.dataset.conceptName
          delete mark.dataset.conceptExplanation
        }
      }
    }
    const observer = new MutationObserver(syncConceptMetadata)
    observer.observe(wrapper, { childList: true, subtree: true })
    syncConceptMetadata()
    return () => observer.disconnect()
  }, [concepts, editor, value])

  useEffect(() => {
    const wrapper = wrapperRef.current
    if (!wrapper || !editor || editor.isDestroyed || !onExplainSelection) return
    const currentEditor = editor

    function handleExplain(event: Event) {
      const mark = event.target instanceof Element
        ? event.target.closest<HTMLElement>('[data-concept-id]')
        : null
      const markText = mark?.textContent ?? ''
      if (mark && markText) {
        try {
          const from = currentEditor.view.posAtDOM(mark, 0)
          const to = currentEditor.view.posAtDOM(mark, mark.childNodes.length)
          onExplainSelection?.({
            from: Math.min(from, to),
            to: Math.max(from, to),
            text: markText,
            revision: revision.current,
            conceptId: mark.dataset.conceptId,
            mentionId: mark.dataset.mentionId,
          })
        } catch {
          selectedText.current = null
        }
        return
      }
      if (selectedText.current) {
        onExplainSelection?.(selectedText.current)
      }
    }
    wrapper.addEventListener('ideacard:explain-concept', handleExplain)
    return () => wrapper.removeEventListener('ideacard:explain-concept', handleExplain)
  }, [editor, onExplainSelection])

  function captureSelection(event: MouseEvent<HTMLDivElement>) {
    if (!editor || readOnly || !onExplainSelection) return
    const selection = window.getSelection()
    if (
      !selection
      || selection.isCollapsed
      || !selection.anchorNode
      || !selection.focusNode
      || !event.currentTarget.contains(selection.anchorNode)
      || !event.currentTarget.contains(selection.focusNode)
    ) {
      selectedText.current = null
      return
    }

    const anchor = editor.view.posAtDOM(selection.anchorNode, selection.anchorOffset)
    const focus = editor.view.posAtDOM(selection.focusNode, selection.focusOffset)
    const anchorElement = selection.anchorNode instanceof Element
      ? selection.anchorNode
      : selection.anchorNode.parentElement
    const focusElement = selection.focusNode instanceof Element
      ? selection.focusNode
      : selection.focusNode.parentElement
    const anchorMark = anchorElement?.closest<HTMLElement>('[data-concept-id]') ?? null
    const focusMark = focusElement?.closest<HTMLElement>('[data-concept-id]') ?? null
    const completeConceptMark = anchorMark
      && anchorMark === focusMark
      && selection.toString() === anchorMark.textContent
      ? anchorMark
      : null
    selectedText.current = {
      from: Math.min(anchor, focus),
      to: Math.max(anchor, focus),
      text: selection.toString(),
      revision: revision.current,
      conceptId: completeConceptMark?.dataset.conceptId,
      mentionId: completeConceptMark?.dataset.mentionId,
    }
  }

  function conceptFromEventTarget(target: EventTarget | null) {
    if (!(target instanceof Element)) return null
    const mark = target.closest<HTMLElement>('[data-concept-id]')
    if (!mark) return null
    return concepts.find((concept) => concept.id === mark.dataset.conceptId) ?? null
  }

  function handlePasteCapture(event: ClipboardEvent<HTMLDivElement>) {
    const files = Array.from(event.clipboardData.files)
      .filter((file) => file.type.startsWith('image/'))
    if (files.length === 0) return

    event.preventDefault()
    event.stopPropagation()
    if (readOnly || !editor) return

    setError(null)
    const insertionPosition = editor.state.selection.from
    void Promise.all(files.map((file) => clipboardImageNode(file, createId)))
      .then((nodes) => {
        editor.chain().focus().insertContentAt(insertionPosition, nodes).run()
      })
      .catch((nextError: unknown) => {
        setError(nextError instanceof DocumentContentError
          ? nextError.message
          : '无法读取粘贴的图片')
      })
  }

  return (
    <div
      className={`rich-editor${readOnly ? ' is-read-only' : ''}`}
      data-ai-explain-enabled={!readOnly && onExplainSelection ? 'true' : undefined}
      ref={wrapperRef}
      onContextMenuCapture={captureSelection}
      onClick={(event) => {
        const concept = conceptFromEventTarget(event.target)
        if (concept) onOpenConcept?.(concept.id)
      }}
      onMouseUp={captureSelection}
      onPasteCapture={handlePasteCapture}
    >
      <EditorContent editor={editor} />
      {error ? (
        <div className="rich-editor-alert" role="alert">
          <AlertCircle aria-hidden="true" size={15} />
          <span>{error}</span>
        </div>
      ) : null}
    </div>
  )
}
