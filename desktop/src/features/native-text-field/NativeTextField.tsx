import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useContext, useEffect, useRef, useState, type ChangeEvent } from 'react'

import { DesktopServiceContext } from '../../services/desktop/useDesktopService'

interface ChangedPayload {
  fieldId: string
  value: string
}

interface FocusPayload {
  fieldId: string
  focused: boolean
}

interface NativeTextFieldProps {
  fieldId: string
  value: string
  readOnly?: boolean
  placeholder?: string
  required?: boolean
  type?: 'text' | 'search' | 'url'
  className?: string
  onChange(value: string): void
}

function isTauriRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function isElementVisible(element: HTMLInputElement, rect: DOMRect) {
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

export function NativeTextField({
  fieldId,
  value,
  readOnly = false,
  placeholder = '',
  required = false,
  type = 'text',
  className = '',
  onChange,
}: NativeTextFieldProps) {
  const service = useContext(DesktopServiceContext)
  const inputRef = useRef<HTMLInputElement>(null)
  const valueRef = useRef(value)
  const readOnlyRef = useRef(readOnly)
  const placeholderRef = useRef(placeholder)
  const onChangeRef = useRef(onChange)
  const scheduleSyncRef = useRef<(() => void) | null>(null)
  const [nativeFailed, setNativeFailed] = useState(false)
  const [nativeFocused, setNativeFocused] = useState(false)
  const nativeAvailable = isTauriRuntime()
    && Boolean(service?.syncNativeTextField)
    && Boolean(service?.removeNativeTextField)
    && Boolean(service?.focusNativeTextField)
  const nativeEnabled = nativeAvailable && !nativeFailed

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
      const element = inputRef.current
      if (!element || disposed) return
      const rect = element.getBoundingClientRect()
      const config = {
        fieldId,
        value: valueRef.current,
        placeholder: placeholderRef.current,
        readOnly: readOnlyRef.current,
        visible: isElementVisible(element, rect),
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
      }
      const signature = JSON.stringify(config)
      if (signature === lastSyncSignature) return
      lastSyncSignature = signature
      void service?.syncNativeTextField?.(config).then(() => {
        if (!disposed) setNativeFailed(false)
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
      listen<ChangedPayload>('native-text-field://changed', (event) => {
        if (event.payload.fieldId !== fieldId) return
        if (event.payload.value === valueRef.current) return
        valueRef.current = event.payload.value
        onChangeRef.current(event.payload.value)
      }),
      listen<FocusPayload>('native-text-field://focus-changed', (event) => {
        if (event.payload.fieldId === fieldId) {
          setNativeFocused(event.payload.focused)
        }
      }),
    ]).then((registered) => {
      if (disposed) {
        for (const unlisten of registered) unlisten()
        return
      }
      unlisteners.push(...registered)
      const element = inputRef.current
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
      void service?.removeNativeTextField?.(fieldId).catch(() => undefined)
    }
  }, [fieldId, nativeAvailable, service])

  useEffect(() => {
    onChangeRef.current = onChange
  }, [onChange])

  useEffect(() => {
    valueRef.current = value
    readOnlyRef.current = readOnly
    placeholderRef.current = placeholder
    scheduleSyncRef.current?.()
  }, [placeholder, readOnly, value])

  function handleHtmlChange(event: ChangeEvent<HTMLInputElement>) {
    if (!nativeEnabled) onChange(event.target.value)
  }

  function focusNativeField() {
    if (nativeEnabled) {
      void service?.focusNativeTextField?.(fieldId).catch(() => setNativeFailed(true))
    }
  }

  const classes = [
    className,
    'native-text-field-host',
    nativeEnabled ? 'is-native-enabled' : '',
    readOnly ? 'is-native-read-only' : '',
    nativeFocused ? 'is-native-focused' : '',
  ].filter(Boolean).join(' ')

  return (
    <input
      ref={inputRef}
      className={classes}
      placeholder={placeholder}
      readOnly={nativeEnabled || readOnly}
      required={required}
      type={type}
      value={value}
      onChange={handleHtmlChange}
      onPointerDown={focusNativeField}
    />
  )
}
