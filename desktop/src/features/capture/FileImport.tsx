import { AlertCircle, Check, X } from 'lucide-react'
import { useEffect, useState, type ReactNode } from 'react'

import { DesktopError } from '../../shared/errors/DesktopError'
import type { FileImportResult } from '../../services/desktop/contracts'

interface FileImportProps {
  onImport(): Promise<FileImportResult[]>
  children(control: FileImportControl): ReactNode
}

interface FileImportControl {
  isImporting: boolean
  startImport(): Promise<void>
}

const errorLabels: Record<string, string> = {
  attachment_too_large: '文件超过大小限制',
  unsupported_file_type: '不支持这种文件类型',
  attachment_source_missing: '找不到文件',
}

function resultLabel(result: FileImportResult) {
  if (result.status === 'saved') return '导入成功'
  if (result.status === 'saved_with_extraction_failure') return '已保存，内容提取失败'
  return errorLabels[result.errorCode ?? ''] ?? '导入失败'
}

export function FileImport({ children, onImport }: FileImportProps) {
  const [results, setResults] = useState<FileImportResult[]>([])
  const [isImporting, setIsImporting] = useState(false)
  const [error, setError] = useState<DesktopError | null>(null)
  const hasPersistentResults = results.some((result) => result.status !== 'saved')

  useEffect(() => {
    if (!results.some((result) => result.status === 'saved')) return
    const timer = window.setTimeout(() => {
      setResults((current) => current.filter((result) => result.status !== 'saved'))
    }, 1000)
    return () => window.clearTimeout(timer)
  }, [results])

  async function startImport() {
    if (isImporting) return
    setError(null)
    setResults([])
    setIsImporting(true)
    try {
      setResults(await onImport())
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      setIsImporting(false)
    }
  }

  return (
    <div className="file-import-control">
      {children({ isImporting, startImport })}
      {error ? (
        <div className="inline-alert is-error file-import-error" role="alert">
          <AlertCircle aria-hidden="true" size={15} />
          <strong>{error.message}</strong>
          <button
            aria-label="关闭导入结果"
            className="icon-button subtle"
            title="关闭导入结果"
            type="button"
            onClick={() => setError(null)}
          >
            <X aria-hidden="true" size={14} />
          </button>
        </div>
      ) : null}
      {results.length > 0 ? (
        <div aria-label="文件导入结果" className="file-import-results" role="status">
          {hasPersistentResults ? (
            <div className="file-import-results-header">
              <span>导入结果</span>
              <button
                aria-label="关闭导入结果"
                className="icon-button subtle"
                title="关闭导入结果"
                type="button"
                onClick={() => setResults([])}
              >
                <X aria-hidden="true" size={14} />
              </button>
            </div>
          ) : null}
          {results.map((result, index) => (
            <div className={`file-import-result is-${result.status}`} key={`${result.originalName}-${index}`}>
              {result.status === 'saved' ? <Check aria-hidden="true" size={14} /> : <AlertCircle aria-hidden="true" size={14} />}
              <strong>{result.originalName}</strong>
              <span>{resultLabel(result)}</span>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  )
}
