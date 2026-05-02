import React, { useMemo, useState } from 'react';
import {
  Chart as ChartJS,
  BarElement,
  BubbleController,
  CategoryScale,
  LinearScale,
  PointElement,
  Tooltip,
  Legend,
} from 'chart.js';
import { Bar, Bubble } from 'react-chartjs-2';
import DashboardCardFrame from '../dashboard/DashboardCardFrame';
import MetricToggleGroup from '../dashboard/MetricToggleGroup';

ChartJS.register(BarElement, BubbleController, CategoryScale, LinearScale, PointElement, Tooltip, Legend);

const CHART_FONT_FAMILY =
  '"MS Sans Serif", "Microsoft Sans Serif", Tahoma, Verdana, sans-serif';

ChartJS.defaults.font.family = CHART_FONT_FAMILY;
ChartJS.defaults.color = '#d4d4d4';

const METRICS = ['expectancy', 'avg_return', 'win_rate', 'closed_count', 'closed_rate'];

const METRIC_META = {
  expectancy: {
    label: 'Expectancy',
    axisLabel: 'Expectancy (%)',
    percent: true,
    scale: 1,
  },
  avg_return: {
    label: 'Avg Return',
    axisLabel: 'Average Return (%)',
    percent: true,
    scale: 1,
  },
  win_rate: {
    label: 'Win %',
    axisLabel: 'Win Rate (%)',
    percent: true,
    scale: 100,
  },
  closed_count: {
    label: 'Closed Trades',
    axisLabel: 'Closed Trades',
    percent: false,
    scale: 1,
  },
  closed_rate: {
    label: 'Closed %',
    axisLabel: 'Closed Coverage (%)',
    percent: true,
    scale: 100,
  },
};

const HARMONIC_COLORS = {
  Bat: {
    fill: 'rgba(34, 139, 230, 0.92)',
    border: 'rgba(170, 220, 255, 0.98)',
  },
  Butterfly: {
    fill: 'rgba(32, 178, 120, 0.92)',
    border: 'rgba(176, 250, 214, 0.98)',
  },
  Gartley: {
    fill: 'rgba(236, 170, 32, 0.94)',
    border: 'rgba(255, 231, 163, 0.98)',
  },
  Crab: {
    fill: 'rgba(214, 78, 78, 0.94)',
    border: 'rgba(255, 184, 184, 0.98)',
  },
  Shark: {
    fill: 'rgba(128, 96, 214, 0.94)',
    border: 'rgba(214, 196, 255, 0.98)',
  },
};

const DEFAULT_HARMONIC_COLOR = {
  fill: 'rgba(68, 145, 210, 0.92)',
  border: 'rgba(185, 224, 255, 0.98)',
};

const SELECTED_STRATEGY_FILL = 'rgba(255, 244, 168, 0.98)';
const SELECTED_STRATEGY_BORDER = 'rgba(255, 255, 255, 0.98)';

const getHarmonicColor = (harmonicType) => HARMONIC_COLORS[harmonicType] ?? DEFAULT_HARMONIC_COLOR;

const getSummary = (strategy) => strategy?.comparison?.summary ?? null;

const getMetricValue = (strategy, metric) => {
  const summary = getSummary(strategy);
  if (!summary) return 0;

  switch (metric) {
    case 'closed_count':
      return summary.closed_count ?? 0;
    case 'closed_rate':
      return summary.total_count ? (summary.closed_count ?? 0) / summary.total_count : 0;
    case 'win_rate':
      return summary.win_rate ?? 0;
    case 'avg_return':
      return summary.avg_return ?? 0;
    case 'expectancy':
    default:
      return summary.expectancy ?? 0;
  }
};

const getChartValue = (strategy, metric) => {
  const rawValue = getMetricValue(strategy, metric);
  return rawValue * (METRIC_META[metric]?.scale ?? 1);
};

const formatMetric = (metric, value) => {
  if (!Number.isFinite(value)) return 'N/A';
  return METRIC_META[metric]?.percent ? `${value.toFixed(2)}%` : `${Math.round(value)}`;
};

const formatPercentValue = (value) =>
  Number.isFinite(value) ? `${value.toFixed(2)}%` : 'N/A';

const buildStrategyLabel = (strategy) =>
  `${strategy.harmonicType} ${strategy.market} ${strategy.bin}`;

