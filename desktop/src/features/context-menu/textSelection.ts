type TextControl = HTMLInputElement | HTMLTextAreaElement

export interface TextSelectionSnapshot {
  kind: 'control' | 'document'
  selectedText: string
  canCut: boolean
  canCopy: boolean
  canPaste: boolean
  canSelectAll: boolean
  cut(): void
  paste(text: string): void
  selectAll(): void
}

const supportedInputTypes = new Set([
  'email',
  'password',
  'search',
  'tel',
  'text',
  'url',
])

function isTextControl(target: EventTarget | null): target is TextControl {
  return target instanceof HTMLTextAreaElement
    || (target instanceof HTMLInputElement && supportedInputTypes.has(target.type))
}

function dispatchInput(target: TextControl, inputType: string, data: string | null) {
  target.dispatchEvent(new InputEvent('input', {
    bubbles: true,
    data,
    inputType,
  }))
}

function captureControlSelection(target: TextControl): TextSelectionSnapshot {
  const start = target.selectionStart ?? 0
  const end = target.selectionEnd ?? start
  const selectedText = target.value.slice(start, end)
  const editable = !target.disabled && !target.readOnly

  return {
    kind: 'control',
    selectedText,
    canCut: editable && selectedText.length > 0,
    canCopy: selectedText.length > 0,
    canPaste: editable,
    canSelectAll: target.value.length > 0,
    cut() {
      if (!editable || start === end) return
      target.setRangeText('', start, end, 'end')
      dispatchInput(target, 'deleteByCut', null)
    },
    paste(text) {
      if (!editable) return
      target.setRangeText(text, start, end, 'end')
      dispatchInput(target, 'insertFromPaste', text)
    },
    selectAll() {
      if (target.value.length === 0) return
      target.focus()
      target.setSelectionRange(0, target.value.length)
    },
  }
}

function documentSelectionSnapshot(selectedText: string): TextSelectionSnapshot {
  return {
    kind: 'document',
    selectedText,
    canCut: false,
    canCopy: true,
    canPaste: false,
    canSelectAll: false,
    cut() {},
    paste() {},
    selectAll() {},
  }
}

function captureDocumentSelection(target: EventTarget | null): TextSelectionSnapshot | null {
  const selection = window.getSelection()
  const conceptMark = target instanceof Element
    ? target.closest<HTMLElement>('[data-concept-id]')
    : null
  const selectionInsideConcept = Boolean(
    conceptMark
    && selection?.anchorNode
    && selection.focusNode
    && conceptMark.contains(selection.anchorNode)
    && conceptMark.contains(selection.focusNode),
  )
  const documentSelection = selection?.toString() ?? ''
  const selectedText = conceptMark
    ? selectionInsideConcept && documentSelection
      ? documentSelection
      : conceptMark.textContent ?? ''
    : documentSelection
  if (!selectedText) return null
  return documentSelectionSnapshot(selectedText)
}

export function captureTextSelection(
  target: EventTarget | null,
): TextSelectionSnapshot | null {
  if (isTextControl(target)) {
    return captureControlSelection(target)
  }
  return captureDocumentSelection(target)
}
