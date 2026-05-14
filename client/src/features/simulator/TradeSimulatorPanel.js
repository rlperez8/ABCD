import React, { useCallback, useEffect, useMemo, useState } from 'react';
import CandleChartPanel from '../candle-chart/CandleChartPanel';
import {
  fetchPhase1Results,
  fetchPhase1RouteReplay,
  fetchSimulatorFamilyReplay,
} from '../../services/patternApi';

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
  { id: 'rules', label: 'Rules' },
];

const SIMULATOR_PLAYBACK_EVENTS_PER_TICK = 1;
const SIMULATOR_PLAYBACK_SPEEDS = {
  slow: { label: 'Slow', intervalMs: 180 },
  medium: { label: 'Medium', intervalMs: 70 },
  fast: { label: 'Fast', intervalMs: 24 },
};
const SIMULATOR_START_DATE = '2021-01-01';
const EMPTY_CHART_CANDLES = [];

const formatMoney = (value) =>
  new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(Number(value) || 0);

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

const normalizeDateInput = (value) => {
  if (!value) {
    return '';
  }

  const rawValue = String(value);
  const isoMatch = rawValue.match(/^(\d{4}-\d{2}-\d{2})/);
  if (isoMatch) {
    return isoMatch[1];
  }

  const date = new Date(rawValue);
  if (Number.isNaN(date.getTime())) {
    return '';
  }

  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
};

const formatDateOption = (value) => {
  const normalizedDate = normalizeDateInput(value);
  if (!normalizedDate) {
    return 'Select date';
  }

  const [year, month, day] = normalizedDate.split('-').map((part) => Number.parseInt(part, 10));
  const date = new Date(year, month - 1, day);
  if (Number.isNaN(date.getTime())) {
    return normalizedDate;
  }

  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
};

const getTradeResultLabel = (trade = {}) => {
  const result = Number(trade.trade_result);
  if (result === 1) return 'Won';
  if (result === 2) return 'Lost';
  return 'Open';
};

const getSimulatorTradeKey = (trade = {}, fallback = '') =>
  [
    trade.pattern_id ?? '',
    trade.pattern_group_id ?? '',
    trade.entry_date ?? '',
    trade.test_index ?? '',
    trade.trade_index ?? fallback,
  ].join('|');

const getReplayPatternMatchKeys = (trade = {}) => {
  const keys = [];

  if (trade.pattern_id) {
    keys.push(`id:${trade.pattern_id}`);
  }

  if (trade.pattern_group_id) {
    keys.push(`group:${trade.pattern_group_id}`);
    [trade.entry_date, trade.reversal_detect_date, trade.d_confirm_date, trade.d_date]
      .filter(Boolean)
      .forEach((dateValue) => {
        keys.push(`group-date:${trade.pattern_group_id}|${String(dateValue).slice(0, 19)}`);
        keys.push(`group-day:${trade.pattern_group_id}|${String(dateValue).slice(0, 10)}`);
      });
  }

  return keys;
};

const getFamilyOptionId = (strategy = {}) =>
  strategy.propStrategyId ?? strategy.familyKey ?? strategy.id ?? '';

const isPhase1FamilyId = (value = '') =>
  /^[a-f0-9]{16}$/i.test(String(value ?? '').trim());

const getPhase1RouteKey = (route = {}) => `${route.run_id ?? ''}|${route.route_id ?? ''}`;

const formatPhase1RouteOption = (route = {}) => {
  const rank = Number(route.result_rank);
  const rankLabel = Number.isFinite(rank) && rank > 0 ? `#${rank}` : 'Route';
  const avgR = Number(route.avg_r);
  const avgRLabel = Number.isFinite(avgR) ? `${avgR.toFixed(2)}R` : 'R N/A';
  const trades = Number(route.trade_count);
  const tradeLabel = Number.isFinite(trades) ? `${trades.toLocaleString()} trades` : 'trades N/A';

  return `${rankLabel} ${route.route_label ?? route.route_id ?? 'Phase 1 route'} / ${avgRLabel} / ${tradeLabel}`;
};

const formatRouteMode = (value = '') =>
  String(value || 'N/A')
    .split('_')
    .filter(Boolean)
    .map((part) => (part.length <= 2 ? part.toUpperCase() : `${part[0].toUpperCase()}${part.slice(1)}`))
    .join(' ');

const getFiniteNumber = (value, fallback = null) => {
  const numericValue = Number(value);
  return Number.isFinite(numericValue) ? numericValue : fallback;
};

const getDateTimeKey = (value) => {
  if (!value) {
    return null;
  }

  if (typeof value === 'string') {
    const match = value.match(/^(\d{4}-\d{2}-\d{2})(?:[T\s](\d{2}:\d{2}(?::\d{2})?))?/);
    if (match) {
      const time = match[2] ? (match[2].length === 5 ? `${match[2]}:00` : match[2]) : '00:00:00';
      return `${match[1]} ${time}`;
    }
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return null;
  }

  const pad = (part) => String(part).padStart(2, '0');
  return `${parsed.getFullYear()}-${pad(parsed.getMonth() + 1)}-${pad(parsed.getDate())} ${pad(parsed.getHours())}:${pad(parsed.getMinutes())}:${pad(parsed.getSeconds())}`;
};

const findCandleIndexByDate = (candles = [], value = null) => {
  const targetKey = getDateTimeKey(value);
  if (!targetKey) {
    return -1;
  }

  const exactIndex = candles.findIndex((candle) =>
    getDateTimeKey(candle?.candle_date ?? candle?.date) === targetKey
  );
  if (exactIndex >= 0) {
    return exactIndex + 1;
  }

  const targetDay = targetKey.slice(0, 10);
  const dayIndex = candles.findIndex((candle) =>
    getDateTimeKey(candle?.candle_date ?? candle?.date)?.slice(0, 10) === targetDay
  );
  return dayIndex >= 0 ? dayIndex + 1 : -1;
};

const mergeSelectedTradeIntoChartPattern = (pattern = null, trade = null, candles = []) => {
  if (!pattern || !trade) {
    return pattern;
  }

  const entryIndex = findCandleIndexByDate(candles, trade.entry_date);
  const exitIndex = findCandleIndexByDate(candles, trade.target_date);
  const exitPrice = getFiniteNumber(
    trade.exit_price ??
      trade.trade_exit_price ??
      (Number(trade.trade_result) === 2
        ? trade.trade_risk_exit_price
        : trade.trade_reward_exit_price),
    pattern.exit_price
  );

  return {
    ...pattern,
    entry_date: trade.entry_date ?? pattern.entry_date,
    target_date: trade.target_date ?? pattern.target_date,
    trade_enter_price: getFiniteNumber(trade.trade_enter_price, pattern.trade_enter_price),
    trade_risk_exit_price: getFiniteNumber(trade.trade_risk_exit_price, pattern.trade_risk_exit_price),
    trade_reward_exit_price: getFiniteNumber(trade.trade_reward_exit_price, pattern.trade_reward_exit_price),
    trade_current_price: exitPrice,
    target_close: exitPrice,
    exit_price: exitPrice,
    entry: entryIndex > 0 ? entryIndex : pattern.entry,
    target: exitIndex > 0 ? exitIndex : pattern.target,
    exit_date: exitIndex > 0 ? exitIndex : pattern.exit_date,
    trade_result: trade.trade_result ?? pattern.trade_result,
    result_r: trade.result_r ?? pattern.result_r,
    risk_points: trade.risk_points ?? pattern.risk_points,
  };
};

