import { Link } from 'react-router-dom'

export function NotFoundPage() {
  return (
    <section className="page-section" aria-labelledby="not-found-title">
      <div className="not-found">
        <p className="page-eyebrow">404</p>
        <h1 id="not-found-title">页面未找到</h1>
        <p className="not-found-description">
          这个地址不存在，或者页面已经移动。
        </p>
        <Link className="not-found-link" to="/">
          返回收集箱
        </Link>
      </div>
    </section>
  )
}
