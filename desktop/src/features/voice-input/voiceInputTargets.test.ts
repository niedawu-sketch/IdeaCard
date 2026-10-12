import { fireEvent } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { VoiceInputTargetRegistry } from './voiceInputTargets'

describe('VoiceInputTargetRegistry', () => {
  const registry = new VoiceInputTargetRegistry()

  afterEach(() => {
    registry.resetForTests()
    document.body.replaceChildren()
  })

  it('tracks the focused text control and applies complete streaming revisions', () => {
    const onTargetChange = vi.fn()
    const stopTracking = registry.trackDomTargets(onTargetChange)
    const textarea = document.createElement('textarea')
    textarea.value = '前缀旧内容后缀'
    document.body.append(textarea)
    textarea.focus()
    textarea.setSelectionRange(2, 5)
    fireEvent.focusIn(textarea)
    const targetToken = onTargetChange.mock.lastCall?.[0] as string

    expect(registry.beginSession(1, targetToken)).toBe(true)
    registry.applyDraft(1, '新的')
    expect(textarea.value).toBe('前缀新的后缀')
    registry.applyDraft(1, '新的内容')
    expect(textarea.value).toBe('前缀新的内容后缀')

    registry.commitSession(1)
    expect(textarea.selectionStart).toBe(6)
    expect(textarea.selectionEnd).toBe(6)
    stopTracking()
  })

  it('restores the original selection when a session is cancelled', () => {
    const onInput = vi.fn()
    const onTargetChange = vi.fn()
    const stopTracking = registry.trackDomTargets(onTargetChange)
    const input = document.createElement('input')
    input.value = '保留替换保留'
    input.addEventListener('input', onInput)
    document.body.append(input)
    input.focus()
    input.setSelectionRange(2, 4)
    fireEvent.focusIn(input)
    const targetToken = onTargetChange.mock.lastCall?.[0] as string

    registry.beginSession(2, targetToken)
    registry.applyDraft(2, '语音草稿')
    registry.rollbackSession(2)

    expect(input.value).toBe('保留替换保留')
    expect(input.selectionStart).toBe(4)
    expect(onInput).toHaveBeenCalledTimes(2)
    stopTracking()
  })

  it('uses the selection cached before the native client takes focus', () => {
    const onTargetChange = vi.fn()
    const stopTracking = registry.trackDomTargets(onTargetChange)
    const textarea = document.createElement('textarea')
    textarea.value = '前缀待替换后缀'
    document.body.append(textarea)
    textarea.focus()
    textarea.setSelectionRange(2, 5)
    fireEvent.focusIn(textarea)
    const targetToken = onTargetChange.mock.lastCall?.[0] as string
    textarea.setSelectionRange(0, 0)

    registry.beginSession(5, targetToken)
    registry.applyDraft(5, '语音')

    expect(textarea.value).toBe('前缀语音后缀')
    stopTracking()
  })

  it('normalizes line breaks for single-line inputs', () => {
    const onTargetChange = vi.fn()
    const stopTracking = registry.trackDomTargets(onTargetChange)
    const input = document.createElement('input')
    document.body.append(input)
    input.focus()
    fireEvent.focusIn(input)
    const targetToken = onTargetChange.mock.lastCall?.[0] as string

    registry.beginSession(3, targetToken)
    registry.applyDraft(3, '第一行\n第二行')

    expect(input.value).toBe('第一行 第二行')
    stopTracking()
  })

  it('does not register password or read-only controls', () => {
    const onTargetChange = vi.fn()
    const stopTracking = registry.trackDomTargets(onTargetChange)
    const password = document.createElement('input')
    password.type = 'password'
    document.body.append(password)

    fireEvent.focusIn(password)

    expect(onTargetChange.mock.lastCall?.[0]).toBeNull()
    stopTracking()
  })

  it('suspends the target while another input method is composing text', () => {
    const onTargetChange = vi.fn()
    const stopTracking = registry.trackDomTargets(onTargetChange)
    const textarea = document.createElement('textarea')
    document.body.append(textarea)
    fireEvent.focusIn(textarea)
    const targetToken = onTargetChange.mock.lastCall?.[0] as string

    fireEvent.compositionStart(textarea)
    expect(onTargetChange.mock.lastCall?.[0]).toBeNull()
    expect(registry.beginSession(4, targetToken)).toBe(false)
    fireEvent.compositionEnd(textarea)

    expect(onTargetChange.mock.lastCall?.[0]).toBe(targetToken)
    stopTracking()
  })
})
