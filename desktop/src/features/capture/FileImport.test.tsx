import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Card } from '../../entities/card/model'
import type { FileImportResult } from '../../services/desktop/contracts'
import { FileImport } from './FileImport'

const card = {
  id: 'card-imported',
  title: '方案',
  originalContent: '内容',
  note: '',
  sourceType: 'document',
  captureMethod: 'file',
  sourceName: '方案.pdf',
  sourceUri: '',
  status: 'unprocessed',
  attachments: [],
  createdAt: '2026-07-14T10:00:00Z',
  updatedAt: '2026-07-14T10:00:00Z',
  archivedAt: null,
} satisfies Card

function renderFileImport(onImport: () => Promise<FileImportResult[]>) {
  render(
    <FileImport onImport={onImport}>
      {({ isImporting, startImport }) => (
        <button disabled={isImporting} type="button" onClick={() => void startImport()}>
          导入本地文件
        </button>
      )}
    </FileImport>,
  )
}

describe('FileImport', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders successful and failed files from the same batch', async () => {
    const user = userEvent.setup()
    const onImport = vi.fn().mockResolvedValue([
      { originalName: '方案.pdf', status: 'saved', card, errorCode: null },
      { originalName: '损坏.docx', status: 'saved_with_extraction_failure', card: null, errorCode: 'file_extraction_failed' },
      { originalName: '超限.mp4', status: 'failed', card: null, errorCode: 'attachment_too_large' },
    ])
    renderFileImport(onImport)

    await user.click(screen.getByRole('button', { name: '导入本地文件' }))

    expect(await screen.findByText('方案.pdf')).toBeInTheDocument()
    expect(screen.getByText('导入成功')).toBeInTheDocument()
    expect(screen.getByText('已保存，内容提取失败')).toBeInTheDocument()
    expect(screen.getByText('文件超过大小限制')).toBeInTheDocument()
  })

  it('removes successful imports after one second', async () => {
    vi.useFakeTimers()
    renderFileImport(vi.fn().mockResolvedValue([
      { originalName: '方案.pdf', status: 'saved', card, errorCode: null },
    ]))

    fireEvent.click(screen.getByRole('button', { name: '导入本地文件' }))
    await act(async () => Promise.resolve())
    expect(screen.getByText('导入成功')).toBeInTheDocument()

    await act(async () => vi.advanceTimersByTime(999))
    expect(screen.getByText('导入成功')).toBeInTheDocument()
    await act(async () => vi.advanceTimersByTime(1))
    expect(screen.queryByText('导入成功')).not.toBeInTheDocument()
  })

  it('keeps failed results after successes expire and allows dismissing them', async () => {
    vi.useFakeTimers()
    renderFileImport(vi.fn().mockResolvedValue([
      { originalName: '方案.pdf', status: 'saved', card, errorCode: null },
      {
        originalName: '超限.mp4',
        status: 'failed',
        card: null,
        errorCode: 'attachment_too_large',
      },
    ]))

    fireEvent.click(screen.getByRole('button', { name: '导入本地文件' }))
    await act(async () => Promise.resolve())
    expect(screen.getByText('方案.pdf')).toBeInTheDocument()
    expect(screen.getByText('超限.mp4')).toBeInTheDocument()

    await act(async () => vi.advanceTimersByTime(1000))
    expect(screen.queryByText('方案.pdf')).not.toBeInTheDocument()
    expect(screen.getByText('超限.mp4')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '关闭导入结果' }))
    expect(screen.queryByLabelText('文件导入结果')).not.toBeInTheDocument()
  })
})
