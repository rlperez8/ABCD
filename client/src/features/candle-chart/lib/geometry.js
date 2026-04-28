export const clamp = (value, min, max) => Math.min(Math.max(value, min), max);

export const getPriceScale = (chartState) =>
  chartState.price.pixelsPerGrid / chartState.price.unitAmount;

export const getCanvasX = (chartState, candleIndex) =>
  -chartState.candles.completeWidth * candleIndex -
  chartState.viewport.xOrigin +
  chartState.candles.completeWidth / 2;

export const getCanvasY = (chartState, price) =>
  chartState.viewport.baselineY - price * getPriceScale(chartState);

export const getPriceAtCanvasY = (chartState, canvasY) =>
  (chartState.viewport.baselineY - canvasY) / getPriceScale(chartState);

export const getHoveredCandleIndex = (chartState, mouseX) =>
  Math.floor((-(mouseX + chartState.viewport.xOrigin)) / chartState.candles.completeWidth) + 1;

export const formatCandleDate = (value) => value?.split(' 00:')[0] || '';

export const getPatternPivotPoints = (pattern) => {
  if (!pattern) {
    return [];
  }

  const isBearish = pattern.market === 'Bearish';

  return [
    { key: 'x', index: pattern.x, price: parseFloat(isBearish ? pattern.x_high : pattern.x_low) },
    { key: 'a', index: pattern.a, price: parseFloat(isBearish ? pattern.a_low : pattern.a_high) },
    { key: 'b', index: pattern.b, price: parseFloat(isBearish ? pattern.b_high : pattern.b_low) },
    { key: 'c', index: pattern.c, price: parseFloat(isBearish ? pattern.c_low : pattern.c_high) },
    { key: 'd', index: pattern.d, price: parseFloat(isBearish ? pattern.d_high : pattern.d_low) },
  ].filter((point) => Number.isFinite(point.index) && Number.isFinite(point.price));
};

export const getPatternBounds = (pattern) => {
  const points = getPatternPivotPoints(pattern);

  if (!points.length) {
    return null;
  }

  return {
    points,
    minPrice: Math.min(...points.map((point) => point.price)),
    maxPrice: Math.max(...points.map((point) => point.price)),
    minIndex: Math.min(...points.map((point) => point.index)),
    maxIndex: Math.max(...points.map((point) => point.index)),
  };
};

export const getPatternAnchorPoints = (pattern) => {
  const pivotPoints = getPatternPivotPoints(pattern);

  if (!pattern) {
    return pivotPoints;
  }

  return [
    ...pivotPoints,
    { key: 'enter', index: pattern.d, price: parseFloat(pattern.trade_enter_price) },
    { key: 'exit', index: pattern.exit_date, price: parseFloat(pattern.exit_price) },
  ].filter((point) => Number.isFinite(point.index) && Number.isFinite(point.price));
};

export const getPatternCoordinates = (chartState, pattern) => {
  const points = getPatternAnchorPoints(pattern);

  return points.reduce((accumulator, point) => {
    accumulator[point.key] = {
      x: getCanvasX(chartState, point.index),
      y: getCanvasY(chartState, point.price),
    };

    return accumulator;
  }, {});
};
