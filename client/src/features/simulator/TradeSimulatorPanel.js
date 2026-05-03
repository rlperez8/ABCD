import React, { useEffect, useMemo, useState } from 'react';
import { fetchSimulatorFamilyReplay } from '../../services/patternApi';

const APEX_ACCOUNT_RULES = {
  '25K': {
    label: '25K',
    startingBalance: 25000,
    profitTarget: 1500,
    maxDrawdown: 1000,
    eodDailyLossLimit: 500,
    evaluationContracts: 4,
    paContracts: 2,
    paTiers: [
      { range: '$0 - $999', contracts: 1, dailyLossLimit: 500, tier: 'Level 1' },
      { range: '$1,000 - $1,999', contracts: 2, dailyLossLimit: 500, tier: 'Level 2' },
      { range: '$2,000 and up', contracts: 2, dailyLossLimit: 1250, tier: 'Level 3' },
    ],
  },
  '50K': {
    label: '50K',
    startingBalance: 50000,
    profitTarget: 3000,
    maxDrawdown: 2000,
    eodDailyLossLimit: 1000,
    evaluationContracts: 6,
    paContracts: 4,
    paTiers: [
      { range: '$0 - $1,499', contracts: 2, dailyLossLimit: 1000, tier: 'Level 1' },
      { range: '$1,500 - $2,999', contracts: 3, dailyLossLimit: 1000, tier: 'Level 2' },
      { range: '$3,000 - $5,999', contracts: 4, dailyLossLimit: 2000, tier: 'Level 3' },
      { range: '$6,000 and up', contracts: 4, dailyLossLimit: 3000, tier: 'Level 4' },
    ],
  },
  '100K': {
    label: '100K',
    startingBalance: 100000,
    profitTarget: 6000,
    maxDrawdown: 3000,
    eodDailyLossLimit: 1500,
    evaluationContracts: 8,
    paContracts: 6,
    paTiers: [
      { range: '$0 - $1,999', contracts: 3, dailyLossLimit: 1750, tier: 'Level 1' },
      { range: '$2,000 - $2,999', contracts: 4, dailyLossLimit: 1750, tier: 'Level 2' },
      { range: '$3,000 - $4,999', contracts: 5, dailyLossLimit: 1750, tier: 'Level 3' },
      { range: '$5,000 - $9,999', contracts: 6, dailyLossLimit: 2500, tier: 'Level 4' },
      { range: '$10,000 and up', contracts: 6, dailyLossLimit: 3500, tier: 'Level 5' },
    ],
  },
  '150K': {
    label: '150K',
    startingBalance: 150000,
    profitTarget: 9000,
    maxDrawdown: 4000,
    eodDailyLossLimit: 2000,
    evaluationContracts: 12,
    paContracts: 10,
    paTiers: [
      { range: '$0 - $1,999', contracts: 4, dailyLossLimit: 2500, tier: 'Level 1' },
      { range: '$2,000 - $2,999', contracts: 5, dailyLossLimit: 2500, tier: 'Level 2' },
      { range: '$3,000 - $4,999', contracts: 7, dailyLossLimit: 2500, tier: 'Level 3' },
      { range: '$5,000 - $9,999', contracts: 10, dailyLossLimit: 3000, tier: 'Level 4' },
      { range: '$10,000 and up', contracts: 10, dailyLossLimit: 4000, tier: 'Level 5' },
    ],
  },
};

const APEX_RULE_SOURCES = [
  {
    label: 'EOD evaluations',
    url: 'https://apextraderfunding.com/help-center/eod-trailing-drawdown-accounts/eod-evaluations/',
  },
  {
    label: 'Intraday evaluations',
    url: 'https://apextraderfunding.com/help-center/evaluation-accounts-ea/intraday-trailing-drawdown-evaluations/',
  },
  {
    label: 'Position sizing',
    url: 'https://apextraderfunding.com/help-center/additional-helpful-items/position-sizing-evaluation/',
  },
  {
    label: 'Daily loss limit',
    url: 'https://apextraderfunding.com/help-center/additional-helpful-items/daily-loss-limit-explained/',
  },
];

const SIMULATOR_TABS = [
  { id: 'overview', label: 'Simulator' },
  { id: 'details', label: 'Details' },
  { id: 'events', label: 'Event Log' },
  { id: 'rules', label: 'Rules' },
];

const SIMULATOR_PLAYBACK_INTERVAL_MS = 70;
const SIMULATOR_PLAYBACK_EVENTS_PER_TICK = 3;

const formatMoney = (value) =>
  new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(Number(value) || 0);

const formatNumber = (value, digits = 0) =>
  Number.isFinite(Number(value)) ? Number(value).toFixed(digits) : 'N/A';

const formatShortDate = (value) => {
  if (!value) {
    return 'N/A';
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return String(value).slice(0, 10);
  }

  return date.toLocaleDateString('en-US', {
    month: '2-digit',
    day: '2-digit',
    year: '2-digit',
  });
};

const getTradeDate = (trade = {}) =>
  trade.entry_date ?? trade.reversal_detect_date ?? trade.d_confirm_date ?? trade.d_date ?? null;

const getTradeEndDate = (trade = {}) => trade.target_date ?? getTradeDate(trade);

const getTradeResultLabel = (trade = {}) => {
  const result = Number(trade.trade_result);
  if (result === 1) return 'Won';
  if (result === 2) return 'Lost';
  return 'Open';
};

const getTestTone = (status = '') => {
  if (status === 'passed') return 'passed';
  if (status === 'failed') return 'failed';
  if (status === 'running') return 'running';
  return 'waiting';
};

const toTime = (value) => {
  const date = value ? new Date(value) : null;
  return date && !Number.isNaN(date.getTime()) ? date.getTime() : null;
};

