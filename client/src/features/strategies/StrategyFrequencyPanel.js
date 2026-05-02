import React from 'react';

const formatPercent = (value) =>
  Number.isFinite(value) ? `${value.toFixed(2)}%` : 'N/A';

const formatNumber = (value, digits = 2) =>
  Number.isFinite(value) ? value.toFixed(digits) : '0.00';

const getCadenceRating = (cadence = {}) => {
  const totalWeeks = Number(cadence.totalCalendarWeeks ?? 0);
  const activeWeeks = Number(cadence.activeWeeks ?? 0);
  const avgPerWeek = Number(cadence.avgSetupsPerWeek ?? 0);
  const zeroWeekRate = Number(cadence.zeroSetupWeekRate ?? 0);
  const activeRate = totalWeeks > 0 ? activeWeeks / totalWeeks : 0;

  if (activeRate >= 0.6 && avgPerWeek >= 1) return 'Steady';
  if (activeRate >= 0.3 || avgPerWeek >= 0.5 || zeroWeekRate <= 0.7) return 'Intermittent';
  return 'Sparse';
};

const MetricCard = ({ label, value, helper }) => (
  <div className="strategy-frequency-metric">
    <div className="strategy-frequency-metric__label">{label}</div>
    <div className="strategy-frequency-metric__value">{value}</div>
    <div className="strategy-frequency-metric__helper">{helper}</div>
  </div>
);

export default function StrategyFrequencyPanel({ strategy = null }) {
  const cadence = strategy?.weeklyCadence ?? {};
  const totalWeeks = Number(cadence.totalCalendarWeeks ?? 0);
  const activeWeeks = Number(cadence.activeWeeks ?? 0);
  const zeroWeeks = Number(cadence.zeroSetupWeeks ?? 0);
  const totalSetups = Number(cadence.totalSetups ?? 0);
  const avgPerWeek = Number(cadence.avgSetupsPerWeek ?? 0);
  const maxPerWeek = Number(cadence.maxSetupsPerWeek ?? 0);
  const zeroWeekRate = Number(cadence.zeroSetupWeekRate ?? 0);
  const activeRate = totalWeeks > 0 ? activeWeeks / totalWeeks : 0;
  const everyWeeks = avgPerWeek > 0 ? 1 / avgPerWeek : null;
  const rating = getCadenceRating(cadence);

  if (!strategy) {
    return <div className="strategy-empty-row">Select a family to inspect frequency.</div>;
  }

  return (
    <div className="strategy-frequency-shell">
      <div className="strategy-frequency-header">
        <div>
          <div className="strategy-frequency-kicker">Selected Family</div>
          <h3 className="strategy-frequency-title">{strategy.familyName ?? strategy.name}</h3>
        </div>
        <div className={`strategy-frequency-rating strategy-frequency-rating--${rating.toLowerCase()}`}>
          {rating}
        </div>
      </div>

      <div className="strategy-frequency-grid">
        <MetricCard
          label="Active Weeks"
          value={`${activeWeeks} / ${totalWeeks}`}
          helper={`${formatPercent(activeRate * 100)} active`}
        />
        <MetricCard
          label="Avg/Wk"
          value={formatNumber(avgPerWeek, 2)}
          helper={everyWeeks ? `1 every ${formatNumber(everyWeeks, 1)} wks` : 'No weekly pace yet'}
        />
        <MetricCard
          label="Max/Wk"
          value={`${Math.round(maxPerWeek)}`}
          helper="Busiest observed week"
        />
        <MetricCard
          label="Zero Weeks"
          value={formatPercent(zeroWeekRate * 100)}
          helper={`${zeroWeeks} quiet weeks`}
        />
        <MetricCard
          label="Total Setups"
          value={`${Math.round(totalSetups)}`}
          helper="Patterns in this family"
        />
      </div>

      <div className="strategy-frequency-narrative">
        <p>
          {activeWeeks} / {totalWeeks} active weeks means it appeared in about{' '}
          {formatPercent(activeRate * 100)} of weeks.
        </p>
        <p>
          {formatNumber(avgPerWeek, 2)} avg/wk means{' '}
          {everyWeeks
            ? `roughly one setup every ${formatNumber(everyWeeks, 1)} weeks on average.`
            : 'there is not enough cadence yet to estimate a repeat interval.'}
        </p>
        <p>{Math.round(maxPerWeek)} max/wk means its busiest week had {Math.round(maxPerWeek)} setups.</p>
        <p>{formatPercent(zeroWeekRate * 100)} zero weeks means most quiet periods are visible.</p>
      </div>
    </div>
  );
}
