export class DesktopError extends Error {
  readonly name = 'DesktopError'
  readonly code: string
  readonly recovery: string

  constructor(code: string, message: string, recovery = '') {
    super(message)
    this.code = code
    this.recovery = recovery
  }

  static fromUnknown(value: unknown): DesktopError {
    try {
      if (value instanceof DesktopError) {
        return value
      }

      if (value instanceof Error) {
        return new DesktopError('unexpected', '发生未知错误')
      }

      if (value && typeof value === 'object') {
        const item = value as { code?: unknown; message?: unknown; recovery?: unknown }
        const code = item.code

        if (typeof code === 'string') {
          const message = item.message

          if (typeof message === 'string') {
            const recovery = item.recovery

            if (recovery === undefined || typeof recovery === 'string') {
              return new DesktopError(code, message, recovery)
            }
          }
        }
      }
    } catch {
      return new DesktopError('unexpected', '发生未知错误')
    }

    return new DesktopError('unexpected', '发生未知错误')
  }
}
