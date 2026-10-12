import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { CaptureMenu } from './CaptureMenu'

describe('CaptureMenu', () => {
  it('opens the menu and closes after clipboard capture', async () => {
    const user = userEvent.setup()
    const onClipboard = vi.fn().mockResolvedValue(undefined)
    render(<CaptureMenu onClipboard={onClipboard} onImport={vi.fn().mockResolvedValue([])} />)

    await user.click(screen.getByRole('button', { name: '更多捕获方式' }))
    await user.click(screen.getByRole('menuitem', { name: '从剪贴板捕获' }))

    expect(onClipboard).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('closes with Escape and an outside click', async () => {
    const user = userEvent.setup()
    render(
      <div>
        <CaptureMenu onClipboard={vi.fn()} onImport={vi.fn().mockResolvedValue([])} />
        <button type="button">外部区域</button>
      </div>,
    )

    await user.click(screen.getByRole('button', { name: '更多捕获方式' }))
    expect(screen.getByRole('menu')).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '更多捕获方式' }))
    expect(screen.getByRole('menu')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '外部区域' }))
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('shows the supported file formats below the import action', async () => {
    const user = userEvent.setup()
    render(<CaptureMenu onClipboard={vi.fn()} onImport={vi.fn().mockResolvedValue([])} />)

    await user.click(screen.getByRole('button', { name: '更多捕获方式' }))

    expect(
      screen.getByText('支持 PDF、DOCX、PPTX、XLSX、MD、TXT、图片'),
    ).toBeInTheDocument()
  })

  it('keeps file import results after the menu closes', async () => {
    const user = userEvent.setup()
    const onImport = vi.fn().mockResolvedValue([
      { originalName: '超限.mp4', status: 'failed', card: null, errorCode: 'attachment_too_large' },
    ])
    render(<CaptureMenu onClipboard={vi.fn()} onImport={onImport} />)

    await user.click(screen.getByRole('button', { name: '更多捕获方式' }))
    await user.click(screen.getByRole('menuitem', { name: '导入本地文件' }))

    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(await screen.findByText('超限.mp4')).toBeInTheDocument()
    expect(screen.getByText('文件超过大小限制')).toBeInTheDocument()
  })
})