const buildStrategyTooltipLines = (strategy, metricX, metricY) => {
  if (!strategy) return [];

  const summary = getSummary(strategy);

  return [
    buildStrategyLabel(strategy),
    `Size: ${strategy.sizeBucket}`,
    `Time Bin: ${strategy.timeBin}`,
    `${METRIC_META[metricX].label}: ${formatMetric(metricX, getChartValue(strategy, metricX))}`,
    `${METRIC_META[metricY].label}: ${formatMetric(metricY, getChartValue(strategy, metricY))}`,
    `Closed Trades: ${summary?.closed_count ?? 0}/${summary?.total_count ?? 0}`,
    `Wins/Losses/Open: ${summary?.win_count ?? 0}/${summary?.loss_count ?? 0}/${summary?.open_count ?? 0}`,
  ];
};

function StrategyRankedChart({
  strategies,
  selectedStrategyId,
  onSelectStrategy,
  onHoverStrategy,
  leaderChartStartIndex = 0,
}) {
  const [metric, setMetric] = useState('expectancy');

  const rankedStrategies = useMemo(() => {
    const filtered = strategies.filter((strategy) => getSummary(strategy));
    const startIndex = Math.max(0, Math.min(leaderChartStartIndex, Math.max(0, filtered.length - 1)));

    return filtered.slice(startIndex, startIndex + 12);
  }, [leaderChartStartIndex, strategies]);

  const rankEnd = Math.min(strategies.length, leaderChartStartIndex + rankedStrategies.length);

  const chartData = useMemo(
    () => ({
      labels: rankedStrategies.map((strategy) => buildStrategyLabel(strategy)),
      datasets: [
        {
          label: METRIC_META[metric].label,
          data: rankedStrategies.map((strategy) => getChartValue(strategy, metric)),
          backgroundColor: rankedStrategies.map((strategy) =>
            strategy.id === selectedStrategyId
              ? SELECTED_STRATEGY_FILL
              : getHarmonicColor(strategy.harmonicType).fill
          ),
          borderColor: rankedStrategies.map((strategy) =>
            strategy.id === selectedStrategyId
              ? SELECTED_STRATEGY_BORDER
              : getHarmonicColor(strategy.harmonicType).border
          ),
          borderWidth: rankedStrategies.map((strategy) => (strategy.id === selectedStrategyId ? 2 : 1)),
          borderRadius: 0,
          borderSkipped: false,
        },
      ],
    }),
    [metric, rankedStrategies, selectedStrategyId]
  );

  const options = useMemo(
    () => ({
      indexAxis: 'y',
      responsive: true,
      maintainAspectRatio: false,
      onClick: (_, elements) => {
        const strategy = rankedStrategies[elements?.[0]?.index];
        if (strategy) {
          onSelectStrategy?.(strategy.id);
        }
      },
      onHover: (event, elements) => {
        const strategy = rankedStrategies[elements?.[0]?.index];
        onHoverStrategy?.(strategy?.id ?? '');

        if (event?.native?.target) {
          event.native.target.style.cursor = strategy ? 'pointer' : 'default';
        }
      },
      plugins: {
        legend: {
          display: false,
        },
        tooltip: {
          backgroundColor: 'rgba(12, 12, 12, 0.98)',
          titleColor: '#f0f0f0',
          bodyColor: '#d8d8d8',
          titleFont: { family: CHART_FONT_FAMILY },
          bodyFont: { family: CHART_FONT_FAMILY },
          borderColor: 'rgba(88, 88, 88, 0.9)',
          borderWidth: 1,
          callbacks: {
            label: (context) => {
              const strategy = rankedStrategies[context.dataIndex];
              return buildStrategyTooltipLines(strategy, metric, metric);
            },
          },
        },
      },
      scales: {
        x: {
          ticks: { color: '#d4d4d4', font: { family: CHART_FONT_FAMILY } },
          grid: { color: 'rgba(126, 126, 126, 0.18)' },
          border: { color: 'rgba(102, 102, 102, 0.58)' },
          title: {
            display: true,
            text: METRIC_META[metric].axisLabel,
            color: '#ececec',
            font: { family: CHART_FONT_FAMILY, size: 11, weight: 'bold' },
          },
        },
        y: {
          ticks: {
            color: '#d4d4d4',
            font: { family: CHART_FONT_FAMILY },
            callback: (_, index) => {
              const strategy = rankedStrategies[index];
              return strategy ? `${strategy.harmonicType} ${strategy.bin}` : '';
            },
          },
          grid: { color: 'rgba(92, 92, 92, 0.1)' },
          border: { color: 'rgba(102, 102, 102, 0.58)' },
        },
      },
    }),
    [metric, onHoverStrategy, onSelectStrategy, rankedStrategies]
  );

  return (
    <DashboardCardFrame
      title="Leaders"
      subtitle={`Family table ranks ${leaderChartStartIndex + 1}-${rankEnd} of ${strategies.length}`}
      controls={<MetricToggleGroup activeMetric={metric} onChange={setMetric} metrics={METRICS} />}
      bodyClassName="strategy-chart-body"
    >
      <div className="strategy-chart-hover-shell" onMouseLeave={() => onHoverStrategy?.('')}>
        <Bar data={chartData} options={options} />
      </div>
    </DashboardCardFrame>
  );
}