const buildServerReplay = (trades = [], startingBalance = 0, maxDrawdown = 0) => {
  let previousTotal = 0;
  let peak = 0;
  const drawdownDistance = Math.abs(Number(maxDrawdown) || 0);

  const points = [
    {
      index: 0,
      trade: null,
      pnl: 0,
      previousTotal: 0,
      total: 0,
      drawdown: 0,
      drawdownLevel: -drawdownDistance,
      isStart: true,
    },
  ];

  trades
    .filter((trade) => !trade.skipped_for_overlap)
    .forEach((trade, index) => {
      const total = (Number(trade.balance) || startingBalance) - startingBalance;
      peak = Math.max(peak, total);
      const drawdownLevel = peak - drawdownDistance;
      points.push({
        index: index + 1,
        trade,
        pnl: Number(trade.pnl) || 0,
        previousTotal,
        total,
        drawdown: total - peak,
        drawdownLevel,
      });
      previousTotal = total;
    });

  return points;
};

const svgPolyline = (points, xForPoint, yForValue, valueKey) =>
  points
    .map((point) => `${xForPoint(point).toFixed(2)},${yForValue(point[valueKey]).toFixed(2)}`)
    .join(' ');

const formatChartMoney = (value) =>
  new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(Number(value) || 0);

function SimulatorChart({
  replayPoints,
  mode,
  profitTarget = null,
  maxDrawdown = null,
  showThresholds = false,
  totalPointCount = null,
}) {
  const width = 1000;
  const height = 330;
  const plot = { left: 104, right: 34, top: 30, bottom: 48 };
  const totalMode = mode === 'total';
  const drawdownDistance = Math.abs(Number(maxDrawdown) || 0);
  const targetLine = totalMode && showThresholds && Number.isFinite(Number(profitTarget))
    ? Math.abs(Number(profitTarget))
    : null;
  const points = (replayPoints.length ? replayPoints : []).map((point) => ({
    ...point,
    total: Number(point.total) || 0,
    pnl: Number(point.pnl) || 0,
    previousTotal: Number(point.previousTotal) || 0,
    drawdownLevel: Number.isFinite(Number(point.drawdownLevel))
      ? Number(point.drawdownLevel)
      : -drawdownDistance,
  }));
  const chartPoints = points.length
    ? points
    : [{ index: 0, total: 0, pnl: 0, previousTotal: 0, drawdownLevel: -drawdownDistance, isStart: true }];
  const valueKey = totalMode ? 'total' : 'pnl';
  const tradePoints = chartPoints.filter((point) => !point.isStart);
  const plotWidth = width - plot.left - plot.right;
  const plotHeight = height - plot.top - plot.bottom;
  const xSpan = Math.max(Number(totalPointCount) || chartPoints.length - 1, 1);
  const values = chartPoints.flatMap((point) =>
    totalMode ? [point.total, point.drawdownLevel] : [point.pnl]
  );
  values.push(0);
  if (targetLine !== null) values.push(targetLine);
  if (totalMode && drawdownDistance) values.push(-drawdownDistance);
  const rawMin = Math.min(...values);
  const rawMax = Math.max(...values);
  const rawSpan = rawMax === rawMin ? Math.max(Math.abs(rawMax), 1) : rawMax - rawMin;
  const minValue = rawMin - rawSpan * 0.12;
  const maxValue = rawMax + rawSpan * 0.12;
  const valueSpan = maxValue - minValue || 1;
  const xForPoint = (point) => plot.left + (point.index / xSpan) * plotWidth;
  const yForValue = (value) => plot.top + ((maxValue - value) / valueSpan) * plotHeight;
  const pnlLine = svgPolyline(chartPoints, xForPoint, yForValue, valueKey);
  const drawdownLine = totalMode
    ? svgPolyline(chartPoints, xForPoint, yForValue, 'drawdownLevel')
    : '';
  const zeroY = yForValue(0);
  const targetY = targetLine !== null ? yForValue(targetLine) : null;
  const currentPoint = tradePoints[tradePoints.length - 1] ?? chartPoints[0];
  const currentDrawdown = totalMode ? currentPoint.drawdownLevel : null;
  const currentDrawdownY = currentDrawdown !== null ? yForValue(currentDrawdown) : null;
  const gridValues = Array.from({ length: 5 }, (_, index) =>
    maxValue - (valueSpan / 4) * index
  );
  const labelX = 8;
  const labelWidth = 88;
  const labelHeight = 24;
  const renderLevelPill = (value, label, className) => {
    const y = yForValue(value);
    return (
      <g className={`simulator-live-chart__level ${className}`}>
        <rect x={labelX} y={y - labelHeight / 2} width={labelWidth} height={labelHeight} rx="3" />
        <text x={labelX + labelWidth - 8} y={y + 4} textAnchor="end">
          {label} {formatChartMoney(value)}
        </text>
      </g>
    );
  };

  return (
    <div className="simulator-chart-shell">
      <svg
        className="simulator-live-chart"
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label="Test 1 PnL with moving trailing drawdown"
      >
        <defs>
          <linearGradient id="simulator-live-chart-bg" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="#111923" />
            <stop offset="100%" stopColor="#06080c" />
          </linearGradient>
          <linearGradient id="simulator-live-chart-profit-fill" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="#39c8ff" stopOpacity="0.2" />
            <stop offset="100%" stopColor="#39c8ff" stopOpacity="0.015" />
          </linearGradient>
        </defs>

        <rect
          x={plot.left}
          y={plot.top}
          width={plotWidth}
          height={plotHeight}
          className="simulator-live-chart__plot"
        />
        <g className="simulator-live-chart__grid">
          {gridValues.map((value) => {
            const y = yForValue(value);
            return (
              <g key={`grid-${value.toFixed(2)}`}>
                <line x1={plot.left} y1={y} x2={width - plot.right} y2={y} />
                <text x={plot.left - 12} y={y + 4} textAnchor="end">
                  {formatChartMoney(value)}
                </text>
              </g>
            );
          })}
        </g>

        {targetLine !== null ? (
          <line
            x1={plot.left}
            y1={targetY}
            x2={width - plot.right}
            y2={targetY}
            className="simulator-live-chart__target"
          />
        ) : null}
        <line
          x1={plot.left}
          y1={zeroY}
          x2={width - plot.right}
          y2={zeroY}
          className="simulator-live-chart__zero"
        />
        {targetLine !== null ? renderLevelPill(targetLine, 'TP', 'simulator-live-chart__level--target') : null}
        {renderLevelPill(0, 'Start', 'simulator-live-chart__level--start')}
        {totalMode && currentDrawdown !== null
          ? renderLevelPill(currentDrawdown, 'Trail', 'simulator-live-chart__level--drawdown')
          : null}

        <polyline points={pnlLine} className="simulator-live-chart__pnl-fill" />
        {totalMode ? (
          <polyline points={drawdownLine} className="simulator-live-chart__drawdown" />
        ) : null}
        <polyline points={pnlLine} className="simulator-live-chart__pnl" />

        {tradePoints.map((point) => {
          const pointX = xForPoint(point);
          const previousY = yForValue(totalMode ? point.previousTotal : 0);
          const pointY = yForValue(point[valueKey]);
          const stemTop = Math.min(previousY, pointY);
          const stemBottom = Math.max(previousY, pointY);
          const tooltipWidth = 226;
          const tooltipHeight = totalMode ? 94 : 78;
          const tooltipX = Math.min(pointX + 12, width - plot.right - tooltipWidth);
          const tooltipY = Math.max(plot.top + 8, stemTop - tooltipHeight - 10);
          const result = getTradeResultLabel(point.trade);
          const symbol = point.trade?.symbol ?? `Trade ${point.index}`;
          const exit = formatShortDate(point.trade?.target_date);
          const tradeTone = point.pnl >= 0 ? 'win' : 'loss';

          return (
            <g
              className={`simulator-live-chart__trade simulator-live-chart__trade--${tradeTone}`}
              key={`${point.index}-${point.total}-${point.pnl}`}
            >
              <line
                className="simulator-live-chart__trade-hit"
                x1={pointX}
                y1={plot.top}
                x2={pointX}
                y2={height - plot.bottom}
              />
              <line
                className="simulator-live-chart__trade-stem"
                x1={pointX}
                y1={stemTop}
                x2={pointX}
                y2={stemBottom}
              />
              <circle className="simulator-live-chart__trade-ring" cx={pointX} cy={pointY} r="9" />
              <circle className="simulator-live-chart__trade-core" cx={pointX} cy={pointY} r="4.5" />
              <g className="simulator-live-chart__tooltip">
                <rect x={tooltipX} y={tooltipY} width={tooltipWidth} height={tooltipHeight} rx="5" />
                <text x={tooltipX + 10} y={tooltipY + 18}>{symbol} / Trade {point.index}</text>
                <text x={tooltipX + 10} y={tooltipY + 36}>Result {result} / {formatChartMoney(point.pnl)}</text>
                <text x={tooltipX + 10} y={tooltipY + 54}>Test P/L {formatChartMoney(point.total)}</text>
                {totalMode ? (
                  <text x={tooltipX + 10} y={tooltipY + 72}>Trailing DD {formatChartMoney(point.drawdownLevel)}</text>
                ) : null}
                <text x={tooltipX + 10} y={tooltipY + (totalMode ? 88 : 70)}>Exit {exit}</text>
              </g>
            </g>
          );
        })}

        {currentPoint ? (
          <>
            <circle
              className="simulator-live-chart__current-pulse"
              cx={xForPoint(currentPoint)}
              cy={yForValue(currentPoint[valueKey])}
              r="12"
            />
            {totalMode && currentDrawdownY !== null ? (
              <circle
                className="simulator-live-chart__drawdown-head"
                cx={xForPoint(currentPoint)}
                cy={currentDrawdownY}
                r="5"
              />
            ) : null}
          </>
        ) : null}

        <text x={plot.left} y={height - 16} className="simulator-live-chart__caption">
          Each dot is one completed trade in the selected test. The red trail rises only after new P/L highs.
        </text>
      </svg>
    </div>
  );
}

