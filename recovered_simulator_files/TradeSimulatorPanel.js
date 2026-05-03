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
  { id: 'rules', label: 'Rules' },
];

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

const getTradePnl = (trade = {}) => {
  const rawPnl = Number(trade.trade_pnl);
  if (Number.isFinite(rawPnl)) {
    return rawPnl;
  }

  const result = Number(trade.trade_result);
  if (result === 1) return 1;
  if (result === 2) return -1;
  return 0;
};

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

const buildLoadedReplay = (trades = [], contracts = 1) => {
  let cumulative = 0;
  let peak = 0;

  const points = [
    {
      index: 0,
      trade: null,
      pnl: 0,
      total: 0,
      drawdown: 0,
      isStart: true,
    },
  ];

  trades.forEach((trade, index) => {
    const pnl = getTradePnl(trade) * contracts;
    cumulative += pnl;
    peak = Math.max(peak, cumulative);

    points.push({
      index: index + 1,
      trade,
      pnl,
      total: cumulative,
      drawdown: cumulative - peak,
    });
  });

  return points;
};

const buildServerReplay = (trades = [], startingBalance = 0) =>
  [
    {
      index: 0,
      trade: null,
      pnl: 0,
      total: 0,
      drawdown: 0,
      isStart: true,
    },
    ...trades
    .filter((trade) => !trade.skipped_for_overlap)
    .map((trade, index) => ({
      index: index + 1,
      trade,
      pnl: Number(trade.pnl) || 0,
      total: (Number(trade.balance) || startingBalance) - startingBalance,
      drawdown: Number(trade.drawdown) || 0,
    })),
  ];

const buildPath = (points, key, xForIndex, yForValue) =>
  points
    .map((point) => `${xForIndex(point.index).toFixed(2)},${yForValue(point[key]).toFixed(2)}`)
    .join(' ');

function SimulatorChart({ replayPoints, mode, profitTarget = null, maxDrawdown = null, showThresholds = false }) {
  const width = 640;
  const height = 220;
  const padX = 34;
  const padY = 26;
  const valueKey = mode === 'total' ? 'total' : 'pnl';
  const drawdownFloor = showThresholds && Number.isFinite(Number(maxDrawdown))
    ? -Math.abs(Number(maxDrawdown))
    : null;
  const targetLine = showThresholds && Number.isFinite(Number(profitTarget))
    ? Math.abs(Number(profitTarget))
    : null;

  const values = replayPoints.flatMap((point) => [point[valueKey], 0]);
  if (drawdownFloor !== null) values.push(drawdownFloor);
  if (targetLine !== null) values.push(targetLine);
  const minValue = values.length ? Math.min(...values) : -1;
  const maxValue = values.length ? Math.max(...values) : 1;
  const span = maxValue === minValue ? 1 : maxValue - minValue;
  const xSpan = Math.max(replayPoints.length - 1, 1);
  const isThresholdOnlyGraph = showThresholds && mode === 'total';

  const xForIndex = (index) => padX + (index / xSpan) * (width - padX * 2);
  const yForValue = (value) => padY + ((maxValue - value) / span) * (height - padY * 2);
  const zeroY = yForValue(0);
  const pnlPath = buildPath(replayPoints, valueKey, xForIndex, yForValue);
  const thresholdStartX = padX;
  const thresholdEndX = width - padX;
  const formatChartMoney = (value) =>
    new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
      maximumFractionDigits: 0,
    }).format(Number(value) || 0);

  return (
    <div className="simulator-chart-shell">
      {replayPoints.length ? (
        <svg
          className="simulator-chart"
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          aria-label="PnL and drawdown preview"
        >
          {isThresholdOnlyGraph ? null : (
            <line x1={padX} y1={zeroY} x2={width - padX} y2={zeroY} className="simulator-chart-zero" />
          )}
          {drawdownFloor !== null ? (
            <line
              x1={thresholdStartX}
              y1={yForValue(drawdownFloor)}
              x2={thresholdEndX}
              y2={yForValue(drawdownFloor)}
              className="simulator-chart-max-drawdown"
            />
          ) : null}
          {targetLine !== null ? (
            <line
              x1={thresholdStartX}
              y1={yForValue(targetLine)}
              x2={thresholdEndX}
              y2={yForValue(targetLine)}
              className="simulator-chart-profit-target"
            />
          ) : null}
          {isThresholdOnlyGraph ? (
            <>
              <line
                x1={thresholdStartX}
                y1={zeroY}
                x2={thresholdEndX}
                y2={zeroY}
                className="simulator-chart-pnl"
              />
              {replayPoints
                .filter((point) => !point.isStart)
                .map((point) => (
                  <g
                    key={`${point.index}-${point.total}-trade-dot`}
                    className={point.pnl >= 0 ? 'simulator-chart-trade-dot simulator-chart-trade-dot--win' : 'simulator-chart-trade-dot simulator-chart-trade-dot--loss'}
                  >
                    <circle
                      className="simulator-chart-trade-dot__halo"
                      cx={xForIndex(point.index)}
                      cy={yForValue(point[valueKey])}
                      r="8"
                    />
                    <circle
                      className="simulator-chart-trade-dot__core"
                      cx={xForIndex(point.index)}
                      cy={yForValue(point[valueKey])}
                      r="4"
                    />
                    <title>
                      {`Trade ${point.index}: ${formatChartMoney(point.pnl)} | Total ${formatChartMoney(point.total)}`}
                    </title>
                  </g>
                ))}
            </>
          ) : (
            <>
              <polyline points={pnlPath} className="simulator-chart-pnl" />
              {replayPoints.map((point) => (
                <circle
                  key={`${point.index}-${point.total}-${point.isStart ? 'start' : 'trade'}`}
                  cx={xForIndex(point.index)}
                  cy={yForValue(point[valueKey])}
                  r={point.isStart ? '4' : '3'}
                  className={
                    point.isStart
                      ? 'simulator-chart-dot--start'
                      : point.pnl >= 0
                      ? 'simulator-chart-dot--win'
                      : 'simulator-chart-dot--loss'
                  }
                />
              ))}
            </>
          )}
        </svg>
      ) : (
        <div className="simulator-empty-state">No loaded family rows yet.</div>
      )}
    </div>
  );
}

