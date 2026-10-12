import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach, vi } from 'vitest'

document.elementFromPoint = vi.fn(() => null)

Range.prototype.getClientRects = vi.fn(() => ({
  item: () => null,
  length: 0,
  [Symbol.iterator]: function* iterator() {},
})) as typeof Range.prototype.getClientRects

Range.prototype.getBoundingClientRect = vi.fn(() => ({
  bottom: 0,
  height: 0,
  left: 0,
  right: 0,
  top: 0,
  width: 0,
  x: 0,
  y: 0,
  toJSON: () => ({}),
}))

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})