const RuleTable = ({ rows }) => (
  <div className="simulator-rule-table">
    {rows.map((row) => (
      <div className="simulator-rule-row" key={row.label}>
        <span>{row.label}</span>
        <strong>{row.value}</strong>
      </div>
    ))}
  </div>
);

const SelectedFamilySnapshot = ({ familyId, selectedStrategy }) => (
  <div className="simulator-family-snapshot">
    <div className="simulator-family-snapshot-main">
      <span>Family ID</span>
      <strong>{familyId || 'None selected'}</strong>
    </div>
    <div className="simulator-family-snapshot-grid">
      <div>
        <span>Market</span>
        <strong>{selectedStrategy?.market ?? 'N/A'}</strong>
      </div>
      <div>
        <span>Harmonic</span>
        <strong>{selectedStrategy?.harmonicType ?? 'N/A'}</strong>
      </div>
      <div>
        <span>Price Ratio</span>
        <strong>{selectedStrategy?.bin ?? 'N/A'}</strong>
      </div>
      <div>
        <span>Time Ratio</span>
        <strong>{selectedStrategy?.timeBin ?? 'N/A'}</strong>
      </div>
      <div>
        <span>Size</span>
        <strong>{selectedStrategy?.sizeBucket ?? 'N/A'}</strong>
      </div>
      <div>
        <span>Reversal</span>
        <strong>{selectedStrategy?.reversalType ?? 'None'}</strong>
      </div>
    </div>
  </div>
);

const EvaluationSnapshot = ({ accountRules, drawdownModel }) => {
  const dailyLossText =
    drawdownModel === 'eod'
      ? formatMoney(accountRules.eodDailyLossLimit)
      : 'None';

  return (
    <div className="simulator-evaluation-snapshot">
      <div className="simulator-evaluation-hero">
        <div>
          <span>Account</span>
          <strong>{accountRules.label}</strong>
        </div>
        <div>
          <span>Starting Balance</span>
          <strong>{formatMoney(accountRules.startingBalance)}</strong>
        </div>
      </div>
      <div className="simulator-evaluation-rules">
        <div className="simulator-evaluation-rule simulator-evaluation-rule--target">
          <span>Profit Target</span>
          <strong>{formatMoney(accountRules.profitTarget)}</strong>
        </div>
        <div className="simulator-evaluation-rule simulator-evaluation-rule--drawdown">
          <span>{drawdownModel === 'eod' ? 'EOD Drawdown' : 'Intraday Drawdown'}</span>
          <strong>{formatMoney(accountRules.maxDrawdown)}</strong>
        </div>
        <div className="simulator-evaluation-rule">
          <span>Daily Loss</span>
          <strong>{dailyLossText}</strong>
        </div>
        <div className="simulator-evaluation-rule">
          <span>Max Contracts</span>
          <strong>{accountRules.evaluationContracts}</strong>
        </div>
        <div className="simulator-evaluation-rule">
          <span>Access Period</span>
          <strong>30 days</strong>
        </div>
        <div className="simulator-evaluation-rule">
          <span>Min Trading Days</span>
          <strong>None</strong>
        </div>
      </div>
    </div>
  );
};

