import { Clipboard, Lightbulb, Search, ScanText } from 'lucide-react'
import type { CaptureKind } from '../capture/captureAdapter'
import { Button } from '../../shared/ui/Button'

interface InboxToolbarProps {
  query: string
  resultCount: number
  onQueryChange: (query: string) => void
  onOpenCapture: (kind: CaptureKind) => void
}

export function InboxToolbar({
  query,
  resultCount,
  onQueryChange,
  onOpenCapture,
}: InboxToolbarProps) {
  return (
    <div className="inbox-toolbar">
      <label className="inbox-search">
        <Search aria-hidden="true" size={16} strokeWidth={1.8} />
        <span className="sr-only">搜索卡片</span>
        <input
          type="search"
          value={query}
          placeholder="搜索标题、内容或我的想法"
          onChange={(event) => onQueryChange(event.target.value)}
        />
      </label>
      <span className="inbox-result-count">{resultCount} 张卡片</span>
      <div className="inbox-capture-actions" aria-label="捕获入口">
        <Button
          aria-label="打开快速记录"
          size="small"
          variant="secondary"
          onClick={() => onOpenCapture('quick')}
        >
          <Lightbulb aria-hidden="true" size={14} />
          快速记录
        </Button>
        <Button
          aria-label="打开剪贴板捕获"
          size="small"
          variant="secondary"
          onClick={() => onOpenCapture('clipboard')}
        >
          <Clipboard aria-hidden="true" size={14} />
          剪贴板
        </Button>
        <Button
          aria-label="打开截图捕获"
          size="small"
          variant="secondary"
          onClick={() => onOpenCapture('screenshot')}
        >
          <ScanText aria-hidden="true" size={14} />
          截图
        </Button>
      </div>
    </div>
  )
}
