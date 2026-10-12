import { describe, expect, it, vi } from 'vitest'

import { captureTextSelection } from './textSelection'

describe('text selection', () => {
  it('cuts selected textarea text and dispatches an input event', () => {
    const textarea = document.createElement('textarea')
    textarea.value = '保留删除保留'
    textarea.setSelectionRange(2, 4)
    const onInput = vi.fn()
    textarea.addEventListener('input', onInput)

    const selection = captureTextSelection(textarea)

    expect(selection?.selectedText).toBe('删除')
    selection?.cut()
    expect(textarea.value).toBe('保留保留')
    expect(onInput).toHaveBeenCalledTimes(1)
  })

  it('pastes text at the captured input selection', () => {
    const input = document.createElement('input')
    input.value = '前后'
    input.setSelectionRange(1, 1)

    const selection = captureTextSelection(input)

    selection?.paste('中')
    expect(input.value).toBe('前中后')
    expect(input.selectionStart).toBe(2)
    expect(input.selectionEnd).toBe(2)
  })

  it('selects all text in the captured control', () => {
    const input = document.createElement('input')
    input.value = '全部文字'
    input.setSelectionRange(0, 0)

    const selection = captureTextSelection(input)

    expect(selection?.canSelectAll).toBe(true)
    selection?.selectAll()
    expect(input.selectionStart).toBe(0)
    expect(input.selectionEnd).toBe(4)
  })

  it('disables editing operations for readonly and disabled controls', () => {
    const readonly = document.createElement('textarea')
    readonly.value = '只读文字'
    readonly.readOnly = true
    readonly.setSelectionRange(0, 2)
    const disabled = document.createElement('input')
    disabled.value = '禁用文字'
    disabled.disabled = true
    disabled.setSelectionRange(0, 2)

    expect(captureTextSelection(readonly)).toMatchObject({
      canCut: false,
      canCopy: true,
      canPaste: false,
    })
    expect(captureTextSelection(disabled)).toMatchObject({
      canCut: false,
      canCopy: true,
      canPaste: false,
    })
  })

  it('captures a normal document selection as copy only', () => {
    const paragraph = document.createElement('p')
    paragraph.textContent = '普通页面文字'
    document.body.append(paragraph)
    const range = document.createRange()
    range.setStart(paragraph.firstChild!, 0)
    range.setEnd(paragraph.firstChild!, 2)
    window.getSelection()?.removeAllRanges()
    window.getSelection()?.addRange(range)

    const selection = captureTextSelection(paragraph)

    expect(selection).toMatchObject({
      kind: 'document',
      selectedText: '普通',
      canCut: false,
      canCopy: true,
      canPaste: false,
      canSelectAll: false,
    })
  })

  it('uses the saved concept text when it is right-clicked without a selection', () => {
    const mark = document.createElement('span')
    mark.dataset.conceptId = 'concept-1'
    mark.textContent = '费曼学习法'
    document.body.append(mark)
    window.getSelection()?.removeAllRanges()

    expect(captureTextSelection(mark)).toMatchObject({
      kind: 'document',
      selectedText: '费曼学习法',
      canCopy: true,
    })

    const collapsedRange = document.createRange()
    collapsedRange.setStart(mark.firstChild!, 2)
    collapsedRange.collapse(true)
    window.getSelection()?.addRange(collapsedRange)
    expect(captureTextSelection(mark)).toMatchObject({
      selectedText: '费曼学习法',
      canCopy: true,
    })
  })

  it('returns null for unsupported targets without a document selection', () => {
    window.getSelection()?.removeAllRanges()

    expect(captureTextSelection(document.createElement('div'))).toBeNull()
    expect(captureTextSelection(null)).toBeNull()
  })
})