const TradeSimulatorPanel = ({
  selectedStrategy = null,
  loadedTrades = [],
  totalTradeCount = 0,
}) => {
  const [activeTab, setActiveTab] = useState('overview');
  const [familyId, setFamilyId] = useState('');
  const [firstStartDate, setFirstStartDate] = useState('2021-04-26');
  const [testsToChain, setTestsToChain] = useState(1);
  const [contractsPerTrade, setContractsPerTrade] = useState(1);
  const [oneTradeAtATime, setOneTradeAtATime] = useState(true);
  const [accountSize, setAccountSize] = useState('50K');
  const [drawdownModel, setDrawdownModel] = useState('intraday');
  const [pnlMode, setPnlMode] = useState('total');
  const [overviewPanel, setOverviewPanel] = useState('trades');
  const [simulatorReplay, setSimulatorReplay] = useState(null);
  const [selectedReplayTestIndex, setSelectedReplayTestIndex] = useState(1);
  const [playbackEventCount, setPlaybackEventCount] = useState(0);
  const [isPlaybackRunning, setPlaybackRunning] = useState(false);
  const [isRunningReplay, setRunningReplay] = useState(false);
  const [replayError, setReplayError] = useState('');

  const selectedFamilyId = selectedStrategy?.propStrategyId ?? selectedStrategy?.id ?? '';

  useEffect(() => {
    setFamilyId(selectedFamilyId);
  }, [selectedFamilyId]);

  useEffect(() => {
    setActiveTab('overview');
    setPnlMode('total');
    setOverviewPanel('trades');
    setTestsToChain(1);
    setSimulatorReplay(null);
    setSelectedReplayTestIndex(1);
    setPlaybackEventCount(0);
    setPlaybackRunning(false);
  }, [selectedFamilyId]);

  useEffect(() => {
    if (!simulatorReplay?.trades?.length || !isPlaybackRunning) {
      return undefined;
    }

    const timer = window.setInterval(() => {
      setPlaybackEventCount((current) => {
        const next = Math.min(
          current + SIMULATOR_PLAYBACK_EVENTS_PER_TICK,
          simulatorReplay.trades.length
        );
        if (next >= simulatorReplay.trades.length) {
          setPlaybackRunning(false);
        }
        return next;
      });
    }, SIMULATOR_PLAYBACK_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [isPlaybackRunning, simulatorReplay]);

  const accountRules = APEX_ACCOUNT_RULES[accountSize] ?? APEX_ACCOUNT_RULES['50K'];
  const contracts = Math.max(1, Number.parseInt(String(contractsPerTrade), 10) || 1);
  const dailyLossLimit = drawdownModel === 'eod' ? accountRules.eodDailyLossLimit : null;

  const visibleReplayTrades = useMemo(
    () => (simulatorReplay?.trades ?? []).slice(0, playbackEventCount),
    [playbackEventCount, simulatorReplay]
  );
  const visibleSelectedReplayTrades = useMemo(
    () => visibleReplayTrades.filter((trade) => trade.test_index === selectedReplayTestIndex),
    [selectedReplayTestIndex, visibleReplayTrades]
  );

  const testReplayPoints = useMemo(
    () => buildServerReplay(visibleSelectedReplayTrades, accountRules.startingBalance, accountRules.maxDrawdown),
    [accountRules.maxDrawdown, accountRules.startingBalance, visibleSelectedReplayTrades]
  );

  const hasSimulatorReplay = Boolean(simulatorReplay);
  const isReplayComplete = hasSimulatorReplay
    ? playbackEventCount >= (simulatorReplay?.trades?.length ?? 0)
    : false;

  const replayTestProgress = useMemo(() => {
    if (!simulatorReplay?.tests?.length) {
      return [];
    }

    return simulatorReplay.tests.map((test) => {
      const testEvents = simulatorReplay.trades.filter(
        (trade) => trade.test_index === test.test_index
      );
      const visibleTestEvents = visibleReplayTrades.filter(
        (trade) => trade.test_index === test.test_index
      );
      const totalEvents = testEvents.length;
      const visibleEvents = visibleTestEvents.length;
      const totalTrades = testEvents.filter((trade) => !trade.skipped_for_overlap).length;
      const visibleTrades = visibleTestEvents.filter((trade) => !trade.skipped_for_overlap).length;
      const totalSkipped = testEvents.filter((trade) => trade.skipped_for_overlap).length;
      const visibleSkipped = visibleTestEvents.filter((trade) => trade.skipped_for_overlap).length;
      const status =
        totalEvents > 0 && visibleEvents >= totalEvents
          ? test.status
          : visibleEvents > 0
          ? 'running'
          : 'waiting';

      return {
        ...test,
        status,
        visibleEvents,
        totalEvents,
        visibleTrades,
        totalTrades,
        visibleSkipped,
        totalSkipped,
      };
    });
  }, [simulatorReplay, visibleReplayTrades]);

  const replaySummary = useMemo(() => {
    const tradePoints = testReplayPoints.filter((point) => !point.isStart);
    const last = testReplayPoints[testReplayPoints.length - 1] ?? null;
    const wins = tradePoints.filter((point) => point.pnl > 0).length;
    const losses = tradePoints.filter((point) => point.pnl < 0).length;
    const maxDrawdown = testReplayPoints.reduce(
      (current, point) => Math.min(current, point.drawdown),
      0
    );

    return {
      total: last?.total ?? 0,
      wins,
      losses,
      maxDrawdown,
      trades: tradePoints.length,
    };
  }, [testReplayPoints]);
  const displayedFamilyRows =
    Number.isFinite(totalTradeCount) && totalTradeCount >= 0
      ? totalTradeCount
      : loadedTrades.length;
  const visibleReplayTakenTrades = visibleReplayTrades.filter((trade) => !trade.skipped_for_overlap).length;
  const totalReplayTakenTrades = simulatorReplay?.trades?.filter((trade) => !trade.skipped_for_overlap).length ?? 0;

  const replayRunLabel = useMemo(() => {
    if (!hasSimulatorReplay) {
      return 'Waiting';
    }

    if (isPlaybackRunning) {
      return `Live ${visibleReplayTakenTrades}/${totalReplayTakenTrades} trades`;
    }

    return isReplayComplete ? 'Complete' : 'Paused';
  }, [hasSimulatorReplay, isPlaybackRunning, isReplayComplete, totalReplayTakenTrades, visibleReplayTakenTrades]);

  const replayStatusCounts = useMemo(
    () => ({
      passed: replayTestProgress.filter((test) => test.status === 'passed').length,
      failed: replayTestProgress.filter((test) => test.status === 'failed').length,
      running: replayTestProgress.filter((test) => test.status === 'running').length,
    }),
    [replayTestProgress]
  );
  const selectedReplayTest = useMemo(
    () =>
      replayTestProgress.find((test) => test.test_index === selectedReplayTestIndex) ??
      replayTestProgress[0] ??
      null,
    [replayTestProgress, selectedReplayTestIndex]
  );
  const selectedReplayTestTrades = useMemo(() => {
    if (!selectedReplayTest || !visibleSelectedReplayTrades.length) {
      return [];
    }

    return visibleSelectedReplayTrades.filter((trade) => !trade.skipped_for_overlap);
  }, [selectedReplayTest, visibleSelectedReplayTrades]);
  const selectedReplayTestSkipped = useMemo(() => {
    if (!selectedReplayTest || !visibleSelectedReplayTrades.length) {
      return 0;
    }

    return visibleSelectedReplayTrades.filter((trade) => trade.skipped_for_overlap).length;
  }, [selectedReplayTest, visibleSelectedReplayTrades]);
  const selectedReplayTestNetPnl = selectedReplayTest
    ? (Number(selectedReplayTest.ending_balance) || 0) -
      (Number(selectedReplayTest.starting_balance) || 0)
    : 0;
  const restartReplayPlayback = () => {
    if (!simulatorReplay?.trades?.length) {
      return;
    }

    setPlaybackEventCount(0);
    setPlaybackRunning(true);
  };

  const toggleReplayPlayback = () => {
    if (!simulatorReplay?.trades?.length) {
      return;
    }

    if (isReplayComplete) {
      restartReplayPlayback();
      return;
    }

    setPlaybackRunning((current) => !current);
  };

  const timelineRows = useMemo(
    () => {
      if (!simulatorReplay?.trades?.length) {
        return [];
      }

      const sourceRows = visibleReplayTrades;
      return sourceRows.slice(0, 32).map((trade, index) => {
        const start = getTradeDate(trade);
        const end = getTradeEndDate(trade);
        const startTime = toTime(start);
        const endTime = toTime(end) ?? startTime;
        const overlaps =
          startTime === null || endTime === null
            ? 0
            : sourceRows.filter((candidate, candidateIndex) => {
                if (candidateIndex === index) {
                  return false;
                }

                const candidateTime = toTime(getTradeDate(candidate));
                return candidateTime !== null && candidateTime > startTime && candidateTime < endTime;
              }).length;

        return {
          trade,
          start,
          end,
          overlaps,
        };
      });
    },
    [simulatorReplay, visibleReplayTrades]
  );

  const runReplay = async () => {
    if (!familyId || isRunningReplay) {
      return;
    }

    const parsedTestsToChain = Math.max(
      1,
      Math.min(250, Number.parseInt(String(testsToChain), 10) || 1)
    );

    setRunningReplay(true);
    setReplayError('');
    const result = await fetchSimulatorFamilyReplay({
      familyId,
      firstStartDate,
      testsToChain: parsedTestsToChain,
      contracts,
      accountRules: {
        ...accountRules,
        dailyLossLimit,
      },
      oneTradeAtATime,
    });

    setRunningReplay(false);
    if (!result) {
      setReplayError('Replay failed. Check that the server is running and the family has rows.');
      return;
    }

    setSimulatorReplay(result);
    setSelectedReplayTestIndex(1);
    setPlaybackEventCount(result.trades?.length ?? 0);
    setPlaybackRunning(false);
    setPnlMode('total');
    setActiveTab('overview');
  };

  const evaluationRows = [
    { label: 'Starting Balance', value: formatMoney(accountRules.startingBalance) },
    { label: 'Profit Target', value: formatMoney(accountRules.profitTarget) },
    {
      label: drawdownModel === 'eod' ? 'Max Drawdown EOD' : 'Max Drawdown Intraday',
      value: formatMoney(accountRules.maxDrawdown),
    },
    {
      label: 'Daily Loss Limit',
      value: drawdownModel === 'eod' ? formatMoney(accountRules.eodDailyLossLimit) : 'None in evaluation',
    },
    { label: 'Max Contracts', value: accountRules.evaluationContracts },
    { label: 'Access Period', value: '30 days' },
    { label: 'Minimum Trading Days', value: 'None' },
  ];

  return (
    <div className="simulator-panel">
      <div className="simulator-topbar">
        <div className="simulator-title-block">
          <div className="simulator-kicker">Trade Simulator</div>
          <div className="simulator-title">{selectedStrategy?.familyName ?? selectedStrategy?.name ?? 'Family Replay'}</div>
        </div>
        <div className="simulator-status-strip">
          <span className="simulator-status-badge simulator-status-badge--passed">
            {hasSimulatorReplay ? `${replayStatusCounts.passed} Passed` : 'Passed'}
          </span>
          <span className="simulator-status-badge simulator-status-badge--failed">
            {hasSimulatorReplay ? `${replayStatusCounts.failed} Failed` : 'Failed'}
          </span>
          <span className="simulator-status-badge">
            {hasSimulatorReplay ? replayRunLabel : 'Waiting'}
          </span>
        </div>
      </div>

      <div className="simulator-tabs">
        {SIMULATOR_TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            className={activeTab === tab.id ? 'simulator-tab simulator-tab--active' : 'simulator-tab'}
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div className="simulator-body">
        <aside className="simulator-config-rail simulator-config-rail--compact">
          <div className="simulator-rail-card simulator-rail-card--hero">
            <div className="simulator-rail-card-head">
              <span>Setup</span>
              <strong>{selectedStrategy?.market ?? 'Family'}</strong>
            </div>
            <label className="simulator-field simulator-field--wide">
              <span>Strategy Family</span>
              <input
                value={familyId}
                onChange={(event) => setFamilyId(event.target.value)}
                spellCheck="false"
              />
            </label>
            <button type="button" className="simulator-secondary-button" onClick={() => setFamilyId(selectedFamilyId)}>
              Use Selected
            </button>
            <div className="simulator-rail-mini-grid">
              <span>{selectedStrategy?.harmonicType ?? 'Pattern'}</span>
              <span>{selectedStrategy?.bin ?? 'Price Bin'}</span>
              <span>{selectedStrategy?.timeBin ?? 'Time Bin'}</span>
            </div>
          </div>

          <div className="simulator-rail-card">
            <div className="simulator-rail-card-head">
              <span>Run</span>
              <strong>{testsToChain} {Number(testsToChain) === 1 ? 'Test' : 'Tests'}</strong>
            </div>
            <label className="simulator-field">
              <span>First Start</span>
              <input
                type="date"
                value={firstStartDate}
                onChange={(event) => setFirstStartDate(event.target.value)}
              />
            </label>

            <div className="simulator-field-row">
              <label className="simulator-field">
                <span>Tests</span>
                <input
                  type="number"
                  min="1"
                  max="250"
                  value={testsToChain}
                  onChange={(event) => setTestsToChain(event.target.value)}
                />
              </label>
              <label className="simulator-field">
                <span>Contracts</span>
                <input
                  type="number"
                  min="1"
                  max={accountRules.evaluationContracts}
                  value={contractsPerTrade}
                  onChange={(event) => setContractsPerTrade(event.target.value)}
                />
              </label>
            </div>

            <label className="simulator-check simulator-check--switch">
              <input
                type="checkbox"
                checked={oneTradeAtATime}
                onChange={(event) => setOneTradeAtATime(event.target.checked)}
              />
              <span>Skip Overlapping Setups</span>
            </label>
          </div>

          <div className="simulator-rail-card simulator-rail-card--rules">
            <div className="simulator-rail-card-head">
              <span>Account</span>
              <strong>{accountRules.label}</strong>
            </div>
            <div className="simulator-account-pills">
              {Object.keys(APEX_ACCOUNT_RULES).map((key) => (
                <button
                  type="button"
                  className={accountSize === key ? 'simulator-account-pill simulator-account-pill--active' : 'simulator-account-pill'}
                  onClick={() => setAccountSize(key)}
                  key={key}
                >
                  {key}
                </button>
              ))}
            </div>

            <div className="simulator-drawdown-toggle">
              <button
                type="button"
                className={drawdownModel === 'intraday' ? 'simulator-drawdown-option simulator-drawdown-option--active' : 'simulator-drawdown-option'}
                onClick={() => setDrawdownModel('intraday')}
              >
                Intraday
              </button>
              <button
                type="button"
                className={drawdownModel === 'eod' ? 'simulator-drawdown-option simulator-drawdown-option--active' : 'simulator-drawdown-option'}
                onClick={() => setDrawdownModel('eod')}
              >
                EOD
              </button>
            </div>

            <div className="simulator-rule-stack">
              <div>
                <span>Target</span>
                <strong>{formatMoney(accountRules.profitTarget)}</strong>
              </div>
              <div>
                <span>Max Down</span>
                <strong>{formatMoney(accountRules.maxDrawdown)}</strong>
              </div>
              <div>
                <span>Balance</span>
                <strong>{formatMoney(accountRules.startingBalance)}</strong>
              </div>
              <div>
                <span>Daily Loss</span>
                <strong>{drawdownModel === 'eod' ? formatMoney(accountRules.eodDailyLossLimit) : 'None'}</strong>
              </div>
            </div>
          </div>

          <button
            type="button"
            className="simulator-primary-button"
            onClick={runReplay}
            disabled={!familyId || isRunningReplay}
          >
            {isRunningReplay
              ? `Running ${Number(testsToChain) === 1 ? 'Test' : 'Tests'}...`
              : `Run ${Number(testsToChain) === 1 ? 'Test' : 'Tests'}`}
          </button>

          <div className="simulator-rail-summary simulator-rail-summary--compact">
            <span>{displayedFamilyRows.toLocaleString()} rows</span>
            <span>{replayRunLabel}</span>
          </div>
          {replayError ? <div className="simulator-error-state">{replayError}</div> : null}
        </aside>

        <section className="simulator-workspace">

          {activeTab === 'details' ? (
            <div className="simulator-setup-grid">
              <div className="simulator-panel-block">
                <div className="simulator-section-title">Selected Family</div>
                <SelectedFamilySnapshot familyId={familyId} selectedStrategy={selectedStrategy} />
              </div>

              <div className="simulator-panel-block">
                <div className="simulator-section-title">Execution Timing</div>
                <div className="simulator-step-list">
                  <span>D pivot confirms after the next candle closes.</span>
                  <span>Entry is the following candle.</span>
                  <span>Target remains the actual C level.</span>
                  <span>Stop mirrors entry-to-C distance.</span>
                  <span>Skip setup if price touches C before entry.</span>
                </div>
              </div>

              <div className="simulator-panel-block simulator-panel-block--wide">
                <div className="simulator-section-title">Evaluation Snapshot</div>
                <EvaluationSnapshot accountRules={accountRules} drawdownModel={drawdownModel} />
              </div>
            </div>
          ) : null}

          {activeTab === 'rules' ? (
            <div className="simulator-rules-grid">
              <div className="simulator-panel-block">
                <div className="simulator-section-title">Apex Evaluation</div>
                <RuleTable rows={evaluationRows} />
              </div>

              <div className="simulator-panel-block">
                <div className="simulator-section-title">PA Scaling - {accountRules.label}</div>
                <div className="simulator-tier-table">
                  <div className="simulator-tier-head">
                    <span>Profit</span>
                    <span>Contracts</span>
                    <span>DLL</span>
                    <span>Tier</span>
                  </div>
                  {accountRules.paTiers.map((tier) => (
                    <div className="simulator-tier-row" key={tier.tier}>
                      <span>{tier.range}</span>
                      <span>{tier.contracts}</span>
                      <span>{formatMoney(tier.dailyLossLimit)}</span>
                      <span>{tier.tier}</span>
                    </div>
                  ))}
                </div>
              </div>

              <div className="simulator-panel-block simulator-panel-block--wide">
                <div className="simulator-section-title">Official References</div>
                <div className="simulator-source-list">
                  <span>Verified May 2, 2026</span>
                  {APEX_RULE_SOURCES.map((source) => (
                    <a href={source.url} target="_blank" rel="noreferrer" key={source.url}>
                      {source.label}
                    </a>
                  ))}
                </div>
              </div>
            </div>
          ) : null}

          {activeTab === 'overview' ? (
            <div className="simulator-overview-grid">
              <div className={`simulator-result-summary simulator-result-summary--compact simulator-result-summary--${selectedReplayTest ? getTestTone(selectedReplayTest.status) : 'waiting'}`}>
                <div className="simulator-result-summary-main">
                  <span>Selected Test</span>
                  <strong>{selectedReplayTest ? `Test ${selectedReplayTest.test_index} ${selectedReplayTest.status}` : 'Ready To Run'}</strong>
                  <small>
                    {selectedReplayTest
                      ? `${formatShortDate(selectedReplayTest.start_date)} to ${formatShortDate(selectedReplayTest.end_date)}`
                      : `${accountRules.label} account / ${Number(testsToChain) || 1} ${Number(testsToChain) === 1 ? 'test' : 'tests'}`}
                  </small>
                </div>

                <div className="simulator-result-summary-grid">
                  <div>
                    <span>Net P/L</span>
                    <strong className={selectedReplayTestNetPnl >= 0 ? 'simulator-value-positive' : 'simulator-value-negative'}>
                      {formatMoney(selectedReplayTestNetPnl)}
                    </strong>
                  </div>
                  <div>
                    <span>Ending Balance</span>
                    <strong>{formatMoney(selectedReplayTest?.ending_balance ?? accountRules.startingBalance)}</strong>
                  </div>
                  <div>
                    <span>Taken Trades</span>
                    <strong>{selectedReplayTest?.visibleTrades ?? 0}/{selectedReplayTest?.totalTrades ?? 0}</strong>
                  </div>
                  <div>
                    <span>Skipped</span>
                    <strong>{selectedReplayTestSkipped}</strong>
                  </div>
                  <div>
                    <span>Max Drawdown</span>
                    <strong>{formatMoney(selectedReplayTest?.max_drawdown ?? 0)}</strong>
                  </div>
                </div>
              </div>

              <div className="simulator-panel-block simulator-chart-panel">
                <div className="simulator-result-header">
                  <div className="simulator-chart-title-block">
                    <div className="simulator-section-title">
                      Test {selectedReplayTest?.test_index ?? 1} PnL Replay
                    </div>
                    <div className="simulator-chart-subtitle">
                      {hasSimulatorReplay
                        ? `${replaySummary.trades}/${selectedReplayTest?.totalTrades ?? 0} trades shown`
                        : 'Run the test replay to reveal each trade on the graph'}
                    </div>
                  </div>
                  <div className="simulator-toggle-group">
                    <button
                      type="button"
                      className={pnlMode === 'trade' ? 'simulator-toggle simulator-toggle--active' : 'simulator-toggle'}
                      onClick={() => setPnlMode('trade')}
                    >
                      Trade PnL
                    </button>
                    <button
                      type="button"
                      className={pnlMode === 'total' ? 'simulator-toggle simulator-toggle--active' : 'simulator-toggle'}
                      onClick={() => setPnlMode('total')}
                    >
                      Total PnL
                    </button>
                    {hasSimulatorReplay ? (
                      <>
                        <button
                          type="button"
                          className={isPlaybackRunning ? 'simulator-toggle simulator-toggle--active' : 'simulator-toggle'}
                          onClick={toggleReplayPlayback}
                        >
                          {isPlaybackRunning ? 'Pause Tests' : isReplayComplete ? 'Replay Tests' : 'Play Tests'}
                        </button>
                        <button
                          type="button"
                          className="simulator-toggle"
                          onClick={restartReplayPlayback}
                        >
                          Restart Tests
                        </button>
                      </>
                    ) : null}
                  </div>
                </div>
                <SimulatorChart
                  replayPoints={testReplayPoints}
                  mode={pnlMode}
                  profitTarget={accountRules.profitTarget}
                  maxDrawdown={accountRules.maxDrawdown}
                  showThresholds={pnlMode === 'total'}
                  totalPointCount={Math.max(selectedReplayTest?.totalTrades ?? 0, 1)}
                />
              </div>

              <div className="simulator-overview-tests">
                <div className="simulator-inspector-head">
                  <div>
                    <div className="simulator-section-title">Tests</div>
                    <span>{hasSimulatorReplay ? `${replayTestProgress.length} loaded` : 'Waiting'}</span>
                  </div>
                </div>
                {hasSimulatorReplay ? (
                  <div className="simulator-test-strip simulator-test-strip--inline">
                    {replayTestProgress.map((test) => {
                      const tone = getTestTone(test.status);
                      const isSelected = selectedReplayTest?.test_index === test.test_index;
                      return (
                        <button
                          type="button"
                          className={`simulator-test-card simulator-test-card--${tone}${isSelected ? ' simulator-test-card--selected' : ''}`}
                          key={test.test_index}
                          onClick={() => setSelectedReplayTestIndex(test.test_index)}
                        >
                          <span>Test {test.test_index}</span>
                          <strong>{test.status}</strong>
                          <small>
                            {test.visibleTrades}/{test.totalTrades || 0} trades
                          </small>
                          {test.totalSkipped ? <small>{test.visibleSkipped}/{test.totalSkipped} skipped</small> : null}
                        </button>
                      );
                    })}
                  </div>
                ) : (
                  <div className="simulator-empty-state simulator-empty-state--compact">
                    Run a test to show the pass/fail cards.
                  </div>
                )}
              </div>

              <div className="simulator-overview-inspector">
                <div className="simulator-inspector-head">
                  <div className="simulator-section-title">Replay Inspector</div>
                  <div className="simulator-toggle-group">
                    <button
                      type="button"
                      className={overviewPanel === 'trades' ? 'simulator-toggle simulator-toggle--active' : 'simulator-toggle'}
                      onClick={() => setOverviewPanel('trades')}
                    >
                      Trades
                    </button>
                    <button
                      type="button"
                      className={overviewPanel === 'summary' ? 'simulator-toggle simulator-toggle--active' : 'simulator-toggle'}
                      onClick={() => setOverviewPanel('summary')}
                    >
                      Summary
                    </button>
                  </div>
                </div>

                {overviewPanel === 'summary' ? (
                  selectedReplayTest ? (
                    <div className="simulator-selected-test-layout">
                      <div className="simulator-selected-test-summary">
                        <div className={selectedReplayTestNetPnl >= 0 ? 'simulator-selected-test-total simulator-selected-test-total--positive' : 'simulator-selected-test-total simulator-selected-test-total--negative'}>
                          <span>Net P/L</span>
                          <strong>{formatMoney(selectedReplayTestNetPnl)}</strong>
                        </div>
                        <div className="simulator-selected-test-facts">
                          <div>
                            <span>Progress</span>
                            <strong>{selectedReplayTest.visibleTrades}/{selectedReplayTest.totalTrades || 0}</strong>
                          </div>
                          <div>
                            <span>Trades</span>
                            <strong>{selectedReplayTest.trade_count}</strong>
                          </div>
                          <div>
                            <span>Skipped</span>
                            <strong>{selectedReplayTestSkipped}</strong>
                          </div>
                          <div>
                            <span>Max DD</span>
                            <strong>{formatMoney(selectedReplayTest.max_drawdown)}</strong>
                          </div>
                          <div>
                            <span>Start</span>
                            <strong>{formatShortDate(selectedReplayTest.start_date)}</strong>
                          </div>
                          <div>
                            <span>Finish</span>
                            <strong>{formatShortDate(selectedReplayTest.end_date)}</strong>
                          </div>
                        </div>
                      </div>
                    </div>
                  ) : (
                    <div className="simulator-empty-state simulator-empty-state--compact">
                      Run a test to see the result summary.
                    </div>
                  )
                ) : null}

                {overviewPanel !== 'summary' ? (
                  <div className="simulator-selected-trades simulator-selected-trades--inline">
                    <div className="simulator-selected-trades-head">
                      <span>Trades It Took</span>
                      <strong>{selectedReplayTestTrades.length}</strong>
                    </div>
                    {selectedReplayTestTrades.length ? (
                      <div className="simulator-selected-trade-list">
                        {selectedReplayTestTrades.map((trade, index) => {
                          const isWin = Number(trade.pnl) >= 0;
                          return (
                            <div
                              className={isWin ? 'simulator-selected-trade simulator-selected-trade--win' : 'simulator-selected-trade simulator-selected-trade--loss'}
                              key={`${trade.pattern_group_id}-${trade.entry_date}-${index}`}
                            >
                              <div className="simulator-selected-trade-index">
                                <span>#{index + 1}</span>
                                <strong>{getTradeResultLabel(trade)}</strong>
                              </div>
                              <div className="simulator-selected-trade-main">
                                <strong>{trade.symbol ?? 'N/A'}</strong>
                                <span>{formatShortDate(trade.entry_date)} to {formatShortDate(trade.target_date)}</span>
                              </div>
                              <div className="simulator-selected-trade-values">
                                <strong>{formatMoney(trade.pnl)}</strong>
                                <span>Balance {formatMoney(trade.balance)}</span>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    ) : (
                      <div className="simulator-empty-state simulator-empty-state--compact">
                        No taken trades have appeared yet.
                      </div>
                    )}
                  </div>
                ) : null}
              </div>
            </div>
          ) : null}

          {activeTab === 'events' ? (
            <div className="simulator-panel-block simulator-panel-block--wide simulator-replay-block">
              <div className="simulator-section-title">Test Event Log</div>
              <div className="simulator-timeline">
                {timelineRows.length ? (
                  timelineRows.map(({ trade, start, end, overlaps }, index) => {
                    const result = getTradeResultLabel(trade);
                    return (
                      <div className="simulator-timeline-row" key={`${trade.pattern_id ?? trade.pattern_group_id ?? index}`}>
                        <div className="simulator-timeline-dot" />
                        <div className="simulator-timeline-main">
                          <strong>{trade.symbol ?? 'N/A'}</strong>
                          <span>{formatShortDate(start)} - {formatShortDate(end)}</span>
                        </div>
                        <span className={`simulator-result-pill simulator-result-pill--${result.toLowerCase()}`}>
                          {trade.skipped_for_overlap ? 'Skipped' : result}
                        </span>
                        <span>{hasSimulatorReplay ? formatMoney(trade.pnl) : `${formatNumber(trade.trade_length, 0)} bars`}</span>
                        <span>{overlaps} overlap</span>
                      </div>
                    );
                  })
                ) : (
                  <div className="simulator-empty-state">Run a replay to load test events.</div>
                )}
              </div>
            </div>
          ) : null}
        </section>
      </div>
    </div>
  );
};

export default TradeSimulatorPanel;
