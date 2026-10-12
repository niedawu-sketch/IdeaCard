import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  forwardRef,
  useContext,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type ChangeEvent,
  type KeyboardEventHandler,
} from 'react'

import { DesktopServiceContext } from '../../services/desktop/useDesktopService'

interface ChangedPayload {
  fieldId: string
  value: string
}

interface FocusPayload {
  fieldId: string
  focused: boolean
}

interface SubmittedPayload {
  fieldId: string
}

interface NativeTextAreaInsets {
  top?: number
  right?: number
  bottom?: number
  left?: number
}

interface NativeTextAreaProps {
  fieldId: string
  value: string
  id?: string
  readOnly?: boolean
  placeholder?: string
  rows?: number
  autoFocus?: boolean
  className?: string
  submitOnEnter?: boolean
  nativeInsets?: NativeTextAreaInsets
  onChange(value: string): void
  onFocus?(): void
  onKeyDown?: KeyboardEventHandler<HTMLTextAreaElement>
  onSubmit?(): void
}

function isTauriRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function isElementVisible(element: HTMLTextAreaElement, rect: DOMRect) {
  if (
    !element.isConnected
    || rect.width <= 0
    || rect.height <= 0
    || rect.bottom <= 0
    || rect.right <= 0
    || rect.top >= window.innerHeight
    || rect.left >= window.innerWidth
  ) {
    return false
  }
  const x = Math.min(Math.max(rect.left + rect.width / 2, 0), window.innerWidth - 1)
  const y = Math.min(Math.max(rect.top + rect.height / 2, 0), window.innerHeight - 1)
  return document.elementFromPoint(x, y) === element
}