const formatFamilyOptionLabel = (strategy = {}) => {
  const familyId = getFamilyOptionId(strategy);
  const details = [
    strategy.market,
    strategy.harmonicType,
    strategy.bin,
    strategy.sizeBucket,
    strategy.timeBin,
  ].filter(Boolean);
  const score = Number(strategy.score);
  const scoreLabel = Number.isFinite(score) ? `score ${score.toFixed(1)}` : null;

  return [familyId, ...details, scoreLabel].filter(Boolean).join(' / ');
};

const getTestTone = (status = '') => {
  if (status === 'passed') return 'passed';
  if (status === 'failed') return 'failed';
  if (status === 'running') return 'running';
  return 'waiting';
};

const getScoreTone = (score) => {
  if (!Number.isFinite(score)) return 'waiting';
  if (score >= 85) return 'strong';
  if (score >= 70) return 'good';
  if (score >= 55) return 'watch';
  return 'weak';
};

const getScoreLabel = (score) => {
  if (!Number.isFinite(score)) return 'Waiting';
  if (score >= 85) return 'Strong run';
  if (score >= 70) return 'Good run';
  if (score >= 55) return 'Needs review';
  return 'Weak run';
};

const buildServerReplay = (
  trades = [],
  startingBalance = 0,
  maxDrawdown = 0,
  drawdownModel = 'intraday'
) => {
  let previousTotal = 0;
  let peak = 0;
  const drawdownDistance = Math.abs(Number(maxDrawdown) || 0);
  const usesIntradayDrawdown = drawdownModel !== 'eod';

  const points = [
    {
      index: 0,
      trade: null,
      pnl: 0,
      previousTotal: 0,
      total: 0,
      closedTotal: 0,
      intratradeLowTotal: 0,
      intratradeHighTotal: 0,
      intratradeLowPnl: 0,
      intratradeHighPnl: 0,
      drawdown: 0,
      drawdownLevel: -drawdownDistance,
      isStart: true,
    },
  ];

  trades
    .filter((trade) => !trade.skipped_for_overlap)
    .forEach((trade, index) => {
      const balanceBefore = Number.isFinite(Number(trade.balance_before))
        ? Number(trade.balance_before)
        : startingBalance + previousTotal;
      const total = (Number(trade.balance) || startingBalance) - startingBalance;
      const closedTotal = Number.isFinite(Number(trade.closed_balance))
        ? Number(trade.closed_balance) - startingBalance
        : total;
      const intratradeLowTotal = Number.isFinite(Number(trade.intratrade_low_balance))
        ? Number(trade.intratrade_low_balance) - startingBalance
        : Math.min(previousTotal, total);
      const intratradeHighTotal = Number.isFinite(Number(trade.intratrade_high_balance))
        ? Number(trade.intratrade_high_balance) - startingBalance
        : Math.max(previousTotal, closedTotal, total);
      const failedIntratradeDrawdown = Boolean(trade.failed_intratrade_drawdown);
      peak = failedIntratradeDrawdown
        ? peak
        : usesIntradayDrawdown
        ? Math.max(peak, intratradeHighTotal, total)
        : Math.max(peak, total);
      const drawdownLevel = peak - drawdownDistance;
      const lowWaterTotal = usesIntradayDrawdown ? Math.min(intratradeLowTotal, total) : total;
      points.push({
        index: index + 1,
        trade,
        pnl: Number(trade.pnl) || 0,
        closedPnl: Number(trade.closed_pnl) || Number(trade.pnl) || 0,
        previousTotal,
        balanceBefore,
        total,
        closedTotal,
        intratradeLowTotal,
        intratradeHighTotal,
        intratradeLowPnl: Number(trade.intratrade_adverse_pnl) || 0,
        intratradeHighPnl: Number(trade.intratrade_favorable_pnl) || 0,
        drawdown: lowWaterTotal - peak,
        drawdownLevel,
        failedIntratradeDrawdown,
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
  selectedTradeKey = '',
  onSelectTrade = null,
  highlightedPatternKeys = [],
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
    closedPnl: Number(point.closedPnl) || Number(point.pnl) || 0,
    previousTotal: Number(point.previousTotal) || 0,
    closedTotal: Number(point.closedTotal) || Number(point.total) || 0,
    intratradeLowTotal: Number.isFinite(Number(point.intratradeLowTotal))
      ? Number(point.intratradeLowTotal)
      : Math.min(Number(point.previousTotal) || 0, Number(point.total) || 0),
    intratradeHighTotal: Number.isFinite(Number(point.intratradeHighTotal))
      ? Number(point.intratradeHighTotal)
      : Math.max(Number(point.previousTotal) || 0, Number(point.total) || 0),
    intratradeLowPnl: Number.isFinite(Number(point.intratradeLowPnl))
      ? Number(point.intratradeLowPnl)
      : Math.min(0, Number(point.pnl) || 0),
    intratradeHighPnl: Number.isFinite(Number(point.intratradeHighPnl))
      ? Number(point.intratradeHighPnl)
      : Math.max(0, Number(point.pnl) || 0),
    drawdownLevel: Number.isFinite(Number(point.drawdownLevel))
      ? Number(point.drawdownLevel)
      : -drawdownDistance,
  }));
  const chartPoints = points.length
    ? points
    : [{
        index: 0,
        total: 0,
        pnl: 0,
        previousTotal: 0,
        closedTotal: 0,
        intratradeLowTotal: 0,
        intratradeHighTotal: 0,
        intratradeLowPnl: 0,
        intratradeHighPnl: 0,
        drawdownLevel: -drawdownDistance,
        isStart: true,
      }];
  const valueKey = totalMode ? 'total' : 'pnl';
  const tradePoints = chartPoints.filter((point) => !point.isStart);
  const plotWidth = width - plot.left - plot.right;
  const plotHeight = height - plot.top - plot.bottom;
  const xSpan = Math.max(Number(totalPointCount) || chartPoints.length - 1, 1);
  const values = chartPoints.flatMap((point) =>
    totalMode
      ? [point.total, point.drawdownLevel, point.intratradeLowTotal, point.intratradeHighTotal]
      : [point.pnl, point.intratradeLowPnl, point.intratradeHighPnl]
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
  const highlightedPatternKeySet = useMemo(
    () => new Set(highlightedPatternKeys),
    [highlightedPatternKeys]
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
          const lowValue = totalMode ? point.intratradeLowTotal : point.intratradeLowPnl;
          const highValue = totalMode ? point.intratradeHighTotal : point.intratradeHighPnl;
          const lowY = yForValue(lowValue);
          const highY = yForValue(highValue);
          const rangeTop = Math.min(lowY, highY);
          const rangeBottom = Math.max(lowY, highY);
          const stemTop = Math.min(previousY, pointY);
          const stemBottom = Math.max(previousY, pointY);
          const tooltipWidth = 244;
          const tooltipHeight = totalMode ? 126 : 112;
          const tooltipX = Math.min(pointX + 12, width - plot.right - tooltipWidth);
          const tooltipY = Math.max(plot.top + 8, stemTop - tooltipHeight - 10);
          const result = getTradeResultLabel(point.trade);
          const symbol = point.trade?.symbol ?? `Trade ${point.index}`;
          const exit = formatShortDate(point.trade?.target_date);
          const tradeTone = point.failedIntratradeDrawdown ? 'drawdown' : point.pnl >= 0 ? 'win' : 'loss';
          const tradeKey = getSimulatorTradeKey(point.trade, point.index);
          const isSelected = tradeKey === selectedTradeKey;
          const isHighlighted =
            point.trade &&
            getReplayPatternMatchKeys(point.trade).some((key) => highlightedPatternKeySet.has(key));
          const resultLine = point.failedIntratradeDrawdown ? 'DD fail' : `Result ${result}`;

          return (
            <g
              className={`simulator-live-chart__trade simulator-live-chart__trade--${tradeTone}${isSelected ? ' simulator-live-chart__trade--selected' : ''}${isHighlighted ? ' simulator-live-chart__trade--highlighted' : ''}`}
              key={`${point.index}-${point.total}-${point.pnl}`}
              onClick={() => onSelectTrade?.(point.trade, point.index)}
            >
              <line
                className="simulator-live-chart__trade-hit"
                x1={pointX}
                y1={plot.top}
                x2={pointX}
                y2={height - plot.bottom}
              />
              <line
                className="simulator-live-chart__trade-range"
                x1={pointX}
                y1={rangeTop}
                x2={pointX}
                y2={rangeBottom}
              />
              <line
                className="simulator-live-chart__trade-range-cap simulator-live-chart__trade-range-cap--high"
                x1={pointX - 6}
                y1={highY}
                x2={pointX + 6}
                y2={highY}
              />
              <line
                className="simulator-live-chart__trade-range-cap simulator-live-chart__trade-range-cap--low"
                x1={pointX - 6}
                y1={lowY}
                x2={pointX + 6}
                y2={lowY}
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
                <text x={tooltipX + 10} y={tooltipY + 36}>
                  {resultLine} / {formatChartMoney(point.pnl)}
                </text>
                <text x={tooltipX + 10} y={tooltipY + 54}>Trade low {formatChartMoney(lowValue)}</text>
                <text x={tooltipX + 10} y={tooltipY + 72}>Trade high {formatChartMoney(highValue)}</text>
                <text x={tooltipX + 10} y={tooltipY + 90}>Test P/L {formatChartMoney(point.total)}</text>
                {totalMode ? (
                  <text x={tooltipX + 10} y={tooltipY + 108}>Trailing DD {formatChartMoney(point.drawdownLevel)}</text>
                ) : null}
                <text x={tooltipX + 10} y={tooltipY + (totalMode ? 122 : 106)}>Exit {exit}</text>
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
          Each dot is the trade result. The vertical wick shows the low/high reached while that trade was open.
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

const formatTradePrice = (value) => {
  const numericValue = Number(value);
  return Number.isFinite(numericValue) ? numericValue.toFixed(2) : 'N/A';
};

const TradeCanvasOverlay = ({ trade = null, selectedPhase1Route = null }) => {
  if (!trade) {
    return null;
  }

  const pnl = Number(trade.pnl);
  const resultR = Number(trade.result_r);
  const exitPrice =
    trade.exit_price ??
    trade.trade_exit_price ??
    (Number(trade.trade_result) === 2
      ? trade.trade_risk_exit_price
      : trade.trade_reward_exit_price);
  const isLoss =
    Boolean(trade.failed_intratrade_drawdown) ||
    Number(trade.trade_result) === 2 ||
    (Number.isFinite(pnl) && pnl < 0) ||
    (Number.isFinite(resultR) && resultR < 0);
  const rows = [
    { label: 'Entry', value: formatTradePrice(trade.trade_enter_price), tone: 'entry' },
    { label: 'Stop', value: formatTradePrice(trade.trade_risk_exit_price), tone: 'stop' },
    { label: 'Target', value: formatTradePrice(trade.trade_reward_exit_price), tone: 'target' },
    { label: 'Exit', value: formatTradePrice(exitPrice), tone: isLoss ? 'stop' : 'target' },
    { label: 'Entry Time', value: formatShortDate(trade.entry_date) },
    { label: 'Exit Time', value: formatShortDate(trade.target_date) },
    {
      label: 'Result',
      value: Number.isFinite(resultR)
        ? `${resultR.toFixed(2)}R`
        : Number.isFinite(pnl)
        ? formatMoney(pnl)
        : 'N/A',
      tone: isLoss ? 'stop' : 'target',
    },
  ];

  return (
    <div className={`simulator-canvas-trade-strip${isLoss ? ' simulator-canvas-trade-strip--loss' : ''}`}>
      <div className="simulator-canvas-trade-strip__head">
        <span>Selected Trade</span>
        <strong>
          {trade.symbol ?? 'N/A'} #{trade.trade_index ?? '-'}
        </strong>
        <small title={selectedPhase1Route?.route_label ?? ''}>
          {selectedPhase1Route?.route_label ?? 'Route replay'}
        </small>
      </div>
      <div className="simulator-canvas-trade-strip__grid">
        {rows.map((row) => (
          <div className={row.tone ? `simulator-canvas-trade-strip__metric simulator-canvas-trade-strip__metric--${row.tone}` : 'simulator-canvas-trade-strip__metric'} key={row.label}>
            <span>{row.label}</span>
            <strong>{row.value}</strong>
          </div>
        ))}
      </div>
    </div>
  );
};

const TradeCanvasDetails = ({
  trade = null,
  tradePattern = null,
  selectedReplayTest = null,
  selectedPhase1Route = null,
  accountRules,
  contracts = 1,
}) => {
  const pnl = Number(trade?.pnl) || 0;
  const closedPnl = Number(trade?.closed_pnl ?? trade?.pnl) || 0;
  const balanceAfter = Number(trade?.balance);
  const hasBalanceAfter = Number.isFinite(balanceAfter);
  const balanceBefore = Number.isFinite(Number(trade?.balance_before))
    ? Number(trade.balance_before)
    : hasBalanceAfter
    ? balanceAfter - pnl
    : null;
  const closedBalance = Number.isFinite(Number(trade?.closed_balance))
    ? Number(trade.closed_balance)
    : hasBalanceAfter
    ? balanceAfter
    : null;
  const intratradeLowBalance = Number.isFinite(Number(trade?.intratrade_low_balance))
    ? Number(trade.intratrade_low_balance)
    : null;
  const intratradeHighBalance = Number.isFinite(Number(trade?.intratrade_high_balance))
    ? Number(trade.intratrade_high_balance)
    : null;
  const testStartingBalance = Number(
    selectedReplayTest?.starting_balance ?? accountRules?.startingBalance
  );
  const runningPnl =
    hasBalanceAfter && Number.isFinite(testStartingBalance)
      ? balanceAfter - testStartingBalance
      : null;
  const profitRemaining =
    Number.isFinite(runningPnl) ? (Number(accountRules?.profitTarget) || 0) - runningPnl : null;
  const drawdown = Number(trade?.drawdown);
  const peakBalance =
    hasBalanceAfter && Number.isFinite(drawdown) ? balanceAfter - drawdown : null;
  const trailingFloor =
    Number.isFinite(peakBalance) ? peakBalance - (Number(accountRules?.maxDrawdown) || 0) : null;
  const drawdownRoom =
    hasBalanceAfter && Number.isFinite(trailingFloor) ? balanceAfter - trailingFloor : null;
  const pointValue = Number(trade?.point_value);
  const contractCount = Math.max(1, Number(contracts) || 1);
  const points =
    Number.isFinite(pointValue) && pointValue > 0 ? closedPnl / (pointValue * contractCount) : null;
  const resultTone = trade?.failed_intratrade_drawdown || pnl < 0 ? 'negative' : 'positive';
  const patternId = tradePattern?.pattern_id ?? trade?.pattern_id ?? 'N/A';
  const routeRows = selectedPhase1Route
    ? [
        { label: 'Rank', value: `#${selectedPhase1Route.result_rank || '-'}` },
        { label: 'Entry', value: formatRouteMode(selectedPhase1Route.entry_mode) },
        { label: 'Stop', value: formatRouteMode(selectedPhase1Route.stop_mode) },
        {
          label: 'Target',
          value: Number.isFinite(Number(selectedPhase1Route.target_r))
            ? `${Number(selectedPhase1Route.target_r).toFixed(2)}R`
            : 'N/A',
        },
        { label: 'Hold', value: `${selectedPhase1Route.max_hold_multiple || '-'}x` },
        {
          label: 'Avg R',
          value: Number.isFinite(Number(selectedPhase1Route.avg_r))
            ? Number(selectedPhase1Route.avg_r).toFixed(3)
            : 'N/A',
        },
        {
          label: 'Win',
          value: Number.isFinite(Number(selectedPhase1Route.win_rate))
            ? `${Number(selectedPhase1Route.win_rate).toFixed(1)}%`
            : 'N/A',
        },
        {
          label: 'PF',
          value: Number.isFinite(Number(selectedPhase1Route.profit_factor))
            ? Number(selectedPhase1Route.profit_factor).toFixed(2)
            : 'N/A',
        },
      ]
    : [];

  if (!trade) {
    return (
      <div className="simulator-trade-detail-rail simulator-trade-detail-rail--empty">
        <div className="simulator-trade-detail-head">
          <span>{tradePattern ? 'Pattern Details' : 'Trade Details'}</span>
          <strong>{tradePattern?.symbol ?? 'Waiting'}</strong>
        </div>

        <div className="simulator-trade-detail-empty-main">
          <span>{tradePattern ? tradePattern.harmonic_type ?? 'XABCD' : selectedReplayTest ? `Test ${selectedReplayTest.test_index}` : 'No Test Loaded'}</span>
          <strong>{tradePattern ? 'XABCD Pattern' : 'No Trade Selected'}</strong>
          <small>
            {tradePattern
              ? `${formatShortDate(tradePattern.x_date)} to ${formatShortDate(tradePattern.d_date)}`
              : 'Run a replay, then select a trade row or graph point.'}
          </small>
        </div>

        <div className="simulator-trade-detail-empty-grid">
          {selectedPhase1Route ? (
            <div className="simulator-trade-detail-id-tile">
              <span>Route</span>
              <strong title={selectedPhase1Route.route_label}>
                {selectedPhase1Route.route_label}
              </strong>
            </div>
          ) : null}
          <div className="simulator-trade-detail-id-tile">
            <span>Pattern ID</span>
            <strong title={patternId}>{patternId}</strong>
          </div>
          <div>
            <span>Market</span>
            <strong>{tradePattern?.market ?? 'N/A'}</strong>
          </div>
          <div>
            <span>Length</span>
            <strong>{tradePattern?.full_pattern_length ?? 'N/A'}</strong>
          </div>
          <div>
            <span>D Price</span>
            <strong>{formatTradePrice(tradePattern?.d_close)}</strong>
          </div>
          <div>
            <span>Contracts</span>
            <strong>{Math.max(1, Number(contracts) || 1)}</strong>
          </div>
        </div>
      </div>
    );
  }

  const priceRows = [
    { label: 'Entry', value: formatTradePrice(trade?.trade_enter_price ?? tradePattern?.trade_enter_price) },
    { label: 'Stop', value: formatTradePrice(trade?.trade_risk_exit_price ?? tradePattern?.trade_risk_exit_price) },
    { label: 'Target', value: formatTradePrice(trade?.trade_reward_exit_price ?? tradePattern?.trade_reward_exit_price) },
    { label: 'Exit', value: formatTradePrice(trade?.exit_price ?? tradePattern?.exit_price) },
    {
      label: 'Low',
      value: formatTradePrice(trade?.trade_lowest_price ?? tradePattern?.trade_lowest_price),
    },
    {
      label: 'High',
      value: formatTradePrice(trade?.trade_highest_price ?? tradePattern?.trade_highest_price),
    },
    {
      label: 'Against',
      value: formatTradePrice(trade?.trade_adverse_price ?? tradePattern?.trade_adverse_price),
    },
    {
      label: 'With',
      value: formatTradePrice(trade?.trade_favorable_price ?? tradePattern?.trade_favorable_price),
    },
  ];
  const accountRows = [
    { label: 'Before', value: Number.isFinite(balanceBefore) ? formatMoney(balanceBefore) : 'N/A' },
    {
      label: 'Low Water',
      value: Number.isFinite(intratradeLowBalance) ? formatMoney(intratradeLowBalance) : 'N/A',
      tone:
        Number.isFinite(intratradeLowBalance) &&
        Number.isFinite(trailingFloor) &&
        intratradeLowBalance <= trailingFloor
          ? 'negative'
          : undefined,
    },
    {
      label: 'High Water',
      value: Number.isFinite(intratradeHighBalance) ? formatMoney(intratradeHighBalance) : 'N/A',
      tone: Number.isFinite(intratradeHighBalance) ? 'positive' : undefined,
    },
    {
      label: 'Close Bal',
      value: Number.isFinite(closedBalance) ? formatMoney(closedBalance) : 'N/A',
    },
    { label: 'After', value: hasBalanceAfter ? formatMoney(balanceAfter) : 'N/A' },
    {
      label: 'Run P/L',
      value: Number.isFinite(runningPnl) ? formatMoney(runningPnl) : 'N/A',
      tone: Number(runningPnl) >= 0 ? 'positive' : 'negative',
    },
    {
      label: 'Target Left',
      value: Number.isFinite(profitRemaining) ? formatMoney(Math.max(0, profitRemaining)) : 'N/A',
    },
    {
      label: 'Trail Floor',
      value: Number.isFinite(trailingFloor) ? formatMoney(trailingFloor) : 'N/A',
    },
    {
      label: 'DD Room',
      value: Number.isFinite(drawdownRoom) ? formatMoney(drawdownRoom) : 'N/A',
      tone: Number(drawdownRoom) <= 0 ? 'negative' : 'positive',
    },
    {
      label: 'Closed P/L',
      value: trade ? formatMoney(closedPnl) : 'N/A',
      tone: closedPnl >= 0 ? 'positive' : 'negative',
    },
  ];

  return (
    <div className="simulator-trade-detail-rail">
      <div className="simulator-trade-detail-head">
        <span>Trade Details</span>
        <strong>{trade?.symbol ?? 'N/A'}</strong>
      </div>

      <div className="simulator-trade-detail-hero">
        <span>
          {trade?.failed_intratrade_drawdown ? 'Drawdown Fail' : trade ? getTradeResultLabel(trade) : 'No Trade'} / Test {trade?.test_index ?? selectedReplayTest?.test_index ?? '-'}
        </span>
        <strong className={trade ? `simulator-value-${resultTone}` : undefined}>
          {trade ? formatMoney(pnl) : formatMoney(0)}
        </strong>
        <small>
          {trade ? `${formatShortDate(trade.entry_date)} to ${formatShortDate(trade.target_date)}` : 'N/A to N/A'}
        </small>
      </div>

      {selectedPhase1Route ? (
        <div className="simulator-trade-detail-section">
          <span title={selectedPhase1Route.route_label}>Route</span>
          <div className="simulator-trade-detail-grid">
            {routeRows.map((row) => (
              <div key={row.label}>
                <span>{row.label}</span>
                <strong>{row.value}</strong>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      <div className="simulator-trade-detail-mini-grid">
        <div>
          <span>Contracts</span>
          <strong>{contractCount}</strong>
        </div>
        <div>
          <span>Points</span>
          <strong>{Number.isFinite(points) ? points.toFixed(2) : 'N/A'}</strong>
        </div>
        <div>
          <span>Point Value</span>
          <strong>{Number.isFinite(pointValue) ? formatMoney(pointValue) : 'N/A'}</strong>
        </div>
        <div className="simulator-trade-detail-id-tile">
          <span>Pattern ID</span>
          <strong title={patternId}>{patternId}</strong>
        </div>
        <div>
          <span>Trade #</span>
          <strong>{trade?.trade_index ?? '-'}</strong>
        </div>
      </div>

      <div className="simulator-trade-detail-section">
        <span>Prices</span>
        <div className="simulator-trade-detail-grid">
          {priceRows.map((row) => (
            <div key={row.label}>
              <span>{row.label}</span>
              <strong>{row.value}</strong>
            </div>
          ))}
        </div>
      </div>

      <div className="simulator-trade-detail-section">
        <span>Account State</span>
        <div className="simulator-trade-detail-grid">
          {accountRows.map((row) => (
            <div key={row.label}>
              <span>{row.label}</span>
              <strong className={row.tone ? `simulator-value-${row.tone}` : undefined}>
                {row.value}
              </strong>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};

const TradeSimulatorPanel = ({
  selectedStrategy = null,
  familyOptions = [],
  familyOptionsCount = null,
  isLoadingFamilyOptions = false,
  onSelectFamilyId = null,
  onReplayChange = null,
  onFirstStartDateChange = null,
  selectedReplayTrade = null,
  onReplayTradeSelect = null,
  highlightedPatternKeys = [],
  loadedTrades = [],
  loadedCandles = [],
  earliestTradeDate = null,
  latestTradeDate = null,
  familyStartDateOptions = [],
  tradeChartData = { candles: [], rust_patterns: null },
  isTradeChartExpanded = false,
  setTradeChartExpanded = () => {},
  chartOverlayTop = 0,
  totalTradeCount = 0,
}) => {
  const [activeTab, setActiveTab] = useState('overview');
  const [familyId, setFamilyId] = useState('');
  const [firstStartDate, setFirstStartDate] = useState('2021-04-26');
  const [testsToChain, setTestsToChain] = useState(1);
  const [contractsPerTrade, setContractsPerTrade] = useState(1);
  const [oneTradeAtATime, setOneTradeAtATime] = useState(true);
  const [useCandidateLogic, setUseCandidateLogic] = useState(false);
  const [accountSize, setAccountSize] = useState('50K');
  const [drawdownModel, setDrawdownModel] = useState('intraday');
  const [pnlMode, setPnlMode] = useState('total');
  const [chartPanelView, setChartPanelView] = useState('pnl');
  const [simulatorReplay, setSimulatorReplay] = useState(null);
  const [selectedReplayTestIndex, setSelectedReplayTestIndex] = useState(1);
  const [playbackEventCount, setPlaybackEventCount] = useState(0);
  const [isPlaybackRunning, setPlaybackRunning] = useState(false);
  const [playbackSpeed, setPlaybackSpeed] = useState('medium');
  const [isRunningReplay, setRunningReplay] = useState(false);
  const [replayError, setReplayError] = useState('');
  const [phase1Routes, setPhase1Routes] = useState([]);
  const [isLoadingPhase1Routes, setLoadingPhase1Routes] = useState(false);
  const [selectedPhase1RouteKey, setSelectedPhase1RouteKey] = useState('');
  const [selectedCanvasTradeKey, setSelectedCanvasTradeKey] = useState('');
  const [selectedCanvasTrade, setSelectedCanvasTrade] = useState(null);
  const [isLoadingTradeCanvas, setLoadingTradeCanvas] = useState(false);
  const [tradeCanvasError, setTradeCanvasError] = useState('');

  const selectedFamilyId = selectedStrategy?.propStrategyId ?? selectedStrategy?.id ?? '';
  const isPhase1FamilyView = selectedStrategy?.outcomeModel === 'phase1-family';
  const isPhase1ReplayMode = isPhase1FamilyView || isPhase1FamilyId(familyId);
  const familyDropdownOptions = useMemo(() => {
    const seen = new Set();
    const options = [];

    familyOptions.forEach((strategy) => {
      const id = getFamilyOptionId(strategy);
      if (!id || seen.has(id)) {
        return;
      }

      seen.add(id);
      options.push(strategy);
    });

    if (familyId && !seen.has(familyId)) {
      options.unshift({
        id: familyId,
        propStrategyId: familyId,
        familyKey: familyId,
      });
    }

    return options;
  }, [familyId, familyOptions]);
  const familyDropdownIds = useMemo(
    () => new Set(familyDropdownOptions.map(getFamilyOptionId)),
    [familyDropdownOptions]
  );
  const hasFamilyDropdownOptions = familyDropdownOptions.length > 0;
  const selectedPhase1Route = useMemo(
    () =>
      phase1Routes.find((route) => getPhase1RouteKey(route) === selectedPhase1RouteKey) ??
      phase1Routes[0] ??
      null,
    [phase1Routes, selectedPhase1RouteKey]
  );
  const familyStartDateSelectOptions = useMemo(() => {
    const seen = new Set();
    const options = [];
    const addDate = (value) => {
      const normalizedDate = normalizeDateInput(value);
      if (!normalizedDate || seen.has(normalizedDate)) {
        return;
      }

      seen.add(normalizedDate);
      options.push(normalizedDate);
    };

    familyStartDateOptions.forEach(addDate);

    if (!options.length) {
      loadedTrades.forEach((trade) => {
        addDate(
          trade.entry_date ??
            trade.reversal_detect_date ??
            trade.d_confirm_date ??
            trade.d_date
        );
      });
    }

    if (!options.length) {
      addDate(earliestTradeDate);
      addDate(latestTradeDate);
      addDate(firstStartDate);
      addDate(SIMULATOR_START_DATE);
    }

    return options.sort();
  }, [earliestTradeDate, familyStartDateOptions, firstStartDate, latestTradeDate, loadedTrades]);
  const familyStartDateOptionSet = useMemo(
    () => new Set(familyStartDateSelectOptions),
    [familyStartDateSelectOptions]
  );

  useEffect(() => {
    setFamilyId(selectedFamilyId);
  }, [onReplayChange, selectedFamilyId]);

  const handleFamilyIdChange = (nextFamilyId) => {
    const normalizedFamilyId = String(nextFamilyId ?? '').trim();
    setFamilyId(normalizedFamilyId);
    if (
      normalizedFamilyId &&
      (familyDropdownIds.has(normalizedFamilyId) || isPhase1FamilyId(normalizedFamilyId))
    ) {
      onSelectFamilyId?.(normalizedFamilyId);
    }
  };
  const accountRules = APEX_ACCOUNT_RULES[accountSize] ?? APEX_ACCOUNT_RULES['50K'];
  const contracts = Math.max(1, Number.parseInt(String(contractsPerTrade), 10) || 1);
  const dailyLossLimit = drawdownModel === 'eod' ? accountRules.eodDailyLossLimit : null;
  const playbackIntervalMs =
    SIMULATOR_PLAYBACK_SPEEDS[playbackSpeed]?.intervalMs ??
    SIMULATOR_PLAYBACK_SPEEDS.medium.intervalMs;

  useEffect(() => {
    setActiveTab('overview');
    setPnlMode('total');
    setChartPanelView('pnl');
    setTestsToChain(1);
    setSimulatorReplay(null);
    setReplayError('');
    setSelectedReplayTestIndex(1);
    setPlaybackEventCount(0);
    setPlaybackRunning(false);
    setSelectedCanvasTradeKey('');
    setSelectedCanvasTrade(null);
    setLoadingTradeCanvas(false);
    setTradeCanvasError('');
    onReplayChange?.(null);
  }, [onReplayChange, selectedFamilyId]);

  useEffect(() => {
    let isCancelled = false;

    const loadPhase1Routes = async () => {
      if (!familyId || !isPhase1ReplayMode) {
        setPhase1Routes([]);
        setSelectedPhase1RouteKey('');
        setLoadingPhase1Routes(false);
        return;
      }

      setLoadingPhase1Routes(true);
      const routes = await fetchPhase1Results({
        familyKey: familyId,
        sourceScope: 'futures',
        limit: 250,
      });

      if (isCancelled) {
        return;
      }

      setPhase1Routes(routes);
      setSelectedPhase1RouteKey((currentKey) => {
        const stillExists = routes.some((route) => getPhase1RouteKey(route) === currentKey);
        return stillExists ? currentKey : getPhase1RouteKey(routes[0] ?? {});
      });
      setLoadingPhase1Routes(false);
    };

    void loadPhase1Routes();

    return () => {
      isCancelled = true;
    };
  }, [familyId, isPhase1ReplayMode]);

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
    }, playbackIntervalMs);

    return () => window.clearInterval(timer);
  }, [isPlaybackRunning, playbackIntervalMs, simulatorReplay]);

  const familyLatestTradeDate = useMemo(
    () =>
      familyStartDateSelectOptions[familyStartDateSelectOptions.length - 1] ||
      normalizeDateInput(latestTradeDate) ||
      normalizeDateInput(firstStartDate) ||
      SIMULATOR_START_DATE,
    [familyStartDateSelectOptions, firstStartDate, latestTradeDate]
  );
  const familyEarliestTradeDate = useMemo(
    () =>
      familyStartDateSelectOptions[0] ||
      normalizeDateInput(earliestTradeDate) ||
      SIMULATOR_START_DATE,
    [earliestTradeDate, familyStartDateSelectOptions]
  );
  const normalizedFirstStartDate = normalizeDateInput(firstStartDate) || firstStartDate;
  const familyDateRangeLabel = `${formatDateOption(familyEarliestTradeDate)} to ${formatDateOption(familyLatestTradeDate)}`;

  useEffect(() => {
    onFirstStartDateChange?.(normalizedFirstStartDate);
  }, [normalizedFirstStartDate, onFirstStartDateChange]);

  const resetSimulatorRunState = useCallback(() => {
    setSimulatorReplay(null);
    setSelectedReplayTestIndex(1);
    setPlaybackEventCount(0);
    setPlaybackRunning(false);
    setSelectedCanvasTradeKey('');
    setSelectedCanvasTrade(null);
    setLoadingTradeCanvas(false);
    setTradeCanvasError('');
    onReplayChange?.(null);
  }, [onReplayChange]);

  const handleFirstStartDateChange = useCallback(
    (nextDate) => {
      setFirstStartDate(nextDate);
      resetSimulatorRunState();
    },
    [resetSimulatorRunState]
  );

  useEffect(() => {
    if (
      !familyStartDateSelectOptions.length ||
      familyStartDateOptionSet.has(normalizedFirstStartDate)
    ) {
      return;
    }

    handleFirstStartDateChange(familyStartDateSelectOptions[0]);
  }, [
    familyId,
    familyStartDateOptionSet,
    familyStartDateSelectOptions,
    handleFirstStartDateChange,
    normalizedFirstStartDate,
  ]);

  const visibleReplayTrades = useMemo(
    () => (simulatorReplay?.trades ?? []).slice(0, playbackEventCount),
    [playbackEventCount, simulatorReplay]
  );
  useEffect(() => {
    if (!simulatorReplay) {
      onReplayChange?.(null);
      return;
    }

    onReplayChange?.({
      ...simulatorReplay,
      trades: visibleReplayTrades,
    });
  }, [onReplayChange, simulatorReplay, visibleReplayTrades]);
  const visibleSelectedReplayTrades = useMemo(
    () => visibleReplayTrades.filter((trade) => trade.test_index === selectedReplayTestIndex),
    [selectedReplayTestIndex, visibleReplayTrades]
  );

  const testReplayPoints = useMemo(
    () =>
      buildServerReplay(
        visibleSelectedReplayTrades,
        accountRules.startingBalance,
        accountRules.maxDrawdown,
        drawdownModel
      ),
    [accountRules.maxDrawdown, accountRules.startingBalance, drawdownModel, visibleSelectedReplayTrades]
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
  const overallRunScore = useMemo(() => {
    const requestedTests = Math.max(1, Number(testsToChain) || 1);
    const completedTests = (simulatorReplay?.tests ?? []).filter((test) =>
      ['passed', 'failed'].includes(test.status)
    );

    if (!completedTests.length) {
      return {
        score: null,
        label: 'Waiting',
        detail: 'Run replay',
        coverage: `${requestedTests} requested`,
        tone: 'waiting',
      };
    }

    const passed = completedTests.filter((test) => test.status === 'passed').length;
    const passRate = passed / requestedTests;
    const drawdownLimit = Math.max(Math.abs(Number(accountRules.maxDrawdown) || 0), 1);
    const averageDrawdownUse =
      completedTests.reduce((total, test) => {
        const drawdownUsed = Math.min(Math.abs(Number(test.max_drawdown) || 0) / drawdownLimit, 1);
        return total + drawdownUsed;
      }, 0) / completedTests.length;
    const averageTrades =
      completedTests.reduce((total, test) => total + Math.max(0, Number(test.trade_count) || 0), 0) /
      completedTests.length;
    const drawdownControl = 1 - averageDrawdownUse;
    const speedScore = averageTrades > 0 ? Math.max(0, 1 - Math.max(0, averageTrades - 3) / 17) : 0;
    const score = Math.max(
      0,
      Math.min(100, Math.round(passRate * 70 + drawdownControl * 20 + speedScore * 10))
    );

    return {
      score,
      label: getScoreLabel(score),
      detail: `${passed}/${requestedTests} passed`,
      coverage: `${completedTests.length}/${requestedTests} scored`,
      tone: getScoreTone(score),
    };
  }, [accountRules.maxDrawdown, simulatorReplay, testsToChain]);
  const displayedFamilyRows =
    Number.isFinite(Number(familyOptionsCount)) && Number(familyOptionsCount) >= 0
      ? Number(familyOptionsCount)
      : Number.isFinite(totalTradeCount) && totalTradeCount >= 0
      ? totalTradeCount
      : loadedTrades.length;
  const familyRowLabel =
    Number.isFinite(Number(familyOptionsCount)) && Number(familyOptionsCount) >= 0
      ? `${displayedFamilyRows.toLocaleString()} families`
      : `${displayedFamilyRows.toLocaleString()} rows`;
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
  const replayButtonDisabled =
    !familyId ||
    isRunningReplay ||
    (isPhase1ReplayMode && (isLoadingPhase1Routes || !selectedPhase1Route));

  const selectedReplayTest = useMemo(
    () =>
      replayTestProgress.find((test) => test.test_index === selectedReplayTestIndex) ??
      replayTestProgress[0] ??
      null,
    [replayTestProgress, selectedReplayTestIndex]
  );
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
  const tradeChartCandles = tradeChartData?.candles ?? EMPTY_CHART_CANDLES;
  const tradeChartPattern = tradeChartData?.rust_patterns ?? null;
  const tradeChartPatternForCanvas = useMemo(
    () => mergeSelectedTradeIntoChartPattern(tradeChartPattern, selectedCanvasTrade, tradeChartCandles),
    [selectedCanvasTrade, tradeChartCandles, tradeChartPattern]
  );
  const tradeChartDataForCanvas = useMemo(
    () => ({
      ...tradeChartData,
      rust_patterns: tradeChartPatternForCanvas,
    }),
    [tradeChartData, tradeChartPatternForCanvas]
  );
  const tradeChartMarket =
    tradeChartPatternForCanvas?.market ?? selectedCanvasTrade?.market ?? selectedStrategy?.market ?? 'Bullish';
  const isUsingSelectedFamily = Boolean(familyId && selectedFamilyId && familyId === selectedFamilyId);

  const restartReplayPlayback = () => {
    if (!simulatorReplay?.trades?.length) {
      return;
    }

    setPlaybackEventCount(0);
    setPlaybackRunning(true);
  };

  const playReplayPlayback = () => {
    if (!simulatorReplay?.trades?.length) {
      return;
    }

    if (isReplayComplete) {
      restartReplayPlayback();
      return;
    }

    setPlaybackRunning(true);
  };

  const pauseReplayPlayback = () => {
    setPlaybackRunning(false);
  };

  const handleSelectReplayTrade = useCallback(
    (trade, pointIndex = '', { notify = true } = {}) => {
      if (!trade) {
        return;
      }

      setSelectedCanvasTradeKey(getSimulatorTradeKey(trade, pointIndex));
      setSelectedCanvasTrade(trade);
      setChartPanelView('canvas');

      const nextTestIndex = Number(trade.test_index);
      if (Number.isFinite(nextTestIndex) && nextTestIndex > 0) {
        setSelectedReplayTestIndex(nextTestIndex);
      }

      if (notify) {
        onReplayTradeSelect?.(trade);
      }
    },
    [onReplayTradeSelect]
  );

  useEffect(() => {
    if (!selectedReplayTrade) {
      return;
    }

    handleSelectReplayTrade(selectedReplayTrade, selectedReplayTrade.trade_index ?? '', {
      notify: false,
    });
  }, [handleSelectReplayTrade, selectedReplayTrade]);

  const runReplay = async () => {
    if (!familyId || isRunningReplay) {
      return;
    }

    if (isPhase1ReplayMode && !selectedPhase1Route) {
      setReplayError(
        isLoadingPhase1Routes
          ? 'Phase 1 routes are still loading.'
          : 'No stored Phase 1 route was found for this family.'
      );
      return;
    }

    const parsedTestsToChain = Math.max(
      1,
      Math.min(250, Number.parseInt(String(testsToChain), 10) || 1)
    );

    setRunningReplay(true);
    setReplayError('');
    const replayAccountRules = {
      ...accountRules,
      dailyLossLimit,
    };
    const result = isPhase1ReplayMode
      ? await fetchPhase1RouteReplay({
          familyKey: familyId,
          runId: selectedPhase1Route.run_id,
          routeId: selectedPhase1Route.route_id,
          firstStartDate,
          testsToChain: parsedTestsToChain,
          contracts,
          accountRules: replayAccountRules,
          drawdownModel,
          oneTradeAtATime,
        })
      : await fetchSimulatorFamilyReplay({
          familyId,
          firstStartDate,
          testsToChain: parsedTestsToChain,
          contracts,
          accountRules: replayAccountRules,
          drawdownModel,
          oneTradeAtATime,
          useCandidateLogic,
        });

    setRunningReplay(false);
    if (!result) {
      setReplayError(
        isPhase1ReplayMode
          ? 'Route replay failed. Check that this route has stored Phase 1 results.'
          : 'Replay failed. Check that the server is running and the family has rows.'
      );
      return;
    }

    setSimulatorReplay(result);
    setSelectedReplayTestIndex(1);
    setPlaybackEventCount(0);
    setPlaybackRunning(Boolean(result.trades?.length));
    setPnlMode('total');
    setChartPanelView('pnl');
    setSelectedCanvasTradeKey('');
    setSelectedCanvasTrade(null);
    setLoadingTradeCanvas(false);
    setTradeCanvasError('');
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
          <div
            className={[
              'simulator-rail-card',
              'simulator-rail-card--hero',
              isUsingSelectedFamily ? 'simulator-rail-card--selected-family' : '',
            ].filter(Boolean).join(' ')}
          >
            <div className="simulator-rail-card-head">
              <span>Setup</span>
              <strong>{isUsingSelectedFamily ? 'Selected' : selectedStrategy?.market ?? 'Family'}</strong>
            </div>
            <label className="simulator-field simulator-field--wide">
              <span>Strategy Family</span>
              {hasFamilyDropdownOptions ? (
                <select
                  value={familyDropdownIds.has(familyId) ? familyId : ''}
                  onChange={(event) => handleFamilyIdChange(event.target.value)}
                  disabled={isLoadingFamilyOptions}
                >
                  {familyDropdownOptions.map((strategy) => {
                    const optionId = getFamilyOptionId(strategy);
                    return (
                      <option value={optionId} key={optionId}>
                        {formatFamilyOptionLabel(strategy)}
                      </option>
                    );
                  })}
                </select>
              ) : (
                <input
                  value={familyId}
                  onChange={(event) => handleFamilyIdChange(event.target.value)}
                  placeholder={isLoadingFamilyOptions ? 'Loading families...' : 'Family ID'}
                  spellCheck="false"
                />
              )}
            </label>
            {hasFamilyDropdownOptions ? (
              <label className="simulator-field simulator-field--wide">
                <span>Family ID</span>
                <input
                  value={familyId}
                  onChange={(event) => handleFamilyIdChange(event.target.value)}
                  placeholder="Paste Phase 1 family ID"
                  spellCheck="false"
                />
              </label>
            ) : null}
            {!hasFamilyDropdownOptions && selectedFamilyId ? (
              <button
                type="button"
                className="simulator-secondary-button"
                onClick={() => handleFamilyIdChange(selectedFamilyId)}
              >
                Use Selected
              </button>
            ) : null}
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
            <label className="simulator-field simulator-date-select">
              <span>First Start</span>
              <select
                value={
                  familyStartDateOptionSet.has(normalizedFirstStartDate)
                    ? normalizedFirstStartDate
                    : familyStartDateSelectOptions[0] ?? ''
                }
                onChange={(event) => handleFirstStartDateChange(event.target.value)}
                disabled={!familyStartDateSelectOptions.length}
              >
                {familyStartDateSelectOptions.map((dateOption) => (
                  <option value={dateOption} key={dateOption}>
                    {formatDateOption(dateOption)}
                  </option>
                ))}
              </select>
              <small>
                {familyDateRangeLabel}
              </small>
            </label>

            {isPhase1ReplayMode ? (
              <label className="simulator-field simulator-date-select">
                <span>Phase 1 Route</span>
                <select
                  value={selectedPhase1Route ? getPhase1RouteKey(selectedPhase1Route) : ''}
                  onChange={(event) => setSelectedPhase1RouteKey(event.target.value)}
                  disabled={isLoadingPhase1Routes || !phase1Routes.length}
                >
                  {phase1Routes.length ? (
                    phase1Routes.map((route) => (
                      <option value={getPhase1RouteKey(route)} key={getPhase1RouteKey(route)}>
                        {formatPhase1RouteOption(route)}
                      </option>
                    ))
                  ) : (
                    <option value="">
                      {isLoadingPhase1Routes ? 'Loading routes...' : 'No stored routes'}
                    </option>
                  )}
                </select>
                <small>
                  {selectedPhase1Route
                    ? `${selectedPhase1Route.route_id} / ${selectedPhase1Route.win_rate.toFixed(1)}% win`
                    : 'Select a stored optimizer route'}
                </small>
              </label>
            ) : null}

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

            {!isPhase1ReplayMode ? (
              <label className="simulator-check simulator-check--switch">
                <input
                  type="checkbox"
                  checked={useCandidateLogic}
                  onChange={(event) => setUseCandidateLogic(event.target.checked)}
                />
                <span>Use Candidate Logic</span>
              </label>
            ) : null}
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
            disabled={replayButtonDisabled}
          >
            {isRunningReplay
              ? `Running ${Number(testsToChain) === 1 ? 'Test' : 'Tests'}...`
              : isPhase1ReplayMode
                ? `Replay Route`
                : `Run ${Number(testsToChain) === 1 ? 'Test' : 'Tests'}`}
          </button>

          <div className="simulator-playback-controls" aria-label="Replay controls">
            <div className="simulator-playback-row">
              <button
                type="button"
                className={!isPlaybackRunning && hasSimulatorReplay ? 'simulator-toggle simulator-toggle--active' : 'simulator-toggle'}
                onClick={playReplayPlayback}
                disabled={!hasSimulatorReplay}
              >
                {isReplayComplete ? 'Replay' : isPlaybackRunning ? 'Playing' : 'Play'}
              </button>
              <button
                type="button"
                className={isPlaybackRunning ? 'simulator-toggle simulator-toggle--active' : 'simulator-toggle'}
                onClick={pauseReplayPlayback}
                disabled={!isPlaybackRunning}
              >
                Pause
              </button>
              <button
                type="button"
                className="simulator-toggle"
                onClick={restartReplayPlayback}
                disabled={!hasSimulatorReplay}
              >
                Restart
              </button>
            </div>
            <div className="simulator-playback-row simulator-playback-row--speed">
              {Object.entries(SIMULATOR_PLAYBACK_SPEEDS).map(([speedKey, speed]) => (
                <button
                  type="button"
                  className={
                    playbackSpeed === speedKey
                      ? 'simulator-toggle simulator-toggle--active'
                      : 'simulator-toggle'
                  }
                  onClick={() => setPlaybackSpeed(speedKey)}
                  key={speedKey}
                >
                  {speed.label}
                </button>
              ))}
            </div>
          </div>

          <div className="simulator-rail-summary simulator-rail-summary--compact">
            <span>{familyRowLabel}</span>
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
                  {hasSimulatorReplay && useCandidateLogic ? (
                    <small>
                      {simulatorReplay.candidate_logic_applied
                        ? `Candidate logic: ${simulatorReplay.candidate_logic_filters.join(', ')}`
                        : 'Candidate logic: no saved avoid rules applied'}
                    </small>
                  ) : null}
                </div>

                <div className="simulator-result-summary-grid">
                  <div className={`simulator-run-score-card simulator-run-score-card--${overallRunScore.tone}`}>
                    <span>{Math.max(1, Number(testsToChain) || 1)} Test Score</span>
                    <strong>
                      {Number.isFinite(overallRunScore.score) ? overallRunScore.score : 'N/A'}
                    </strong>
                    <small>{overallRunScore.detail}</small>
                    <small>{overallRunScore.coverage}</small>
                  </div>
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
                      {chartPanelView === 'canvas'
                        ? 'Trade Canvas'
                        : `Test ${selectedReplayTest?.test_index ?? 1} PnL Replay`}
                    </div>
                    <div className="simulator-chart-subtitle">
                      {chartPanelView === 'canvas'
                        ? selectedCanvasTrade
                          ? `${selectedCanvasTrade.symbol ?? 'N/A'} / ${formatMoney(selectedCanvasTrade.pnl)}`
                          : tradeChartPatternForCanvas
                          ? `${tradeChartPatternForCanvas.symbol ?? 'N/A'} / XABCD pattern`
                          : 'Click a trade row or graph dot to load the candle chart'
                        : hasSimulatorReplay
                        ? `${replaySummary.trades}/${selectedReplayTest?.totalTrades ?? 0} trades shown`
                        : 'Run the test replay to reveal each trade on the graph'}
                    </div>
                  </div>
                  <div className="simulator-toggle-group">
                    <button
                      type="button"
                      className={chartPanelView === 'pnl' ? 'simulator-toggle simulator-toggle--active' : 'simulator-toggle'}
                      onClick={() => setChartPanelView('pnl')}
                    >
                      PnL
                    </button>
                    <button
                      type="button"
                      className={chartPanelView === 'canvas' ? 'simulator-toggle simulator-toggle--active' : 'simulator-toggle'}
                      onClick={() => setChartPanelView('canvas')}
                    >
                      Canvas
                    </button>
                    {chartPanelView === 'pnl' ? (
                      <>
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
                      </>
                    ) : null}
                  </div>
                </div>
                <div className="simulator-chart-canvas-panel">
                  <div className="simulator-trade-canvas-layout">
                    <TradeCanvasDetails
                      trade={selectedCanvasTrade}
                      tradePattern={tradeChartPatternForCanvas}
                      selectedReplayTest={selectedReplayTest}
                      selectedPhase1Route={selectedPhase1Route}
                      accountRules={accountRules}
                      contracts={contracts}
                    />
                    <div className="simulator-trade-canvas-stage">
                      {chartPanelView === 'canvas' ? (
                        tradeChartCandles.length && tradeChartPatternForCanvas && !isLoadingTradeCanvas && !tradeCanvasError ? (
                          <div className="simulator-chart-canvas-shell">
                            <TradeCanvasOverlay
                              trade={selectedCanvasTrade}
                              selectedPhase1Route={selectedPhase1Route}
                            />
                            <CandleChartPanel
                              chartData={tradeChartDataForCanvas}
                              isSectionsExpanded={isTradeChartExpanded}
                              setSectionsExpanded={setTradeChartExpanded}
                              focusMode="prop"
                              market={tradeChartMarket}
                              overlayTopOffset={chartOverlayTop}
                            />
                          </div>
                        ) : (
                          <div className="simulator-chart-canvas-shell simulator-chart-canvas-shell--empty">
                            <div className="simulator-canvas-placeholder">
                              <div className="simulator-canvas-placeholder-grid" />
                            </div>
                          </div>
                        )
                      ) : (
                        <SimulatorChart
                          replayPoints={testReplayPoints}
                          mode={pnlMode}
                          profitTarget={accountRules.profitTarget}
                          maxDrawdown={accountRules.maxDrawdown}
                          showThresholds={pnlMode === 'total'}
                          totalPointCount={Math.max(selectedReplayTest?.totalTrades ?? 0, 1)}
                          selectedTradeKey={selectedCanvasTradeKey}
                          onSelectTrade={handleSelectReplayTrade}
                          highlightedPatternKeys={highlightedPatternKeys}
                        />
                      )}
                    </div>
                  </div>
                </div>
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

            </div>
          ) : null}
        </section>
      </div>
    </div>
  );
};

export default TradeSimulatorPanel;
