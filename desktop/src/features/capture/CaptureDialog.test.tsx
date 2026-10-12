import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { CaptureDraft } from '../../entities/capture/model'
import { CaptureDialog } from './CaptureDialog'

const urlDraft: CaptureDraft = {
  kind: 'clipboard',
  title: '',
  content: 'https://example.com/article',
  note: '',
  sourceType: 'document',
  sourceName: 'example.com',
  sourceUri: 'https://example.com/article',
  temporaryAttachmentIds: [],
}

describe('CaptureDialog', () => {
  it('closes the capture dialog when Escape is pressed', async () => {
    const user = userEvent.setup()
    const onClose = vi.fn()
    render(<CaptureDialog draft={urlDraft} onClose={onClose} onSave={vi.fn()} />)

    await user.keyboard('{Escape}')

    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('shows URL classification and keeps edited content after save failure', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockRejectedValue({
      code: 'database_operation_failed',
      message: '保存失败',
      recovery: '重新打开应用',
    })
    render(<CaptureDialog draft={urlDraft} onClose={vi.fn()} onSave={onSave} />)
    const dialog = screen.getByRole('dialog', { name: '确认捕获' })

    expect(within(dialog).getByText('文档资料')).toBeInTheDocument()
    const content = within(dialog).getByLabelText('记录内容')
    await user.type(content, '\n我的补充')
    await user.click(within(dialog).getByRole('button', { name: '保存' }))

    expect(await within(dialog).findByRole('alert')).toHaveTextContent('保存失败')
    expect(content).toHaveValue('https://example.com/article\n我的补充')
  })

  it('allows an OCR-empty image draft to be saved when it has an attachment', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue({})
    render(
      <CaptureDialog
        draft={{
          ...urlDraft,
          content: '',
          sourceType: 'personal',
          sourceName: '剪贴板图片',
          sourceUri: '',
          temporaryAttachmentIds: ['image-id'],
        }}
        onClose={vi.fn()}
        onSave={onSave}
      />,
    )

    expect(screen.getByText('个人记录')).toBeInTheDocument()
    expect(screen.queryByText('已附原图')).not.toBeInTheDocument()
    const save = screen.getByRole('button', { name: '保存' })
    expect(save).toBeEnabled()
    await user.click(save)
    expect(onSave).toHaveBeenCalledTimes(1)
  })
})