export const NativeTextArea = forwardRef<HTMLTextAreaElement, NativeTextAreaProps>(
function NativeTextArea({
  fieldId,
  value,
  id,
  readOnly = false,
  placeholder,
  rows,
  autoFocus = false,
  className = '',
  submitOnEnter = false,
  nativeInsets,
  onChange,
  onFocus,
  onKeyDown,
  onSubmit,
}, forwardedRef) {
  const service = useContext(DesktopServiceContext)
  const textAreaRef = useRef<HTMLTextAreaElement>(null)
  const valueRef = useRef(value)
  const readOnlyRef = useRef(readOnly)
  const submitOnEnterRef = useRef(submitOnEnter)
  const nativeInsetsRef = useRef(nativeInsets)
  const onChangeRef = useRef(onChange)
  const onFocusRef = useRef(onFocus)
  const onSubmitRef = useRef(onSubmit)
  const scheduleSyncRef = useRef<(() => void) | null>(null)
  const autoFocusPendingRef = useRef(autoFocus)
  const [nativeFailed, setNativeFailed] = useState(false)
  const [nativeFocused, setNativeFocused] = useState(false)
  const nativeAvailable = isTauriRuntime()
    && Boolean(service?.syncNativeTextArea)
    && Boolean(service?.removeNativeTextArea)
    && Boolean(service?.focusNativeTextArea)
  const nativeEnabled = nativeAvailable && !nativeFailed

  useImperativeHandle(forwardedRef, () => textAreaRef.current as HTMLTextAreaElement)

  useEffect(() => {
    if (!nativeAvailable) return

    let disposed = false
    let lastSyncSignature: string | null = null
    let frameRequest: number | null = null
    let resizeObserver: ResizeObserver | null = null
    let mutationObserver: MutationObserver | null = null
    const unlisteners: UnlistenFn[] = []

    function syncField() {
      frameRequest = null
      const element = textAreaRef.current
      if (!element || disposed) return
      const rect = element.getBoundingClientRect()
      const visible = isElementVisible(element, rect)
      const insets = nativeInsetsRef.current
      const top = Math.max(0, insets?.top ?? 0)
      const right = Math.max(0, insets?.right ?? 0)
      const bottom = Math.max(0, insets?.bottom ?? 0)
      const left = Math.max(0, insets?.left ?? 0)
      const config = {
        fieldId,
        value: valueRef.current,
        readOnly: readOnlyRef.current,
        submitOnEnter: submitOnEnterRef.current,
        visible,
        x: rect.x + left,
        y: rect.y + top,
        width: Math.max(0, rect.width - left - right),
        height: Math.max(0, rect.height - top - bottom),
      }
      const signature = JSON.stringify(config)
      if (signature === lastSyncSignature) return
      lastSyncSignature = signature
      void service?.syncNativeTextArea?.(config).then(() => {
        if (disposed) return
        setNativeFailed(false)
        if (autoFocusPendingRef.current && visible && !readOnlyRef.current) {
          autoFocusPendingRef.current = false
          void service?.focusNativeTextArea?.(fieldId).catch(() => {
            if (!disposed) setNativeFailed(true)
          })
        }
      }).catch(() => {
        if (lastSyncSignature === signature) lastSyncSignature = null
        if (!disposed) setNativeFailed(true)
      })
    }

    function scheduleSync() {
      if (frameRequest !== null) window.cancelAnimationFrame(frameRequest)
      frameRequest = window.requestAnimationFrame(syncField)
    }
    scheduleSyncRef.current = scheduleSync

    const listenerPromise = Promise.all([
      listen<ChangedPayload>('native-text-area://changed', (event) => {
        if (event.payload.fieldId !== fieldId) return
        if (event.payload.value === valueRef.current) return
        valueRef.current = event.payload.value
        onChangeRef.current(event.payload.value)
      }),
      listen<FocusPayload>('native-text-area://focus-changed', (event) => {
        if (event.payload.fieldId === fieldId) {
          setNativeFocused(event.payload.focused)
          if (event.payload.focused) onFocusRef.current?.()
        }
      }),
      listen<SubmittedPayload>('native-text-area://submitted', (event) => {
        if (event.payload.fieldId === fieldId) onSubmitRef.current?.()
      }),
    ]).then((registered) => {
      if (disposed) {
        for (const unlisten of registered) unlisten()
        return
      }
      unlisteners.push(...registered)
      const element = textAreaRef.current
      if (element) {
        resizeObserver = new ResizeObserver(scheduleSync)
        let observedElement: Element | null = element
        while (observedElement) {
          resizeObserver.observe(observedElement)
          observedElement = observedElement.parentElement
        }
      }
      mutationObserver = new MutationObserver(scheduleSync)
      mutationObserver.observe(document.body, {
        childList: true,
        subtree: true,
      })
      window.addEventListener('resize', scheduleSync)
      window.addEventListener('scroll', scheduleSync, true)
      scheduleSync()
    }).catch(() => {
      if (!disposed) setNativeFailed(true)
    })

    return () => {
      disposed = true
      scheduleSyncRef.current = null
      if (frameRequest !== null) window.cancelAnimationFrame(frameRequest)
      resizeObserver?.disconnect()
      mutationObserver?.disconnect()
      window.removeEventListener('resize', scheduleSync)
      window.removeEventListener('scroll', scheduleSync, true)
      for (const unlisten of unlisteners) unlisten()
      void listenerPromise
      void service?.removeNativeTextArea?.(fieldId).catch(() => undefined)
    }
  }, [fieldId, nativeAvailable, service])

  useEffect(() => {
    onChangeRef.current = onChange
    onFocusRef.current = onFocus
    onSubmitRef.current = onSubmit
  }, [onChange, onFocus, onSubmit])

  useEffect(() => {
    valueRef.current = value
    readOnlyRef.current = readOnly
    submitOnEnterRef.current = submitOnEnter
    nativeInsetsRef.current = nativeInsets
    scheduleSyncRef.current?.()
  }, [nativeInsets, readOnly, submitOnEnter, value])

  function handleHtmlChange(event: ChangeEvent<HTMLTextAreaElement>) {
    if (!nativeEnabled) onChange(event.target.value)
  }

  function focusNativeField() {
    if (nativeEnabled) {
      void service?.focusNativeTextArea?.(fieldId).catch(() => setNativeFailed(true))
    }
  }

  const classes = [
    className,
    'native-text-area-host',
    nativeEnabled ? 'is-native-enabled' : '',
    readOnly ? 'is-native-read-only' : '',
    nativeFocused ? 'is-native-focused' : '',
  ].filter(Boolean).join(' ')

  return (
    <textarea
      ref={textAreaRef}
      autoFocus={autoFocus}
      className={classes}
      id={id}
      placeholder={placeholder}
      readOnly={nativeEnabled || readOnly}
      rows={rows}
      value={value}
      onChange={handleHtmlChange}
      onFocus={nativeEnabled ? undefined : onFocus}
      onKeyDown={onKeyDown}
      onPointerDown={focusNativeField}
    />
  )
})