function StrategyYearlyChart({ strategy, isLoading = false }) {
  const [metric, setMetric] = useState('expectancy');
  const yearlyPerformance = useMemo(
    () => strategy?.comparison?.yearly_performance ?? [],
    [strategy]
  );

  const chartData = useMemo(
    () => ({
      labels: yearlyPerformance.map((item) => `${item.year}`),
      datasets: [
        {
          label: METRIC_META[metric]?.label ?? metric,
          data: yearlyPerformance.map(
            (item) => (item?.[metric] ?? 0) * (METRIC_META[metric]?.scale ?? 1)
          ),
          backgroundColor: yearlyPerformance.map((item) => {
            const value = item?.[metric] ?? 0;
            return value >= 0
              ? 'rgba(116, 224, 170, 0.94)'
              : 'rgba(255, 128, 128, 0.94)';
          }),
          borderColor: 'rgba(28, 28, 28, 0.92)',
          borderWidth: 1,
          borderRadius: 0,
          borderSkipped: false,
        },
      ],
    }),
    [metric, yearlyPerformance]
  );

  const options = useMemo(
    () => ({
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: {
          display: false,
        },
        tooltip: {
          backgroundColor: 'rgba(12, 12, 12, 0.98)',
          titleColor: '#f0f0f0',
          bodyColor: '#d8d8d8',
          titleFont: { family: CHART_FONT_FAMILY },
          bodyFont: { family: CHART_FONT_FAMILY },
          borderColor: 'rgba(88, 88, 88, 0.9)',
          borderWidth: 1,
          callbacks: {
            label: (context) => {
              const point = yearlyPerformance[context.dataIndex];
              if (!point) {
                return [];
              }

              return [
                `Year: ${point.year}`,
                `${METRIC_META[metric].label}: ${formatMetric(metric, context.raw)}`,
                `Closed Trades: ${point.closed_count}/${point.total_count}`,
                `Wins/Losses/Open: ${point.win_count}/${point.loss_count}/${point.open_count}`,
                `Win %: ${formatMetric('win_rate', (point.win_rate ?? 0) * 100)}`,
              ];
            },
          },
        },
      },
      scales: {
        x: {
          ticks: { color: '#d4d4d4', font: { family: CHART_FONT_FAMILY } },
          grid: { color: 'rgba(126, 126, 126, 0.18)' },
          border: { color: 'rgba(102, 102, 102, 0.58)' },
          title: {
            display: true,
            text: 'Year',
            color: '#ececec',
            font: { family: CHART_FONT_FAMILY, size: 11, weight: 'bold' },
          },
        },
        y: {
          ticks: {
            color: '#d4d4d4',
            font: { family: CHART_FONT_FAMILY },
            callback: (value) =>
              METRIC_META[metric]?.percent ? `${value}%` : `${value}`,
          },
          grid: { color: 'rgba(126, 126, 126, 0.18)' },
          border: { color: 'rgba(102, 102, 102, 0.58)' },
          title: {
            display: true,
            text: METRIC_META[metric]?.axisLabel ?? metric,
            color: '#ececec',
            font: { family: CHART_FONT_FAMILY, size: 11, weight: 'bold' },
          },
        },
      },
    }),
    [metric, yearlyPerformance]
  );

  return (
    <DashboardCardFrame
      title="Yearly"
      subtitle="Consistency by year"
      controls={
        <MetricToggleGroup
          activeMetric={metric}
          onChange={setMetric}
          metrics={['expectancy', 'avg_return', 'win_rate', 'closed_count']}
        />
      }
      bodyClassName="strategy-chart-body"
    >
      {yearlyPerformance.length ? (
        <Bar data={chartData} options={options} />
      ) : isLoading ? (
        <div className="strategy-empty-row">Loading yearly profile...</div>
      ) : (
        <div className="strategy-empty-row">No yearly rows for the selected strategy.</div>
      )}
    </DashboardCardFrame>
  );
}

