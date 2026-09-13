import { ArrowDownRight, ArrowUpRight, FileUp, Save } from "lucide-react";
import { money, monthLabel, periodLabel, scoreTone } from "../format";
import { useI18n } from "../i18n";
import type { DashboardSnapshot, MonthlySnapshot } from "../types";

interface OverviewProps {
  snapshot: DashboardSnapshot;
  busy: boolean;
  status: string;
  onImport: () => void;
  onSaveReport: () => void;
}

function ScoreSegments({ score }: { score: number }) {
  const { L } = useI18n();
  const filled = Math.round(score / 5);
  return (
    <div className={`score-segments ${scoreTone(score)}`} aria-label={L.overview.scoreOutOf(score)}>
      {Array.from({ length: 20 }, (_, index) => (
        <span key={index} className={index < filled ? "filled" : ""} />
      ))}
    </div>
  );
}

function TrendChart({ data, currency }: { data: MonthlySnapshot[]; currency: string }) {
  const { L } = useI18n();
  if (data.length < 2) return <div className="chart-empty">{L.overview.needTrend}</div>;
  const values = data.map((item) => item.netFlowMinor);
  const min = Math.min(...values, 0);
  const max = Math.max(...values, 1);
  const range = Math.max(max - min, 1);
  const width = 720;
  const height = 220;
  const points = values
    .map((value, index) => {
      const x = data.length === 1 ? width / 2 : (index / (data.length - 1)) * width;
      const y = height - ((value - min) / range) * (height - 32) - 16;
      return `${x},${y}`;
    })
    .join(" ");
  const zeroY = height - ((0 - min) / range) * (height - 32) - 16;

  return (
    <div className="trend-chart">
      <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={L.overview.trendChart}>
        <line className="zero-line" x1="0" y1={zeroY} x2={width} y2={zeroY} />
        <polyline className="trend-line" points={points} />
        {values.map((_, index) => {
          const [x, y] = points.split(" ")[index].split(",");
          return <circle key={index} className="trend-point" cx={x} cy={y} r="4" />;
        })}
      </svg>
      <div className="chart-labels">
        {data.map((item) => (
          <span key={`${item.year}-${item.month}`} title={money(item.netFlowMinor, currency, L.locale)}>
            {monthLabel(item.year, item.month, L.locale)}
          </span>
        ))}
      </div>
    </div>
  );
}

function Metric({ name, value, tone }: { name: string; value: string; tone?: string }) {
  return (
    <div className="metric-row">
      <span>{name}</span>
      <strong className={tone}>{value}</strong>
    </div>
  );
}

