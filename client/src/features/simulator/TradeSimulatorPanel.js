import React, { useEffect, useMemo, useState } from 'react';

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
  { id: 'setup', label: 'Setup' },
  { id: 'rules', label: 'Rules' },
  { id: 'results', label: 'Results' },
  { id: 'replay', label: 'Replay' },
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
  trade.reversal_detect_date ?? trade.d_confirm_date ?? trade.d_date ?? null;

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

const toTime = (value) => {
  const date = value ? new Date(value) : null;
  return date && !Number.isNaN(date.getTime()) ? date.getTime() : null;
};

const buildLoadedReplay = (trades = [], contracts = 1) => {
  let cumulative = 0;
  let peak = 0;

  return trades.map((trade, index) => {
    const pnl = getTradePnl(trade) * contracts;
    cumulative += pnl;
    peak = Math.max(peak, cumulative);

    return {
      index,
      trade,
      pnl,
      total: cumulative,
      drawdown: cumulative - peak,
    };
  });
};

const buildPath = (points, key, xForIndex, yForValue) =>
  points
    .map((point) => `${xForIndex(point.index).toFixed(2)},${yForValue(point[key]).toFixed(2)}`)
    .join(' ');

function SimulatorChart({ replayPoints, mode }) {
  const width = 640;
  const height = 220;
  const padX = 34;
  const padY = 26;
  const valueKey = mode === 'total' ? 'total' : 'pnl';

  const values = replayPoints.flatMap((point) => [point[valueKey], point.drawdown, 0]);
  const minValue = values.length ? Math.min(...values) : -1;
  const maxValue = values.length ? Math.max(...values) : 1;
  const span = maxValue === minValue ? 1 : maxValue - minValue;
  const xSpan = Math.max(replayPoints.length - 1, 1);

  const xForIndex = (index) => padX + (index / xSpan) * (width - padX * 2);
  const yForValue = (value) => padY + ((maxValue - value) / span) * (height - padY * 2);
  const zeroY = yForValue(0);
  const pnlPath = buildPath(replayPoints, valueKey, xForIndex, yForValue);
  const drawdownPath = buildPath(replayPoints, 'drawdown', xForIndex, yForValue);

  return (
    <div className="simulator-chart-shell">
      {replayPoints.length ? (
        <svg
          className="simulator-chart"
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          aria-label="PnL and drawdown preview"
        >
          <line x1={padX} y1={zeroY} x2={width - padX} y2={zeroY} className="simulator-chart-zero" />
          <polyline points={drawdownPath} className="simulator-chart-drawdown" />
          <polyline points={pnlPath} className="simulator-chart-pnl" />
          {replayPoints.map((point) => (
            <circle
              key={`${point.index}-${point.total}`}
              cx={xForIndex(point.index)}
              cy={yForValue(point[valueKey])}
              r="3"
              className={point.pnl >= 0 ? 'simulator-chart-dot--win' : 'simulator-chart-dot--loss'}
            />
          ))}
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
  const [activeTab, setActiveTab] = useState('setup');
  const [familyId, setFamilyId] = useState('');
  const [firstStartDate, setFirstStartDate] = useState('2021-04-26');
  const [testsToChain, setTestsToChain] = useState(10);
  const [contractsPerTrade, setContractsPerTrade] = useState(1);
  const [oneTradeAtATime, setOneTradeAtATime] = useState(true);
  const [accountSize, setAccountSize] = useState('50K');
  const [drawdownModel, setDrawdownModel] = useState('intraday');
  const [pnlMode, setPnlMode] = useState('trade');

  const selectedFamilyId = selectedStrategy?.propStrategyId ?? selectedStrategy?.id ?? '';

  useEffect(() => {
    setFamilyId(selectedFamilyId);
  }, [selectedFamilyId]);

  const accountRules = APEX_ACCOUNT_RULES[accountSize] ?? APEX_ACCOUNT_RULES['50K'];
  const contracts = Math.max(1, Number.parseInt(String(contractsPerTrade), 10) || 1);

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

  const replaySummary = useMemo(() => {
    const last = replayPoints[replayPoints.length - 1] ?? null;
    const wins = replayPoints.filter((point) => point.pnl > 0).length;
    const losses = replayPoints.filter((point) => point.pnl < 0).length;
    const maxDrawdown = replayPoints.reduce(
      (current, point) => Math.min(current, point.drawdown),
      0
    );

    return {
      total: last?.total ?? 0,
      wins,
      losses,
      maxDrawdown,
      trades: replayPoints.length,
    };
  }, [replayPoints]);

  const timelineRows = useMemo(
    () =>
      orderedTrades.slice(0, 24).map((trade, index) => {
        const start = getTradeDate(trade);
        const end = getTradeEndDate(trade);
        const startTime = toTime(start);
        const endTime = toTime(end) ?? startTime;
        const overlaps =
          startTime === null || endTime === null
            ? 0
            : orderedTrades.filter((candidate, candidateIndex) => {
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
      }),
    [orderedTrades]
  );

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
          <span className="simulator-status-badge simulator-status-badge--passed">Passed</span>
          <span className="simulator-status-badge simulator-status-badge--failed">Failed</span>
          <span className="simulator-status-badge">Waiting</span>
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
        </aside>

        <section className="simulator-workspace">
          {activeTab === 'setup' ? (
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

          {activeTab === 'results' ? (
            <div className="simulator-results-grid">
              <div className="simulator-panel-block simulator-panel-block--wide">
                <div className="simulator-result-header">
                  <div className="simulator-section-title">Loaded Family PnL</div>
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
                  </div>
                </div>
                <SimulatorChart replayPoints={replayPoints} mode={pnlMode} />
              </div>

              <div className="simulator-metric-grid">
                <MetricTile label="Loaded Trades" value={replaySummary.trades} />
                <MetricTile label="Wins" value={replaySummary.wins} tone="passed" />
                <MetricTile label="Losses" value={replaySummary.losses} tone="failed" />
                <MetricTile label="Preview Total" value={formatNumber(replaySummary.total, 2)} />
                <MetricTile label="Preview Drawdown" value={formatNumber(replaySummary.maxDrawdown, 2)} tone="failed" />
              </div>
            </div>
          ) : null}

          {activeTab === 'replay' ? (
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
                          {result}
                        </span>
                        <span>{formatNumber(trade.trade_length, 0)} bars</span>
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
