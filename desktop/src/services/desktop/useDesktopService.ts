import { createContext, useContext } from 'react'

import type { DesktopService } from './contracts'

export const DesktopServiceContext = createContext<DesktopService | null>(null)

export function useDesktopService() {
  const service = useContext(DesktopServiceContext)
  if (!service) {
    throw new Error('useDesktopService 必须在 DesktopServiceProvider 内使用')
  }
  return service
}