const MetricTile = ({ label, value, tone = 'neutral' }) => (
  <div className={`simulator-metric simulator-metric--${tone}`}>
    <span className="simulator-metric-label">{label}</span>
    <span className="simulator-metric-value">{value}</span>
  </div>
);

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

const TradeSimulatorPanel = ({
  selectedStrategy = null,
  loadedTrades = [],
  totalTradeCount = 0,
}) => {
  const [activeTab, setActiveTab] = useState('overview');
  const [familyId, setFamilyId] = useState('');
  const [firstStartDate, setFirstStartDate] = useState('2021-04-26');
  const [testsToChain, setTestsToChain] = useState(10);
  const [contractsPerTrade, setContractsPerTrade] = useState(1);
  const [pointValue, setPointValue] = useState(50);
  const [oneTradeAtATime, setOneTradeAtATime] = useState(true);
  const [accountSize, setAccountSize] = useState('50K');
  const [drawdownModel, setDrawdownModel] = useState('intraday');
  const [pnlMode, setPnlMode] = useState('trade');
  const [simulatorReplay, setSimulatorReplay] = useState(null);
  const [playbackEventCount, setPlaybackEventCount] = useState(0);
  const [isPlaybackRunning, setPlaybackRunning] = useState(false);
  const [isRunningReplay, setRunningReplay] = useState(false);
  const [replayError, setReplayError] = useState('');

  const selectedFamilyId = selectedStrategy?.propStrategyId ?? selectedStrategy?.id ?? '';

  useEffect(() => {
    setFamilyId(selectedFamilyId);
  }, [selectedFamilyId]);

  useEffect(() => {
    if (!simulatorReplay?.trades?.length || !isPlaybackRunning) {
      return undefined;
    }

    const timer = window.setInterval(() => {
      setPlaybackEventCount((current) => {
        const next = Math.min(current + 1, simulatorReplay.trades.length);
        if (next >= simulatorReplay.trades.length) {
          setPlaybackRunning(false);
        }
        return next;
      });
    }, 420);

    return () => window.clearInterval(timer);
  }, [isPlaybackRunning, simulatorReplay]);

  const accountRules = APEX_ACCOUNT_RULES[accountSize] ?? APEX_ACCOUNT_RULES['50K'];
  const contracts = Math.max(1, Number.parseInt(String(contractsPerTrade), 10) || 1);
  const normalizedPointValue = Math.max(0, Number.parseFloat(String(pointValue)) || 0);
  const dailyLossLimit = drawdownModel === 'eod' ? accountRules.eodDailyLossLimit : null;

  const orderedTrades = useMemo(
    () =>
      loadedTrades
        .filter((trade) => getTradeDate(trade))
        .slice()
        .sort((left, right) => (toTime(getTradeDate(left)) ?? 0) - (toTime(getTradeDate(right)) ?? 0)),
    [loadedTrades]
  );

  const replayPoints = useMemo(
    () => buildLoadedReplay(orderedTrades.slice(0, 40), contracts),
    [contracts, orderedTrades]
  );

  const visibleReplayTrades = useMemo(
    () => (simulatorReplay?.trades ?? []).slice(0, playbackEventCount),
    [playbackEventCount, simulatorReplay]
  );

  const simulatedReplayPoints = useMemo(
    () => buildServerReplay(visibleReplayTrades, accountRules.startingBalance),
    [accountRules.startingBalance, visibleReplayTrades]
  );
  const hasSimulatorReplay = Boolean(simulatorReplay);
  const chartReplayPoints = hasSimulatorReplay ? simulatedReplayPoints : replayPoints;
  const isReplayComplete = hasSimulatorReplay
    ? playbackEventCount >= (simulatorReplay?.trades?.length ?? 0)
    : false;

  const replayTestProgress = useMemo(() => {
    if (!simulatorReplay?.tests?.length) {
      return [];
    }

    return simulatorReplay.tests.map((test) => {
      const totalEvents = simulatorReplay.trades.filter(
        (trade) => trade.test_index === test.test_index
      ).length;
      const visibleEvents = visibleReplayTrades.filter(
        (trade) => trade.test_index === test.test_index
      ).length;
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
      };
    });
  }, [simulatorReplay, visibleReplayTrades]);

  const replaySummary = useMemo(() => {
    const sourcePoints = hasSimulatorReplay ? simulatedReplayPoints : replayPoints;
    const tradePoints = sourcePoints.filter((point) => !point.isStart);
    const last = sourcePoints[sourcePoints.length - 1] ?? null;
    const passed = replayTestProgress.filter((test) => test.status === 'passed').length;
    const failed = replayTestProgress.filter((test) => test.status === 'failed').length;
    const wins = tradePoints.filter((point) => point.pnl > 0).length;
    const losses = tradePoints.filter((point) => point.pnl < 0).length;
    const maxDrawdown = sourcePoints.reduce(
      (current, point) => Math.min(current, point.drawdown),
      0
    );

    return {
      total: last?.total ?? 0,
      wins: hasSimulatorReplay ? passed : wins,
      losses: hasSimulatorReplay ? failed : losses,
      maxDrawdown,
      trades: tradePoints.length,
      skipped: visibleReplayTrades.filter((trade) => trade.skipped_for_overlap).length,
      running: replayTestProgress.filter((test) => test.status === 'running').length,
      waiting: replayTestProgress.filter((test) => test.status === 'waiting').length,
    };
  }, [
    hasSimulatorReplay,
    replayPoints,
    replayTestProgress,
    simulatedReplayPoints,
    visibleReplayTrades,
  ]);

  const replayRunLabel = useMemo(() => {
    if (!hasSimulatorReplay) {
      return 'Waiting';
    }

    if (isPlaybackRunning) {
      return `Live ${playbackEventCount}/${simulatorReplay.trades.length}`;
    }

    return isReplayComplete ? 'Complete' : 'Paused';
  }, [hasSimulatorReplay, isPlaybackRunning, isReplayComplete, playbackEventCount, simulatorReplay]);

  const replayStatusCounts = useMemo(
    () => ({
      passed: replayTestProgress.filter((test) => test.status === 'passed').length,
      failed: replayTestProgress.filter((test) => test.status === 'failed').length,
      running: replayTestProgress.filter((test) => test.status === 'running').length,
    }),
    [replayTestProgress]
  );

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
      const sourceRows = simulatorReplay?.trades?.length ? visibleReplayTrades : orderedTrades;
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
    [orderedTrades, simulatorReplay, visibleReplayTrades]
  );

  const runReplay = async () => {
    if (!familyId || isRunningReplay) {
      return;
    }

    setRunningReplay(true);
    setReplayError('');
    const result = await fetchSimulatorFamilyReplay({
      familyId,
      firstStartDate,
      testsToChain: Number.parseInt(String(testsToChain), 10) || 1,
      contracts,
      pointValue: normalizedPointValue,
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
    setPlaybackEventCount(0);
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

  const selectedIdentityRows = [
    { label: 'Family ID', value: familyId || 'None selected' },
    { label: 'Market', value: selectedStrategy?.market ?? 'N/A' },
    { label: 'Dominant Harmonic', value: selectedStrategy?.harmonicType ?? 'N/A' },
    { label: 'Price Ratio Accuracy', value: selectedStrategy?.bin ?? 'N/A' },
    { label: 'Time Ratio Accuracy', value: selectedStrategy?.timeBin ?? 'N/A' },
    { label: 'Size', value: selectedStrategy?.sizeBucket ?? 'N/A' },
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
        <aside className="simulator-config-rail">
          <label className="simulator-field simulator-field--wide">
            <span>Family ID</span>
            <input
              value={familyId}
              onChange={(event) => setFamilyId(event.target.value)}
              spellCheck="false"
            />
          </label>
          <button type="button" className="simulator-secondary-button" onClick={() => setFamilyId(selectedFamilyId)}>
            Use Selected
          </button>

          <label className="simulator-field">
            <span>First Test Start</span>
            <input
              type="date"
              value={firstStartDate}
              onChange={(event) => setFirstStartDate(event.target.value)}
            />
          </label>

          <div className="simulator-field-row">
            <label className="simulator-field">
              <span>Tests To Chain</span>
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
          <label className="simulator-field">
            <span>Point Value</span>
            <input
              type="number"
              min="0"
              step="0.01"
              value={pointValue}
              onChange={(event) => setPointValue(event.target.value)}
            />
          </label>

          <label className="simulator-field">
            <span>Apex Account</span>
            <select value={accountSize} onChange={(event) => setAccountSize(event.target.value)}>
              {Object.keys(APEX_ACCOUNT_RULES).map((key) => (
                <option value={key} key={key}>
                  {key}
                </option>
              ))}
            </select>
          </label>

          <label className="simulator-field">
            <span>Drawdown Model</span>
            <select value={drawdownModel} onChange={(event) => setDrawdownModel(event.target.value)}>
              <option value="intraday">Intraday Trail</option>
              <option value="eod">EOD</option>
            </select>
          </label>

          <label className="simulator-check">
            <input
              type="checkbox"
              checked={oneTradeAtATime}
              onChange={(event) => setOneTradeAtATime(event.target.checked)}
            />
            <span>One trade at a time</span>
          </label>

          <div className="simulator-rail-summary">
            <span>{formatMoney(accountRules.profitTarget)} target</span>
            <span>{formatMoney(accountRules.maxDrawdown)} drawdown</span>
            <span>{totalTradeCount.toLocaleString()} family rows</span>
          </div>

          <button
            type="button"
            className="simulator-primary-button"
            onClick={runReplay}
            disabled={!familyId || isRunningReplay}
          >
            {isRunningReplay ? 'Running...' : 'Run Replay'}
          </button>
          {replayError ? <div className="simulator-error-state">{replayError}</div> : null}
        </aside>

        <section className="simulator-workspace">
          {activeTab === 'overview' ? (
            <div className="simulator-setup-grid">
              <div className="simulator-panel-block">
                <div className="simulator-section-title">Selected Family</div>
                <RuleTable rows={selectedIdentityRows} />
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
                <RuleTable rows={evaluationRows} />
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
            <div className="simulator-results-grid">
              <div className="simulator-panel-block simulator-panel-block--wide">
                <div className="simulator-result-header">
                  <div className="simulator-section-title">
                    PnL Levels
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
                          {isPlaybackRunning ? 'Pause' : isReplayComplete ? 'Replay' : 'Play'}
                        </button>
                        <button
                          type="button"
                          className="simulator-toggle"
                          onClick={restartReplayPlayback}
                        >
                          Restart
                        </button>
                      </>
                    ) : null}
                  </div>
                </div>
                <SimulatorChart
                  replayPoints={chartReplayPoints}
                  mode={pnlMode}
                  profitTarget={accountRules.profitTarget}
                  maxDrawdown={accountRules.maxDrawdown}
                  showThresholds={pnlMode === 'total'}
                />
                {hasSimulatorReplay ? (
                  <div className="simulator-test-strip">
                    {replayTestProgress.map((test) => {
                      const tone = getTestTone(test.status);
                      return (
                        <div className={`simulator-test-card simulator-test-card--${tone}`} key={test.test_index}>
                          <span>Test {test.test_index}</span>
                          <strong>{test.status}</strong>
                          <small>
                            {test.visibleEvents}/{test.totalEvents || 0}
                          </small>
                        </div>
                      );
                    })}
                  </div>
                ) : null}
              </div>

              <div className="simulator-metric-grid">
                <MetricTile label={hasSimulatorReplay ? 'Taken Trades' : 'Loaded Trades'} value={replaySummary.trades} />
                <MetricTile label={hasSimulatorReplay ? 'Passed Tests' : 'Wins'} value={replaySummary.wins} tone="passed" />
                <MetricTile label={hasSimulatorReplay ? 'Failed Tests' : 'Losses'} value={replaySummary.losses} tone="failed" />
                <MetricTile label={hasSimulatorReplay ? 'Replay Total' : 'Preview Total'} value={hasSimulatorReplay ? formatMoney(replaySummary.total) : formatNumber(replaySummary.total, 2)} />
                <MetricTile label="Max Drawdown" value={hasSimulatorReplay ? formatMoney(replaySummary.maxDrawdown) : formatNumber(replaySummary.maxDrawdown, 2)} tone="failed" />
                {hasSimulatorReplay ? <MetricTile label="Overlap Skips" value={replaySummary.skipped} /> : null}
                {hasSimulatorReplay ? <MetricTile label="Running" value={replaySummary.running} /> : null}
              </div>
            </div>
          ) : null}

          {activeTab === 'overview' ? (
            <div className="simulator-panel-block simulator-panel-block--wide simulator-replay-block">
              <div className="simulator-section-title">Family Timeline</div>
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
                  <div className="simulator-empty-state">No timeline rows loaded.</div>
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
