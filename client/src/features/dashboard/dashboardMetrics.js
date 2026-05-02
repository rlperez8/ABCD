export const METRIC_OPTIONS = ['expectancy', 'avg_return', 'win_rate', 'count', 'closed_rate'];

export const METRIC_META = {
  expectancy: {
    label: 'Expectancy',
    axisLabel: 'Expectancy (%)',
    isPercent: true,
  },
  avg_return: {
    label: 'Avg Return',
    axisLabel: 'Average Return (%)',
    isPercent: true,
  },
  win_rate: {
    label: 'Win %',
    axisLabel: 'Win Rate (%)',
    isPercent: true,
  },
  count: {
    label: 'Closed Trades',
    axisLabel: 'Closed Trades',
    isPercent: false,
  },
  closed_count: {
    label: 'Closed Trades',
    axisLabel: 'Closed Trades',
    isPercent: false,
  },
  closed_rate: {
    label: 'Closed %',
    axisLabel: 'Closed Coverage (%)',
    isPercent: true,
  },
  avg_win: {
    label: 'Avg Win',
    axisLabel: 'Average Win (%)',
    isPercent: true,
  },
  avg_loss: {
    label: 'Avg Loss',
    axisLabel: 'Average Loss (%)',
    isPercent: true,
  },
};

export const getMetricValue = (item, metric) => {
  if (!item) return 0;
  return item[metric] ?? 0;
};

export const getMetricChartValue = (item, metric) => {
  const value = getMetricValue(item, metric);
  return METRIC_META[metric]?.isPercent ? value * 100 : value;
};

export const formatMetricValue = (metric, value, fallback = 'N/A') => {
  if (value == null) return fallback;

  if (METRIC_META[metric]?.isPercent) {
    return `${value.toFixed(2)}%`;
  }

  return `${Math.round(value)}`;
};

export const formatMetricFromItem = (item, metric, fallback = 'N/A') =>
  formatMetricValue(metric, getMetricChartValue(item, metric), fallback);

export const hasClosedTrades = (item) => (item?.count ?? 0) > 0;

export const getTooltipLines = (item) => {
  if (!item) return [];

  const lines = [
    `${item.harmonic_type} | ${item.bin}`,
    `Expectancy: ${hasClosedTrades(item) ? formatMetricFromItem(item, 'expectancy') : 'N/A'}`,
    `Avg Return: ${formatMetricFromItem(item, 'avg_return')}`,
    `Win %: ${hasClosedTrades(item) ? formatMetricFromItem(item, 'win_rate') : 'N/A'}`,
    `Closed %: ${item.total_count ? formatMetricFromItem(item, 'closed_rate') : 'N/A'}`,
    `Closed Trades: ${item.count}/${item.total_count}`,
    `Wins/Losses/Open: ${item.win_count}/${item.loss_count}/${item.open_count}`,
  ];

  if (hasClosedTrades(item)) {
    lines.push(`Avg Win / Loss: ${formatMetricFromItem(item, 'avg_win')} / ${formatMetricFromItem(item, 'avg_loss')}`);
  }

  return lines;
};

export const getSampleBadge = (item) =>
  item ? `Closed ${item.count}/${item.total_count}` : 'No sample';

export const getMetricOptions = (metrics = METRIC_OPTIONS) =>
  metrics.map((metric) => ({
    metric,
    label: METRIC_META[metric]?.label ?? metric,
  }));
