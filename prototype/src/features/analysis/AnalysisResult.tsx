import type { AnalysisOutput } from '../../entities/analysis/model'
import type { Card } from '../../entities/card/model'

interface AnalysisResultProps {
  output: AnalysisOutput
  cards: Card[]
  onReferenceClick: (cardId: string) => void
}

export function AnalysisResult({
  output,
  cards,
  onReferenceClick,
}: AnalysisResultProps) {
  const cardsById = new Map(cards.map((card) => [card.id, card]))

  return (
    <div className="analysis-result">
      <section className="analysis-result-section">
        <h2>直接回应</h2>
        <p>{output.summary}</p>
      </section>

      <section className="analysis-result-section">
        <h2>可延伸方向</h2>
        <div className="analysis-theme-list">
          {output.themes.map((theme) => (
            <article className="analysis-theme" key={theme.title}>
              <h3>{theme.title}</h3>
              <p>{theme.detail}</p>
              {theme.references.length > 0 ? (
                <div className="analysis-reference-list" aria-label="来源引用">
                  {theme.references.map((reference) => {
                    const cardTitle =
                      cardsById.get(reference.cardId)?.title ?? reference.label

                    return (
                      <button
                        className="analysis-reference"
                        key={`${theme.title}-${reference.cardId}`}
                        onClick={() => onReferenceClick(reference.cardId)}
                        type="button"
                      >
                        来源卡片：{cardTitle}
                      </button>
                    )
                  })}
                </div>
              ) : null}
            </article>
          ))}
        </div>
      </section>

      <ResultList title="需要查询或确认" items={output.conflicts} />
      <ResultList title="下一步行动" items={output.actions} />
      <ResultList title="尚未解决的问题" items={output.openQuestions} />
    </div>
  )
}

interface ResultListProps {
  title: string
  items: string[]
}

function ResultList({ title, items }: ResultListProps) {
  return (
    <section className="analysis-result-section">
      <h2>{title}</h2>
      <ul className="analysis-result-list">
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </section>
  )
}