const fillContractWeeks = (contractWeeks = []) => {
  const grouped = new Map();

  contractWeeks.forEach((row) => {
    const week = Number(row.contract_week_index ?? 0);
    const current = grouped.get(week) ?? {
      contract_week_index: week,
      total_count: 0,
      closed_count: 0,
      open_count: 0,
      win_count: 0,
      loss_count: 0,
      expectancy_sum: 0,
      avg_return_sum: 0,
    };

    const total = Number(row.total_count ?? 0);
    const closed = Number(row.closed_count ?? 0);
    current.total_count += total;
    current.closed_count += closed;
    current.open_count += Number(row.open_count ?? 0);
    current.win_count += Number(row.win_count ?? 0);
    current.loss_count += Number(row.loss_count ?? 0);
    current.expectancy_sum += Number(row.expectancy ?? 0) * closed;
    current.avg_return_sum += Number(row.avg_return ?? 0) * total;
    grouped.set(week, current);
  });

  const maxWeek = Math.max(0, ...[...grouped.keys()]);
  const rows = [];

  for (let week = 1; week <= maxWeek; week += 1) {
    const row = grouped.get(week) ?? {
      contract_week_index: week,
      total_count: 0,
      closed_count: 0,
      open_count: 0,
      win_count: 0,
      loss_count: 0,
      expectancy_sum: 0,
      avg_return_sum: 0,
    };

    rows.push({
      ...row,
      expectancy: row.closed_count ? row.expectancy_sum / row.closed_count : 0,
      avg_return: row.total_count ? row.avg_return_sum / row.total_count : 0,
      win_rate: row.closed_count ? row.win_count / row.closed_count : 0,
    });
  }

  return rows;
};

function StrategyContractWeekChart({ contractWeeks = [], isLoading = false }) {
  const [metric, setMetric] = useState('expectancy');
  const weeklyRows = useMemo(() => fillContractWeeks(contractWeeks), [contractWeeks]);

  const chartData = useMemo(
    () => ({
      labels: weeklyRows.map((item) => `W${item.contract_week_index}`),
      datasets: [
        {
          label: METRIC_META[metric]?.label ?? metric,
          data: weeklyRows.map((item) => {
            if (metric === 'closed_count') return item.closed_count ?? 0;
            return (item?.[metric] ?? 0) * (METRIC_META[metric]?.scale ?? 1);
          }),
          backgroundColor: weeklyRows.map((item) =>
            (item?.[metric] ?? 0) >= 0
              ? 'rgba(116, 224, 170, 0.9)'
              : 'rgba(255, 128, 128, 0.9)'
          ),
          borderColor: 'rgba(28, 28, 28, 0.92)',
          borderWidth: 1,
          borderRadius: 0,
          borderSkipped: false,
        },
      ],
    }),
    [metric, weeklyRows]
  );

  const options = useMemo(
    () => ({
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: { display: false },
        tooltip: {
          backgroundColor: 'rgba(12, 12, 12, 0.98)',
          titleColor: '#f0f0f0',
          bodyColor: '#d8d8d8',
          titleFont: { family: CHART_FONT_FAMILY },
          bodyFont: { family: CHART_FONT_FAMILY },
          borderColor: 'rgba(88, 88, 88, 0.9)',
          borderWidth: 1,
          callbacks: {
            label: (context) => {
              const point = weeklyRows[context.dataIndex];
              if (!point) return [];

              return [
                `Contract Week: W${point.contract_week_index}`,
                `${METRIC_META[metric]?.label ?? metric}: ${
                  metric === 'closed_count' ? Math.round(context.raw) : formatMetric(metric, context.raw)
                }`,
                `Closed Trades: ${point.closed_count}/${point.total_count}`,
                `Wins/Losses/Open: ${point.win_count}/${point.loss_count}/${point.open_count}`,
              ];
            },
          },
        },
      },
      scales: {
        x: {
          ticks: { color: '#d4d4d4', font: { family: CHART_FONT_FAMILY } },
          grid: { color: 'rgba(126, 126, 126, 0.12)' },
          border: { color: 'rgba(102, 102, 102, 0.58)' },
        },
        y: {
          ticks: {
            color: '#d4d4d4',
            font: { family: CHART_FONT_FAMILY },
            callback: (value) =>
              metric === 'closed_count' ? `${value}` : `${value}%`,
          },
          grid: { color: 'rgba(126, 126, 126, 0.18)' },
          border: { color: 'rgba(102, 102, 102, 0.58)' },
        },
      },
    }),
    [metric, weeklyRows]
  );

  return (
    <DashboardCardFrame
      title="Selected Contract Weeks"
      subtitle="Performance by week inside futures contracts"
      controls={
        <MetricToggleGroup
          activeMetric={metric}
          onChange={setMetric}
          metrics={['expectancy', 'avg_return', 'win_rate', 'closed_count']}
        />
      }
      bodyClassName="strategy-chart-body"
    >
      {weeklyRows.length ? (
        <Bar data={chartData} options={options} />
      ) : isLoading ? (
        <div className="strategy-empty-row">Loading contract weeks...</div>
      ) : (
        <div className="strategy-empty-row">No contract-week rows for this family yet.</div>
      )}
    </DashboardCardFrame>
  );
}

