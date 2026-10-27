import { Component, ErrorInfo, ReactNode } from "react";
import { useTranslation } from "react-i18next";

/** 出错时代替整块界面的那一屏：说一句、给"再试一次"（重画这一块），细节小字放下面。 */
function Fallback({ error, onRetry }: { error: Error; onRetry: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="module" data-active>
      <div className="empty crash">
        <h2>{t("crash.title")}</h2>
        <p className="ash">{t("crash.body")}</p>
        <div className="empty-actions">
          <button className="btn primary" onClick={onRetry}>{t("crash.retry")}</button>
          <button className="btn" onClick={() => location.reload()}>{t("crash.reload")}</button>
        </div>
        <p className="ash small mono crash-detail">{error.message}</p>
      </div>
    </div>
  );
}

/**
 * 主区域外面一层：里面哪个组件渲染时抛了异常，不再整窗白掉，而是换成一句说明加"再试一次"。
 * 项目、判定都在 projects.json 里，重画一次不丢东西；侧栏照常可用，可以去设置或换项目。
 */
export default class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state: { error: Error | null } = { error: null };
  static getDerivedStateFromError(error: Error) { return { error }; }
  componentDidCatch(error: Error, info: ErrorInfo) { console.error("界面出错：", error, info.componentStack); }
  render() {
    return this.state.error ? <Fallback error={this.state.error} onRetry={() => this.setState({ error: null })} /> : this.props.children;
  }
}
