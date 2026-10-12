import { describe, expect, it } from 'vitest'

import { DesktopError } from './DesktopError'

const SENTINEL = 'SQL /private/ideacard.sqlite3 SENTINEL'

function convertWithoutThrowing(value: unknown): DesktopError {
  let result: DesktopError | undefined

  expect(() => {
    result = DesktopError.fromUnknown(value)
  }).not.toThrow()
  expect(result).toBeDefined()

  return result as DesktopError
}

function expectUnknownFallback(error: DesktopError) {
  expect(error).toMatchObject({
    name: 'DesktopError',
    code: 'unexpected',
    message: '发生未知错误',
    recovery: '',
  })
}

describe('DesktopError', () => {
  it('preserves stable code and recovery detail', () => {
    const error = DesktopError.fromUnknown({
      code: 'database_open_failed',
      message: '无法打开数据库',
      recovery: '检查数据目录',
    })

    expect(error).toMatchObject({
      name: 'DesktopError',
      code: 'database_open_failed',
      message: '无法打开数据库',
      recovery: '检查数据目录',
    })
  })

  it('returns an existing DesktopError unchanged', () => {
    const original = new DesktopError('capture_failed', '无法捕获内容', '请重试')

    expect(DesktopError.fromUnknown(original)).toBe(original)
  })

  it('does not expose a native Error message', () => {
    const error = DesktopError.fromUnknown(new Error(SENTINEL))

    expectUnknownFallback(error)
    expect(String(error)).not.toContain(SENTINEL)
    expect(JSON.stringify(error)).not.toContain(SENTINEL)
  })

  it('rejects a native Error with a stable-looking code', () => {
    const nativeError = Object.assign(new Error(SENTINEL), { code: 'ENOENT' })
    const error = DesktopError.fromUnknown(nativeError)

    expectUnknownFallback(error)
    expect(String(error)).not.toContain(SENTINEL)
    expect(JSON.stringify(error)).not.toContain(SENTINEL)
  })

  it.each([undefined, null, 'failure', 42, true, Symbol('failure'), 1n, () => undefined])(
    'uses the unknown error fallback for non-object input %#',
    (value) => {
      expect(DesktopError.fromUnknown(value)).toMatchObject({
        code: 'unexpected',
        message: '发生未知错误',
        recovery: '',
      })
    },
  )

  it.each([
    { code: { internal: 'database_failed' }, message: '无法读取数据' },
    { code: 'database_failed', message: { internal: 'SQL sentinel' } },
    { code: 500, message: '无法读取数据' },
    { code: 'database_failed', message: ['SQL sentinel'] },
  ])('uses the unknown error fallback for invalid contract fields %#', (value) => {
    const error = DesktopError.fromUnknown(value)

    expect(error).toMatchObject({
      code: 'unexpected',
      message: '发生未知错误',
      recovery: '',
    })
    expect(error.message).not.toContain('sentinel')
    expect(error.message).not.toContain('[object Object]')
  })

  it('rejects a non-string recovery value', () => {
    const error = DesktopError.fromUnknown({
      code: 'database_failed',
      message: '无法读取数据',
      recovery: { internal: SENTINEL },
    })

    expectUnknownFallback(error)
    expect(JSON.stringify(error)).not.toContain(SENTINEL)
  })

  it('accepts a stable payload without recovery detail', () => {
    const error = DesktopError.fromUnknown({
      code: 'capture_failed',
      message: '无法捕获内容',
    })

    expect(error).toMatchObject({
      code: 'capture_failed',
      message: '无法捕获内容',
      recovery: '',
    })
  })

  it.each([
    [
      'code getter',
      {
        get code() {
          throw new Error(SENTINEL)
        },
        message: '无法读取数据',
      },
    ],
    [
      'message getter',
      {
        code: 'database_failed',
        get message() {
          throw new Error(SENTINEL)
        },
      },
    ],
    [
      'recovery getter',
      {
        code: 'database_failed',
        message: '无法读取数据',
        get recovery() {
          throw new Error(SENTINEL)
        },
      },
    ],
  ])('contains failures from a throwing %s', (_, value) => {
    expectUnknownFallback(convertWithoutThrowing(value))
  })

  it('contains failures from a revoked Proxy', () => {
    const { proxy, revoke } = Proxy.revocable({}, {})
    revoke()

    expectUnknownFallback(convertWithoutThrowing(proxy))
  })

  it('contains failures from Symbol.hasInstance', () => {
    const descriptor = Object.getOwnPropertyDescriptor(DesktopError, Symbol.hasInstance)
    let error: DesktopError

    Object.defineProperty(DesktopError, Symbol.hasInstance, {
      configurable: true,
      value: () => {
        throw new Error(SENTINEL)
      },
    })

    try {
      error = convertWithoutThrowing({})
    } finally {
      if (descriptor) {
        Object.defineProperty(DesktopError, Symbol.hasInstance, descriptor)
      } else {
        Reflect.deleteProperty(DesktopError, Symbol.hasInstance)
      }
    }

    expectUnknownFallback(error)
  })
})
