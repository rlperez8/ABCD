import React from 'react';

const formatPercent = (value) =>
  Number.isFinite(value) ? `${value.toFixed(2)}%` : 'N/A';

const formatNumber = (value, digits = 0) =>
  Number.isFinite(value) ? value.toFixed(digits) : 'N/A';

const MetricCard = ({ label, value, helper = null, muted = false, featured = false }) => (
  <div
    className={[
      'strategy-score-card',
      muted ? 'strategy-score-card--muted' : '',
      featured ? 'strategy-score-card--featured' : '',
    ]
      .filter(Boolean)
      .join(' ')}
  >
    <div className="strategy-score-card-label">{label}</div>
    <div className="strategy-score-card-value">{value}</div>
    {helper ? <div className="strategy-score-card-helper">{helper}</div> : null}
  </div>
);

const StrategyWorkbenchCard = ({ strategy, isLoading = false }) => {
  const summary = strategy?.comparison?.summary;

  if (!strategy) {
    return (
      <div className="strategy-scoreboard">
        <div className="strategy-scoreboard-header">
          <div className="strategy-scoreboard-title">Selected Strategy</div>
          <div className="strategy-scoreboard-status">Waiting</div>
        </div>
        <div className="strategy-empty">Choose a strategy to inspect its profile.</div>
      </div>
    );
  }

  const cards = [
    {
      label: 'Pattern',
      value: strategy.harmonicType,
      helper: strategy.name,
      featured: true,
    },
    {
      label: 'Market',
      value: strategy.market,
    },
    {
      label: 'Price Bin',
      value: strategy.bin,
    },
    {
      label: 'Reversal',
      value: strategy.reversalType ?? 'None',
    },
    {
      label: 'Size',
      value: strategy.sizeBucket,
    },
    {
      label: 'Time Bin',
      value: strategy.timeBin,
    },
    {
      label: 'Worst Year',
      value: formatPercent(strategy.worstYearExpectancy),
      helper: `${strategy.downYears ?? 0} down years`,
      muted: (strategy.downYears ?? 0) > 0,
    },
    {
      label: 'Expectancy',
      value: formatPercent(summary?.expectancy),
      helper: isLoading ? 'Refreshing' : 'Closed-trade edge',
    },
    {
      label: 'Win Rate',
      value:
        Number.isFinite(summary?.win_rate)
          ? formatPercent(summary.win_rate * 100)
          : 'N/A',
      helper: `${summary?.win_count ?? 0} wins / ${summary?.loss_count ?? 0} losses`,
    },
    {
      label: 'Sample',
      value: `${summary?.closed_count ?? 0}/${summary?.total_count ?? 0}`,
      helper: `${summary?.open_count ?? 0} still open`,
    },
    {
      label: 'Avg Return',
      value: formatPercent(summary?.avg_return),
    },
    {
      label: 'Avg Bars',
      value: formatNumber(summary?.avg_trade_length, 0),
      helper: 'Bars held on average',
    },
    {
      label: 'Avg Win',
      value: formatPercent(summary?.avg_win),
    },
    {
      label: 'Avg Loss',
      value: formatPercent(summary?.avg_loss),
      muted: true,
    },
    {
      label: 'AB / XA',
      value: formatNumber(summary?.avg_ab_xa, 2),
      helper: 'Price retracement',
    },
    {
      label: 'BC / AB',
      value: formatNumber(summary?.avg_bc_ab, 2),
      helper: 'Price retracement',
    },
    {
      label: 'CD / BC',
      value: formatNumber(summary?.avg_cd_bc, 2),
      helper: 'Price retracement',
    },
    {
      label: 'CD / XA',
      value: formatNumber(summary?.avg_cd_xa, 2),
      helper: 'Price retracement',
    },
  ];

  return (
    <div className="strategy-scoreboard">
      <div className="strategy-scoreboard-header">
        <div className="strategy-scoreboard-title">Selected Strategy</div>
        <div className="strategy-scoreboard-status">
          {isLoading ? 'Refreshing' : 'Live'}
        </div>
      </div>
      <div className="strategy-scoreboard-grid">
        {cards.map((card) => (
          <MetricCard
            key={card.label}
            label={card.label}
            value={card.value}
            helper={card.helper}
            muted={card.muted}
            featured={card.featured}
          />
        ))}
      </div>
    </div>
  );
};

export default StrategyWorkbenchCard;