function StrategyCohortMap({ strategies, selectedStrategyId, onSelectStrategy, onHoverStrategy }) {
  const plottedStrategies = useMemo(
    () => strategies.filter((strategy) => getSummary(strategy)?.closed_count > 0),
    [strategies]
  );

  const chartData = useMemo(
    () => ({
      datasets: [
        {
          label: 'Cohorts',
          data: plottedStrategies.map((strategy) => {
            const summary = getSummary(strategy);
            const winRate = Number.isFinite(summary?.win_rate) ? summary.win_rate : 0;

            return {
              x: summary?.closed_count ?? 0,
              y: summary?.expectancy ?? 0,
              r: Math.max(6, Math.min(16, 6 + winRate * 10)),
              strategyId: strategy.id,
            };
          }),
          backgroundColor: plottedStrategies.map((strategy) =>
            strategy.id === selectedStrategyId
              ? SELECTED_STRATEGY_FILL
              : getHarmonicColor(strategy.harmonicType).fill
          ),
          borderColor: plottedStrategies.map((strategy) =>
            strategy.id === selectedStrategyId
              ? SELECTED_STRATEGY_BORDER
              : getHarmonicColor(strategy.harmonicType).border
          ),
          borderWidth: plottedStrategies.map((strategy) => (strategy.id === selectedStrategyId ? 2 : 1)),
          hoverBorderWidth: 2,
        },
      ],
    }),
    [plottedStrategies, selectedStrategyId]
  );

  const options = useMemo(
    () => ({
      responsive: true,
      maintainAspectRatio: false,
      onClick: (_, elements) => {
        const strategy = plottedStrategies[elements?.[0]?.index];
        if (strategy) {
          onSelectStrategy?.(strategy.id);
        }
      },
      onHover: (event, elements) => {
        const strategy = plottedStrategies[elements?.[0]?.index];
        onHoverStrategy?.(strategy?.id ?? '');

        if (event?.native?.target) {
          event.native.target.style.cursor = strategy ? 'pointer' : 'default';
        }
      },
      plugins: {
        legend: {
          display: false,
        },
        tooltip: {
          backgroundColor: 'rgba(12, 12, 12, 0.98)',
          titleColor: '#f0f0f0',
          bodyColor: '#d8d8d8',
          titleFont: { family: CHART_FONT_FAMILY },
          bodyFont: { family: CHART_FONT_FAMILY },
          borderColor: 'rgba(88, 88, 88, 0.9)',
          borderWidth: 1,
          callbacks: {
            label: (context) => {
              const strategy = plottedStrategies[context.dataIndex];
              if (!strategy) {
                return [];
              }

              const summary = getSummary(strategy);
                return [
                buildStrategyLabel(strategy),
                `Size: ${strategy.sizeBucket}`,
                `Time Bin: ${strategy.timeBin}`,
                `Closed Trades: ${summary?.closed_count ?? 0}`,
                `Expectancy: ${formatPercentValue(summary?.expectancy)}`,
                `Win Rate: ${
                  Number.isFinite(summary?.win_rate)
                    ? formatPercentValue(summary.win_rate * 100)
                    : 'N/A'
                }`,
                `Avg Return: ${formatPercentValue(summary?.avg_return)}`,
              ];
            },
          },
        },
      },
      scales: {
        x: {
          ticks: { color: '#d4d4d4', font: { family: CHART_FONT_FAMILY } },
          grid: { color: 'rgba(126, 126, 126, 0.18)' },
          border: { color: 'rgba(102, 102, 102, 0.58)' },
          title: {
            display: true,
            text: 'Closed Trades',
            color: '#ececec',
            font: { family: CHART_FONT_FAMILY, size: 11, weight: 'bold' },
          },
        },
        y: {
          ticks: {
            color: '#d4d4d4',
            font: { family: CHART_FONT_FAMILY },
            callback: (value) => `${value}%`,
          },
          grid: { color: 'rgba(126, 126, 126, 0.18)' },
          border: { color: 'rgba(102, 102, 102, 0.58)' },
          title: {
            display: true,
            text: 'Expectancy (%)',
            color: '#ececec',
            font: { family: CHART_FONT_FAMILY, size: 11, weight: 'bold' },
          },
        },
      },
    }),
    [onHoverStrategy, onSelectStrategy, plottedStrategies]
  );

  return (
    <DashboardCardFrame
      title="Cohort Map"
      subtitle="Sample size versus edge"
      controls={<span className="dashboard-card-label">Bubble = Win %</span>}
      bodyClassName="strategy-chart-body"
    >
      <div className="strategy-chart-hover-shell" onMouseLeave={() => onHoverStrategy?.('')}>
        <Bubble data={chartData} options={options} />
      </div>
    </DashboardCardFrame>
  );
}

