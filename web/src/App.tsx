import { ThemeToggle } from './shared/ThemeToggle';

export function App() {
  return (
    <div className="app-shell">
      <header className="app-header">
        <span className="site-name">Oxipage</span>
        <div className="header-actions">
          <ThemeToggle />
        </div>
      </header>
      <main className="card">
        <p className="text-secondary">설계 토큰 스캐폴드 — Task 6에서 콘텐츠가 들어옵니다.</p>
      </main>
    </div>
  );
}