export function Overview({ snapshot, busy, status, onImport, onSaveReport }: OverviewProps) {
  const { L, categoryLabel, valueLabel } = useI18n();
  const { health, categories, currency, transactionCount } = snapshot;
  if (transactionCount === 0) {
    return (
      <main className="workspace empty-workspace">
        <div className="empty-instrument dot-grid-subtle">
          <span className="eyebrow">{L.overview.emptyEyebrow}</span>
          <h1>{L.overview.emptyTitle.split("\n").map((line, index) => <span key={line}>{index > 0 && <br />}{line}</span>)}</h1>
          <p>{L.overview.emptyDescription}</p>
          <button className="primary-button" onClick={onImport} disabled={busy}>
            <FileUp size={17} /> {L.header.importOfx}
          </button>
          <span className="inline-status">{status || L.overview.waiting}</span>
        </div>
      </main>
    );
  }

  const trendPositive = health.trendDirection === "improving";
  const componentRows = [
    [L.overview.components.netFlow, health.components.netFlowScore, false],
    [L.overview.components.stability, health.components.stabilityScore, false],
    [L.overview.components.trend, health.components.trendScore, false],
    [L.overview.components.savings, health.components.savingsScore, false],
    [L.overview.components.volatility, health.components.volatilityPenalty, true],
    [L.overview.components.commitment, health.components.overcommitmentPenalty, true],
  ] as const;

  return (
    <main className="workspace overview-workspace">
      <section className="overview-hero">
        <div className="score-block">
          <span className="eyebrow">GATORHEALTH / {health.engineVersion}</span>
          <div className={`hero-score ${scoreTone(health.score)}`}>
            {health.score.toFixed(1)}<small>/100</small>
          </div>
          <ScoreSegments score={health.score} />
          <div className="score-caption">
            <span>{L.overview.grade} {health.grade}</span>
            <span className={trendPositive ? "good" : health.trendDirection === "declining" ? "danger" : ""}>
              {trendPositive ? <ArrowUpRight size={15} /> : <ArrowDownRight size={15} />}
              {valueLabel(health.trendDirection)}
            </span>
          </div>
        </div>

        <div className="hero-meta">
          <Metric name={L.overview.period} value={periodLabel(health.periodStart, health.periodEnd, L.locale, L.overview.noData)} />
          <Metric name={L.overview.months} value={String(health.monthsAnalyzed)} />
          <Metric name={L.overview.transactions} value={String(transactionCount)} />
          <Metric name={L.overview.accounts} value={String(snapshot.accountCount)} />
          <div className="hero-actions">
            <button className="secondary-button" onClick={onImport} disabled={busy}><FileUp size={16} /> {L.overview.import}</button>
            <button className="primary-button" onClick={onSaveReport} disabled={busy}><Save size={16} /> {L.overview.saveReport}</button>
          </div>
          <span className="inline-status">{status || L.overview.databaseReady}</span>
        </div>
      </section>

      <section className="overview-stats section-band">
        <Metric name={L.overview.avgIncome} value={money(health.avgMonthlyIncomeMinor, currency, L.locale)} tone="good" />
        <Metric name={L.overview.avgExpenses} value={money(health.avgMonthlyExpensesMinor, currency, L.locale)} />
        <Metric
          name={L.overview.avgNetFlow}
          value={money(health.avgMonthlyNetFlowMinor, currency, L.locale)}
          tone={health.avgMonthlyNetFlowMinor >= 0 ? "good" : "danger"}
        />
      </section>

      <section className="analysis-grid section-band">
        <div className="trend-section">
          <div className="section-heading">
            <div><span className="eyebrow">{L.overview.signal}</span><h2>{L.overview.netFlowOverTime}</h2></div>
            <strong className={health.trendDirection === "improving" ? "good" : ""}>{valueLabel(health.trendDirection)}</strong>
          </div>
          <TrendChart data={health.monthlySnapshots} currency={currency} />
        </div>

        <div className="components-section">
          <div className="section-heading"><div><span className="eyebrow">{L.overview.componentsEyebrow}</span><h2>{L.overview.scoreAnatomy}</h2></div></div>
          <div className="component-list">
            {componentRows.map(([name, value, penalty]) => (
              <div className="component-row" key={name}>
                <div><span>{name}</span><strong>{(value * 100).toFixed(0)}%</strong></div>
                <div className={`mini-segments ${penalty && value > 0.3 ? "danger" : ""}`}>
                  {Array.from({ length: 10 }, (_, index) => <span key={index} className={index < Math.round(value * 10) ? "filled" : ""} />)}
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="category-section section-band">
        <div className="section-heading"><div><span className="eyebrow">{L.overview.expenseMap}</span><h2>{L.overview.whereMoneyWent}</h2></div></div>
        <div className="category-list">
          {categories.slice(0, 8).map((category, index) => (
            <div className="category-row" key={category.category}>
              <span className="category-index">{String(index + 1).padStart(2, "0")}</span>
              <div className="category-main">
                <div><strong>{categoryLabel(category.category)}</strong><span>{category.count} {L.overview.transactionAbbr}</span></div>
                <div className="category-track"><span style={{ width: `${category.percentage}%` }} /></div>
              </div>
              <div className="category-value"><strong>{money(category.amountMinor, currency, L.locale)}</strong><span>{category.percentage.toFixed(1)}%</span></div>
            </div>
          ))}
        </div>
      </section>
    </main>
  );
}
