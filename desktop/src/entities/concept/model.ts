import type { CardStatus } from '../card/model'
import type { RichDocument } from '../document/model'

export interface CardConcept {
  id: string
  name: string
  explanation: string
  mentionId: string
}

export interface ConceptSummary {
  id: string
  name: string
  explanation: string
  mentionCount: number
  updatedAt: string
}

export interface ConceptSource {
  mentionId: string
  cardId: string
  cardTitle: string
  selectedText: string
  cardStatus: CardStatus
  createdAt: string
}

export interface ConceptDetail extends ConceptSummary {
  mentions: ConceptSource[]
}

export interface SaveConceptAnnotationInput {
  cardId: string
  conceptId: string
  mentionId: string
  selectedText: string
  name: string
  explanation: string
  overwriteExisting: boolean
  document: RichDocument
}

export interface SavedConceptAnnotation {
  concept: ConceptSummary
  mention: CardConcept
  document: RichDocument
  originalContent: string
  updatedAt: string
}