export default function StrategyInsightCharts({
  strategies = [],
  selectedStrategy = null,
  selectedStrategyId = '',
  isHydratingStrategy = false,
  contractWeeks = [],
  isLoadingContractWeeks = false,
  leaderChartStartIndex = 0,
  onSelectStrategy,
  onHoverStrategy,
}) {
  const [graphPage, setGraphPage] = useState('filtered');

  if (!strategies.length) {
    return null;
  }

  return (
    <div className="strategy-graphs-shell">
      <div className="strategy-graphs-tabs">
        <button
          type="button"
          className={
            graphPage === 'filtered'
              ? 'strategy-graphs-tab strategy-graphs-tab--active'
              : 'strategy-graphs-tab'
          }
          onClick={() => setGraphPage('filtered')}
        >
          Filtered Strategies
        </button>
        <button
          type="button"
          className={
            graphPage === 'selected'
              ? 'strategy-graphs-tab strategy-graphs-tab--active'
              : 'strategy-graphs-tab'
          }
          onClick={() => setGraphPage('selected')}
        >
          Selected Strategy
        </button>
      </div>

      {graphPage === 'filtered' ? (
        <div className="strategy-chart-grid">
          <div className="q">
            <StrategyRankedChart
              strategies={strategies}
              selectedStrategyId={selectedStrategyId}
              leaderChartStartIndex={leaderChartStartIndex}
              onSelectStrategy={onSelectStrategy}
              onHoverStrategy={onHoverStrategy}
            />
          </div>
          <div className="q">
            <StrategyCohortMap
              strategies={strategies}
              selectedStrategyId={selectedStrategyId}
              onSelectStrategy={onSelectStrategy}
              onHoverStrategy={onHoverStrategy}
            />
          </div>
        </div>
      ) : (
        <div className="strategy-chart-grid">
          <div className="q">
            <StrategyYearlyChart strategy={selectedStrategy} isLoading={isHydratingStrategy} />
          </div>
          <div className="q">
            <StrategyContractWeekChart
              contractWeeks={contractWeeks}
              isLoading={isLoadingContractWeeks}
            />
          </div>
        </div>
      )}
    </div>
  );
}
