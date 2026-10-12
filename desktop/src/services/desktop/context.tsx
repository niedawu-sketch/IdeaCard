import type { ReactNode } from 'react'

import type { DesktopService } from './contracts'
import { tauriDesktopService } from './tauriClient'
import { DesktopServiceContext } from './useDesktopService'

interface DesktopServiceProviderProps {
  children: ReactNode
  service?: DesktopService
}

export function DesktopServiceProvider({
  children,
  service = tauriDesktopService,
}: DesktopServiceProviderProps) {
  return (
    <DesktopServiceContext.Provider value={service}>{children}</DesktopServiceContext.Provider>
  )
}
