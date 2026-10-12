import type { Editor } from '@tiptap/core'
import { TextSelection } from '@tiptap/pm/state'

import { voiceInputTargets, type VoiceInputTarget } from './voiceInputTargets'

function normalizeDraft(text: string) {
  return text.replace(/[\r\n]+/g, ' ')
}

function createTipTapVoiceTarget(editor: Editor): VoiceInputTarget {
  return {
    beginSession() {
      if (editor.isDestroyed || !editor.isEditable || editor.view.composing) return null

      const start = editor.state.selection.from
      const originalEnd = editor.state.selection.to
      const originalSlice = editor.state.doc.slice(start, originalEnd)
      let currentEnd = originalEnd
      let currentDraft = ''

      function selectEnd(position: number) {
        const safePosition = Math.min(position, editor.state.doc.content.size)
        return TextSelection.near(editor.state.doc.resolve(safePosition))
      }

      function applyDraft(text: string, addToHistory: boolean) {
        if (editor.isDestroyed) return
        const nextDraft = normalizeDraft(text)
        const transaction = editor.state.tr
          .insertText(nextDraft, start, currentEnd)
          .setMeta('addToHistory', addToHistory)
          .setMeta('voiceInput', true)
        currentEnd = start + nextDraft.length
        transaction.setSelection(
          TextSelection.near(transaction.doc.resolve(
            Math.min(currentEnd, transaction.doc.content.size),
          )),
        )
        editor.view.dispatch(transaction)
        currentDraft = nextDraft
      }

      function restoreOriginal() {
        if (editor.isDestroyed) return
        const transaction = editor.state.tr
          .replaceRange(start, currentEnd, originalSlice)
          .setMeta('addToHistory', false)
          .setMeta('voiceInput', true)
        transaction.setSelection(
          TextSelection.near(transaction.doc.resolve(
            Math.min(originalEnd, transaction.doc.content.size),
          )),
        )
        editor.view.dispatch(transaction)
        currentEnd = originalEnd
      }

      return {
        applyDraft(text) {
          applyDraft(text, false)
        },
        commit() {
          if (editor.isDestroyed) return
          const finalDraft = currentDraft
          restoreOriginal()
          if (finalDraft.length > 0) {
            applyDraft(finalDraft, true)
          }
          editor.commands.focus()
          editor.view.dispatch(
            editor.state.tr.setSelection(selectEnd(start + finalDraft.length)),
          )
        },
        rollback() {
          if (editor.isDestroyed) return
          restoreOriginal()
          editor.commands.focus()
        },
      }
    },
  }
}

export function registerTipTapVoiceInput(editor: Editor) {
  return voiceInputTargets.registerCustomTarget(
    editor.view.dom,
    createTipTapVoiceTarget(editor),
  ).unregister
}
