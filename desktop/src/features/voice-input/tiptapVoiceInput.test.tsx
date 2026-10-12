import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { documentFromPlainText } from '../../entities/document/model'
import { RichTextEditor } from '../editor/RichTextEditor'
import { voiceInputTargets } from './voiceInputTargets'

describe('TipTap voice input target', () => {
  afterEach(() => {
    voiceInputTargets.resetForTests()
  })

  it('replaces one provisional range as the streaming draft is revised', () => {
    const onTargetChange = vi.fn()
    const stopTracking = voiceInputTargets.trackDomTargets(onTargetChange)
    render(
      <RichTextEditor
        label="语音记录"
        value={documentFromPlainText('已有内容')}
        onChange={vi.fn()}
      />,
    )
    const editor = screen.getByRole('textbox', { name: '语音记录' })
    fireEvent.focus(editor)
    fireEvent.focusIn(editor)
    const targetToken = onTargetChange.mock.lastCall?.[0] as string

    expect(voiceInputTargets.beginSession(11, targetToken)).toBe(true)
    voiceInputTargets.applyDraft(11, '第一版')
    expect(editor).toHaveTextContent('第一版已有内容')
    voiceInputTargets.applyDraft(11, '修订后的版本')
    expect(editor).toHaveTextContent('修订后的版本已有内容')
    voiceInputTargets.commitSession(11)
    expect(editor).toHaveTextContent('修订后的版本已有内容')
    fireEvent.keyDown(editor, { key: 'z', metaKey: true })
    expect(editor).toHaveTextContent('已有内容')
    stopTracking()
  })

  it('restores selected rich text when voice input is cancelled', () => {
    const onTargetChange = vi.fn()
    const stopTracking = voiceInputTargets.trackDomTargets(onTargetChange)
    render(
      <RichTextEditor
        label="语音记录"
        value={documentFromPlainText('原始内容')}
        onChange={vi.fn()}
      />,
    )
    const editor = screen.getByRole('textbox', { name: '语音记录' })
    fireEvent.focus(editor)
    fireEvent.focusIn(editor)
    const targetToken = onTargetChange.mock.lastCall?.[0] as string

    voiceInputTargets.beginSession(12, targetToken)
    voiceInputTargets.applyDraft(12, '临时语音')
    voiceInputTargets.rollbackSession(12)

    expect(editor).toHaveTextContent('原始内容')
    stopTracking()
  })
})
