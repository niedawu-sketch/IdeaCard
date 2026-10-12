import { ExternalLink } from 'lucide-react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'

interface AiMarkdownProps {
  content: string
  onOpenLink(url: string): void
}

function safeWebUrl(value: string | undefined) {
  if (!value) return null
  try {
    const url = new URL(value)
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.toString() : null
  } catch {
    return null
  }
}

export function AiMarkdown({ content, onOpenLink }: AiMarkdownProps) {
  return (
    <div className="ai-markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        components={{
          a({ href, children }) {
            const url = safeWebUrl(href)
            if (!url) return <span>{children}</span>
            return (
              <a
                href={url}
                onClick={(event) => {
                  event.preventDefault()
                  onOpenLink(url)
                }}
              >
                {children}
                <ExternalLink aria-hidden="true" size={11} />
              </a>
            )
          },
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  )
}
