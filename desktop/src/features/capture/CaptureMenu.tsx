import { ClipboardPaste, FileUp, LoaderCircle, Plus } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

import type { FileImportResult } from '../../services/desktop/contracts'
import { FileImport } from './FileImport'

interface CaptureMenuProps {
  onClipboard(): Promise<void> | void
  onImport(): Promise<FileImportResult[]>
}

export function CaptureMenu({ onClipboard, onImport }: CaptureMenuProps) {
  const [isOpen, setIsOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!isOpen) return

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === 'Escape') setIsOpen(false)
    }

    function closeOnOutsideClick(event: MouseEvent) {
      if (!rootRef.current?.contains(event.target as Node)) setIsOpen(false)
    }

    document.addEventListener('keydown', closeOnEscape)
    document.addEventListener('mousedown', closeOnOutsideClick)
    return () => {
      document.removeEventListener('keydown', closeOnEscape)
      document.removeEventListener('mousedown', closeOnOutsideClick)
    }
  }, [isOpen])

  return (
    <div className="capture-menu-control" ref={rootRef}>
      <FileImport onImport={onImport}>
        {({ isImporting, startImport }) => (
          <>
            <button
              aria-expanded={isOpen}
              aria-haspopup="menu"
              aria-label="更多捕获方式"
              className="icon-button"
              disabled={isImporting}
              title="更多捕获方式"
              type="button"
              onClick={() => setIsOpen((current) => !current)}
            >
              {isImporting ? (
                <LoaderCircle aria-hidden="true" className="is-spinning" size={16} />
              ) : (
                <Plus aria-hidden="true" size={16} />
              )}
            </button>
            {isOpen ? (
              <div className="capture-menu-popover" role="menu">
                <button
                  className="capture-menu-item"
                  role="menuitem"
                  type="button"
                  onClick={() => {
                    setIsOpen(false)
                    void onClipboard()
                  }}
                >
                  <ClipboardPaste aria-hidden="true" size={15} />
                  从剪贴板捕获
                </button>
                <button
                  aria-label="导入本地文件"
                  className="capture-menu-item has-support-note"
                  role="menuitem"
                  type="button"
                  onClick={() => {
                    setIsOpen(false)
                    void startImport()
                  }}
                >
                  <FileUp aria-hidden="true" size={15} />
                  <span className="capture-menu-item-copy">
                    <span>导入本地文件</span>
                    <small>支持 PDF、DOCX、PPTX、XLSX、MD、TXT、图片</small>
                  </span>
                </button>
              </div>
            ) : null}
          </>
        )}
      </FileImport>
    </div>
  )
}
