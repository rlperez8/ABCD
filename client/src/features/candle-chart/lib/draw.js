import {
  formatCandleDate,
  clamp,
  getCanvasX,
  getCanvasY,
  getHoveredCandleIndex,
  getPatternCoordinates,
  getPriceAtCanvasY,
} from './geometry.js';

const BULLISH_COLOR = 'rgba(15, 198, 91, 1)';
const BEARISH_COLOR = 'rgb(237, 55, 63)';
const GRID_COLOR = 'rgba(89, 101, 112, 0.18)';
const GRID_MAJOR_COLOR = 'rgba(126, 139, 151, 0.28)';
const CROSSHAIR_COLOR = 'rgba(232, 238, 240, 0.72)';
const PRICE_PANEL_BACKGROUND = 'rgba(16, 18, 19, 0.98)';
const DATE_PANEL_BACKGROUND = 'rgba(16, 18, 19, 0.98)';
const TAG_BORDER_COLOR = 'rgba(116, 128, 135, 0.56)';
const AXIS_TEXT_COLOR = 'rgba(224, 229, 230, 0.82)';
const AXIS_MUTED_TEXT_COLOR = 'rgba(156, 166, 169, 0.68)';
const TARGET_PRICE_GRID_PX = 58;
const RETRACEMENT_COLOR = 'rgba(227, 230, 221, 0.58)';
const LABEL_BACKGROUND = 'rgba(13, 15, 16, 0.94)';
const LABEL_TEXT_COLOR = '#f4f7f7';
const LABEL_BORDER_COLOR = 'rgba(94, 210, 255, 0.34)';
const REVERSAL_D_CANDLE_COLOR = '#f5d742';
const PROP_ENTRY_COLOR = '#ffffff';
const TRADE_ENTRY_COLUMN_COLOR = '#38bdf8';
const TRADE_TARGET_COLOR = '#53f0a7';
const TRADE_STOP_COLOR = '#ff2f3f';
const TRADE_EXIT_COLOR = '#f59e0b';
const TRADE_PROFIT_ZONE = 'rgba(83, 240, 167, 0.14)';
const TRADE_LOSS_ZONE = 'rgba(255, 95, 109, 0.14)';
const TREND_LINE_SERIES = [
  {
    key: 'threeMonth',
    color: '#49c6ff',
    width: 2,
  },
  {
    key: 'sixMonth',
    color: '#f4d35e',
    width: 2,
  },
  {
    key: 'twelveMonth',
    color: '#f08c4f',
    width: 2,
  },
];
const BULLISH_PALETTE = {
  line: 'rgba(112, 231, 205, 0.98)',
  glow: 'rgba(32, 196, 168, 0.28)',
  fill: 'rgba(36, 160, 137, 0.28)',
  node: '#8ef0d8',
  zone: 'rgba(36, 160, 137, 0.24)',
};
const BEARISH_PALETTE = {
  line: 'rgba(255, 167, 167, 0.98)',
  glow: 'rgba(222, 92, 119, 0.28)',
  fill: 'rgba(174, 70, 93, 0.26)',
  node: '#ffc1c1',
  zone: 'rgba(174, 70, 93, 0.24)',
};
const GRAPH_PALETTE = {
  line: '#7dd3fc',
  glow: 'rgba(14, 165, 233, 0.26)',
  fill: 'rgba(56, 189, 248, 0.08)',
  node: '#e0f2fe',
  zone: 'rgba(14, 165, 233, 0.12)',
};

const REVERSAL_SIGNAL_META = {
  bullish_key_reversal: 'Bullish Key Reversal',
  bearish_key_reversal: 'Bearish Key Reversal',
  bullish_engulfing: 'Bullish Engulfing',
  bearish_engulfing: 'Bearish Engulfing',
  bullish_outside_reversal: 'Bullish Outside Reversal',
  bearish_outside_reversal: 'Bearish Outside Reversal',
  hammer: 'Hammer',
  shooting_star: 'Shooting Star',
  morning_star: 'Morning Star',
  evening_star: 'Evening Star',
  three_white_soldiers: 'Three White Soldiers',
  three_black_crows: 'Three Black Crows',
};

const REVERSAL_TYPE_TO_SIGNAL_KEY = {
  BullishKeyReversal: 'bullish_key_reversal',
  BearishKeyReversal: 'bearish_key_reversal',
  BullishEngulfing: 'bullish_engulfing',
  BearishEngulfing: 'bearish_engulfing',
  BullishOutsideReversal: 'bullish_outside_reversal',
  BearishOutsideReversal: 'bearish_outside_reversal',
  Hammer: 'hammer',
  ShootingStar: 'shooting_star',
  MorningStar: 'morning_star',
  EveningStar: 'evening_star',
  ThreeWhiteSoldiers: 'three_white_soldiers',
  ThreeBlackCrows: 'three_black_crows',
};

const isTradeLoss = (pattern) =>
  Number(pattern?.trade_result) === 2 ||
  Number(pattern?.result_r) < 0 ||
  String(pattern?.exit_reason || '').toLowerCase() === 'stop';

const toFinitePrice = (value) => {
  if (value === null || value === undefined || value === '') {
    return NaN;
  }

  const numericValue = Number(value);
  return Number.isFinite(numericValue) ? numericValue : NaN;
};

const firstFinitePrice = (...values) => {
  for (const value of values) {
    const numericValue = toFinitePrice(value);

    if (Number.isFinite(numericValue)) {
      return numericValue;
    }
  }

  return NaN;
};

const getExitReason = (pattern) => String(pattern?.exit_reason || '').toLowerCase();

const isStopExit = (pattern) => {
  const reason = getExitReason(pattern);
  return reason.includes('stop') || (!reason && isTradeLoss(pattern));
};

const isTargetExit = (pattern) => {
  const reason = getExitReason(pattern);
  return reason.includes('target') || reason.includes('take_profit') || reason.includes('profit_target');
};

const formatResultR = (value) => {
  const resultR = Number(value);
  if (!Number.isFinite(resultR)) {
    return '';
  }

  return `${resultR > 0 ? '+' : ''}${resultR.toFixed(2)}R`;
};

const getTradeDirectionLabel = (pattern) => {
  const direction = String(pattern?.trade_direction ?? pattern?.direction ?? '').toUpperCase();
  if (direction === 'LONG' || direction === 'BUY') {
    return 'LONG';
  }

  if (direction === 'SHORT' || direction === 'SELL') {
    return 'SHORT';
  }

  const market = String(pattern?.market ?? '').toLowerCase();
  if (market === 'bullish') {
    return 'LONG';
  }

  if (market === 'bearish') {
    return 'SHORT';
  }

  return 'TRADE';
};

const getTradeResultLabel = (pattern) => {
  const resultR = Number(pattern?.result_r);
  if (Number.isFinite(resultR)) {
    return resultR < 0 ? 'LOSS' : 'PROFIT';
  }

  if (isStopExit(pattern)) {
    return 'STOP';
  }

  if (isTargetExit(pattern)) {
    return 'TARGET';
  }

  return 'EXIT';
};

const getActualExitColor = (pattern) => {
  const resultR = Number(pattern?.result_r);
  if (Number.isFinite(resultR)) {
    return resultR < 0 ? TRADE_STOP_COLOR : TRADE_TARGET_COLOR;
  }

  if (isStopExit(pattern)) {
    return TRADE_STOP_COLOR;
  }

  if (isTargetExit(pattern)) {
    return TRADE_TARGET_COLOR;
  }

  return TRADE_EXIT_COLOR;
};

const getActualExitLabel = (pattern) => {
  const resultLabel = formatResultR(pattern?.result_r);

  if (isStopExit(pattern)) {
    return resultLabel ? `Stop Exit ${resultLabel}` : 'Stop Exit';
  }

  if (isTargetExit(pattern)) {
    return resultLabel ? `Target Hit ${resultLabel}` : 'Target Hit';
  }

  return resultLabel ? `Exit ${resultLabel}` : 'Actual Exit';
};

const getTradeLevelPriority = (level) => {
  switch (level?.key) {
    case 'exit':
      return 4;
    case 'target':
    case 'stop':
      return 3;
    case 'entry':
      return 2;
    default:
      return 1;
  }
};

const dedupeTradeLevelsByY = (levels = [], minPixelGap = 4) =>
  levels.reduce((deduped, level) => {
    if (!Number.isFinite(level?.y)) {
      return deduped;
    }

    const existingIndex = deduped.findIndex((item) => Math.abs(item.y - level.y) <= minPixelGap);
    if (existingIndex < 0) {
      deduped.push(level);
      return deduped;
    }

    const existing = deduped[existingIndex];
    const next =
      getTradeLevelPriority(level) >= getTradeLevelPriority(existing)
        ? level
        : existing;
    const other = next === level ? existing : level;
    deduped[existingIndex] = {
      ...next,
      label:
        next.label && other.label && next.label !== other.label
          ? `${next.label} / ${other.label}`
          : next.label || other.label,
    };
    return deduped;
  }, []);

const normalizeReversalFilter = (activeReversalFilter) =>
  activeReversalFilter?.mode && activeReversalFilter?.value ? activeReversalFilter : null;

const getMatchedReversalLabel = (pattern, activeReversalFilter) => {
  const normalizedFilter = normalizeReversalFilter(activeReversalFilter);

  if (!pattern || !normalizedFilter) {
    return null;
  }

  if (
    normalizedFilter.mode === 'type' &&
    String(pattern.reversal_type ?? 'None') === normalizedFilter.value
  ) {
    return normalizedFilter.label ?? normalizedFilter.value;
  }

  if (normalizedFilter.mode === 'signal' && pattern?.[normalizedFilter.value]) {
    return normalizedFilter.label ?? REVERSAL_SIGNAL_META[normalizedFilter.value] ?? normalizedFilter.value;
  }

  return null;
};

const getMatchedReversalSignalKey = (pattern, activeReversalFilter) => {
  const normalizedFilter = normalizeReversalFilter(activeReversalFilter);

  if (pattern && normalizedFilter) {
    if (normalizedFilter.mode === 'signal' && pattern?.[normalizedFilter.value]) {
      return normalizedFilter.value;
    }

    if (
      normalizedFilter.mode === 'type' &&
      String(pattern.reversal_type ?? 'None') === normalizedFilter.value
    ) {
      return REVERSAL_TYPE_TO_SIGNAL_KEY[normalizedFilter.value] ?? null;
    }
  }

  const primaryType = String(pattern?.reversal_type ?? 'None');
  return REVERSAL_TYPE_TO_SIGNAL_KEY[primaryType] ?? null;
};

const getReversalCandleIndexes = (pattern, activeReversalFilter) => {
  const dIndex = Number(pattern?.d);
  const signalKey = getMatchedReversalSignalKey(pattern, activeReversalFilter);

  if (!Number.isFinite(dIndex) || !signalKey) {
    return new Set();
  }

  if (signalKey === 'morning_star' || signalKey === 'evening_star') {
    return new Set([dIndex - 1, dIndex, dIndex + 1].filter((index) => index >= 1));
  }

  if (signalKey === 'bullish_engulfing' || signalKey === 'bearish_engulfing') {
    return new Set([dIndex - 1, dIndex].filter((index) => index >= 1));
  }

  if (signalKey === 'three_white_soldiers') {
    return new Set([dIndex - 2, dIndex - 1, dIndex].filter((index) => index >= 1));
  }

  if (signalKey === 'three_black_crows') {
    return new Set([dIndex - 2, dIndex - 1, dIndex].filter((index) => index >= 1));
  }

  return new Set([dIndex]);
};

const getCandleStrokeColor = (candle) =>
  candle.candle_close > candle.candle_open ? BULLISH_COLOR : BEARISH_COLOR;

const DAY_MONTH_FORMATTER = new Intl.DateTimeFormat('en-US', {
  month: 'short',
  day: 'numeric',
});

const DAY_MONTH_YEAR_FORMATTER = new Intl.DateTimeFormat('en-US', {
  month: 'short',
  day: 'numeric',
  year: 'numeric',
});

const MONTH_YEAR_FORMATTER = new Intl.DateTimeFormat('en-US', {
  month: 'short',
  year: '2-digit',
});
const YEAR_FORMATTER = new Intl.DateTimeFormat('en-US', {
  year: 'numeric',
});

const getNiceAxisStep = (rawStep) => {
  if (!Number.isFinite(rawStep) || rawStep <= 0) {
    return 1;
  }

  const magnitude = 10 ** Math.floor(Math.log10(rawStep));
  const normalized = rawStep / magnitude;

  if (normalized <= 1) return magnitude;
  if (normalized <= 2) return magnitude * 2;
  if (normalized <= 2.5) return magnitude * 2.5;
  if (normalized <= 5) return magnitude * 5;
  return magnitude * 10;
};

const parseAxisDate = (value) => {
  if (!value) {
    return null;
  }

  const normalized = `${value}`.split(' ')[0];
  const parsed = new Date(`${normalized}T00:00:00`);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
};

const formatAxisDateLabel = (value, completeWidth, gridIncrement = 1) => {
  const parsedDate = parseAxisDate(value);

  if (!parsedDate) {
    return formatCandleDate(value);
  }

  if (completeWidth >= 24 || gridIncrement <= 5) {
    return DAY_MONTH_YEAR_FORMATTER.format(parsedDate);
  }

  if (completeWidth >= 10 || gridIncrement <= 25) {
    return DAY_MONTH_FORMATTER.format(parsedDate);
  }

  if (gridIncrement >= 500) {
    return YEAR_FORMATTER.format(parsedDate);
  }

  return MONTH_YEAR_FORMATTER.format(parsedDate);
};

const formatAxisPrice = (price, unitAmount) => {
  const absoluteUnit = Math.abs(unitAmount);
  let decimals = 2;

  if (absoluteUnit >= 100) {
    decimals = 0;
  } else if (absoluteUnit >= 1) {
    decimals = 2;
  } else if (absoluteUnit >= 0.1) {
    decimals = 3;
  } else if (absoluteUnit >= 0.01) {
    decimals = 4;
  } else {
    decimals = 5;
  }

  return price.toFixed(decimals);
};

const getPriceAxisTicks = (chartState, height) => {
  const priceScale = chartState.price.pixelsPerGrid / chartState.price.unitAmount;

  if (!Number.isFinite(priceScale) || priceScale <= 0) {
    return [];
  }

  const visibleTopPrice = getPriceAtCanvasY(chartState, 0);
  const visibleBottomPrice = getPriceAtCanvasY(chartState, height);
  const minPrice = Math.min(visibleTopPrice, visibleBottomPrice);
  const maxPrice = Math.max(visibleTopPrice, visibleBottomPrice);
  const rawPriceStep = TARGET_PRICE_GRID_PX / priceScale;
  const priceStep = getNiceAxisStep(rawPriceStep);
  const firstPrice = Math.ceil(minPrice / priceStep) * priceStep;
  const ticks = [];

  for (let price = firstPrice; price <= maxPrice + priceStep * 0.5; price += priceStep) {
    const y = getCanvasY(chartState, price);

    if (y >= -1 && y <= height + 1) {
      ticks.push({
        price,
        priceStep,
        y: Math.round(y) + 0.5,
        isBaseline: Math.abs(price) < priceStep * 0.001,
      });
    }

    if (ticks.length > 80) {
      break;
    }
  }

  return ticks;
};

export class Mouse {
  constructor(chartStateRef) {
    this.chartStateRef = chartStateRef;
  }

  mouse_Y = (canvas, ctx) => {
    const chartState = this.chartStateRef.current;
    const mouseY = chartState.mouse.pos.y;

    ctx.save();
    ctx.beginPath();
    ctx.lineWidth = 0.5;
    ctx.strokeStyle = CROSSHAIR_COLOR;
    ctx.setLineDash([5, 5]);
    ctx.moveTo(0, mouseY);
    ctx.lineTo(canvas.width, mouseY);
    ctx.stroke();
    ctx.restore();
  };

  mouse_X = (canvas, ctx, hoveredIndexRef, setHoveredCandle) => {
    const chartState = this.chartStateRef.current;
    const hoveredIndex = getHoveredCandleIndex(chartState, chartState.mouse.pos.x);
    hoveredIndexRef.current = hoveredIndex;

    const pixelStart =
      hoveredIndex * chartState.candles.completeWidth - chartState.candles.completeWidth / 2;
    const guideX = -pixelStart - chartState.viewport.xOrigin;

    ctx.save();
    ctx.beginPath();
    ctx.lineWidth = 0.5;
    ctx.strokeStyle = CROSSHAIR_COLOR;
    ctx.setLineDash([5, 5]);
    ctx.moveTo(guideX, canvas.height);
    ctx.lineTo(guideX, 0);
    ctx.stroke();
    ctx.restore();

    const hoveredCandle = chartState.candles.items[hoveredIndex - 1];
    const nextHoveredCandle = {
      high: hoveredCandle?.candle_high,
      close: hoveredCandle?.candle_close,
      open: hoveredCandle?.candle_open,
      low: hoveredCandle?.candle_low,
      threeMonth: hoveredCandle?.three_month ?? null,
      sixMonth: hoveredCandle?.six_month ?? null,
      twelveMonth: hoveredCandle?.twelve_month ?? null,
      volume: hoveredCandle?.volume,
      color:
        hoveredCandle?.candle_open > hoveredCandle?.candle_close
          ? '#ef5350'
          : hoveredCandle?.candle_open < hoveredCandle?.candle_close
          ? '#26a69a'
          : '',
    };

    setHoveredCandle((previous) => {
      const isUnchanged = Object.entries(nextHoveredCandle).every(
        ([key, value]) => previous?.[key] === value
      );

      return isUnchanged
        ? previous
        : {
            ...previous,
            ...nextHoveredCandle,
          };
    });
  };

  mouse_price = (canvas, ctxPrice) => {
    const chartState = this.chartStateRef.current;
    const price = getPriceAtCanvasY(chartState, chartState.mouse.pos.y);
    const pillHeight = 28;
    const fontSize = Math.max(12, Math.min(14, Math.floor(canvas.width * 0.16)));

    ctxPrice.save();
    ctxPrice.beginPath();
    ctxPrice.fillStyle = PRICE_PANEL_BACKGROUND;
    ctxPrice.strokeStyle = TAG_BORDER_COLOR;
    ctxPrice.lineWidth = 1;
    ctxPrice.roundRect(
      6,
      chartState.mouse.pos.y - pillHeight / 2,
      Math.max(canvas.width - 12, 0),
      pillHeight,
      10
    );
    ctxPrice.fill();
    ctxPrice.stroke();

    ctxPrice.font = `600 ${fontSize}px "Segoe UI"`;
    ctxPrice.fillStyle = '#FFFFFF';
    ctxPrice.textBaseline = 'middle';
    ctxPrice.textAlign = 'center';
    ctxPrice.fillText(price.toFixed(2), Math.floor(canvas.width / 2), chartState.mouse.pos.y);
    ctxPrice.restore();
  };

  price_background = (canvas, ctxPrice) => {
    void canvas;
    void ctxPrice;
  };

  mouse_date = (canvasDate, ctxDate) => {
    const chartState = this.chartStateRef.current;
    const hoveredIndex = getHoveredCandleIndex(chartState, chartState.mouse.pos.x) - 1;
    const hoveredCandle = chartState.candles.items[hoveredIndex];
    const label = formatCandleDate(hoveredCandle?.candle_date);

    if (hoveredIndex < 0 || !label) {
      return;
    }

    const centerX = getCanvasX(chartState, hoveredIndex + 1);
    const fontSize = 12;
    const horizontalPadding = 14;
    ctxDate.save();
    ctxDate.font = `600 ${fontSize}px "Segoe UI"`;
    const metrics = ctxDate.measureText(label);
    const pillWidth = Math.max(96, Math.ceil(metrics.width + horizontalPadding * 2));
    const clampedX = Math.min(
      Math.max(centerX, pillWidth / 2 + 6),
      canvasDate.width - pillWidth / 2 - 6
    );

    ctxDate.fillStyle = '#FFFFFF';
    ctxDate.textBaseline = 'middle';
    ctxDate.textAlign = 'center';
    ctxDate.fillText(label, clampedX, canvasDate.height / 2);
    ctxDate.restore();
  };

  date_background = (ctxDate, canvasDate) => {
    const chartState = this.chartStateRef.current;
    const hoveredIndex = getHoveredCandleIndex(chartState, chartState.mouse.pos.x) - 1;
    const hoveredCandle = chartState.candles.items[hoveredIndex];
    const label = formatCandleDate(hoveredCandle?.candle_date);

    if (hoveredIndex < 0 || !label) {
      return;
    }

    const centerX = getCanvasX(chartState, hoveredIndex + 1);
    const fontSize = 12;
    const horizontalPadding = 14;
    const pillHeight = 30;

    ctxDate.save();
    ctxDate.font = `600 ${fontSize}px "Segoe UI"`;
    const metrics = ctxDate.measureText(label);
    const pillWidth = Math.max(96, Math.ceil(metrics.width + horizontalPadding * 2));
    const pillX = Math.min(
      Math.max(centerX - pillWidth / 2, 6),
      canvasDate.width - pillWidth - 6
    );

    ctxDate.beginPath();
    ctxDate.fillStyle = DATE_PANEL_BACKGROUND;
    ctxDate.strokeStyle = TAG_BORDER_COLOR;
    ctxDate.lineWidth = 1;
    ctxDate.roundRect(
      pillX,
      (canvasDate.height - pillHeight) / 2,
      pillWidth,
      pillHeight,
      10
    );
    ctxDate.fill();
    ctxDate.stroke();
    ctxDate.restore();
  };
}

export class Chart {
  constructor(chartStateRef) {
    this.chartStateRef = chartStateRef;
  }

  graphBackground = (ctx, canvas) => {
    const gradient = ctx.createLinearGradient(0, 0, canvas.width, canvas.height);
    gradient.addColorStop(0, '#0f1b28');
    gradient.addColorStop(1, '#060a0f');

    ctx.save();
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.restore();
  };

  getVisibleCandleRange = (canvasWidth, overscan = 0) => {
    const chartState = this.chartStateRef.current;
    const completeWidth = chartState.candles.completeWidth;
    const leftIndex = Math.max(
      1,
      Math.floor((-(canvasWidth + chartState.viewport.xOrigin)) / completeWidth)
    );
    const rightIndex = Math.min(
      chartState.candles.items.length,
      Math.ceil((-(chartState.viewport.xOrigin - completeWidth)) / completeWidth)
    );

    return {
      start: Math.max(1, Math.min(leftIndex, rightIndex) - overscan),
      end: Math.min(chartState.candles.items.length, Math.max(leftIndex, rightIndex) + overscan),
    };
  };

  getCandleGeometry = (candle, x) => {
    const chartState = this.chartStateRef.current;
    const candleCenterX = Math.round(x) + 0.5;
    const openY = getCanvasY(chartState, candle.candle_open);
    const closeY = getCanvasY(chartState, candle.candle_close);
    const candleTopY = Math.round(Math.min(openY, closeY));
    const candleBottomY = Math.round(Math.max(openY, closeY));
    const candleHeight = Math.max(candleBottomY - candleTopY, 1);
    const candleRenderWidth = Math.max(
      1,
      Math.min(
        Math.round(chartState.candles.width),
        Math.floor(chartState.candles.completeWidth - 1)
      )
    );
    const candleLeftX = Math.round(candleCenterX - candleRenderWidth / 2);
    const highY = Math.round(getCanvasY(chartState, candle.candle_high));
    const lowY = Math.round(getCanvasY(chartState, candle.candle_low));

    return {
      candleCenterX,
      candleRenderWidth,
      candleLeftX,
      candleTopY,
      candleBottomY,
      candleHeight,
      highY,
      lowY,
    };
  };

  grid_X = (ctx, canvas) => {
    const chartState = this.chartStateRef.current;
    const { start, end } = this.getVisibleCandleRange(canvas.width, chartState.viewport.xGridIncrement);
    const gridIncrement = Math.max(chartState.viewport.xGridIncrement, 1);
    const alignedStart = Math.max(gridIncrement, Math.ceil(start / gridIncrement) * gridIncrement);

    ctx.save();
    ctx.beginPath();
    ctx.strokeStyle = GRID_MAJOR_COLOR;
    ctx.lineWidth = 1;

    for (let candleIndex = alignedStart; candleIndex <= end; candleIndex += gridIncrement) {
      const x = Math.round(getCanvasX(chartState, candleIndex)) + 0.5;

      ctx.moveTo(x, 0);
      ctx.lineTo(x, canvas.height);
    }

    ctx.stroke();
    ctx.restore();
  };

  grid_Y = (ctx, canvas) => {
    const chartState = this.chartStateRef.current;
    const ticks = getPriceAxisTicks(chartState, canvas.height);

    ctx.save();
    ctx.beginPath();
    ctx.strokeStyle = GRID_COLOR;
    ctx.lineWidth = 1;

    ticks.forEach((tick) => {
      ctx.moveTo(0, tick.y);
      ctx.lineTo(canvas.width, tick.y);
    });

    ctx.stroke();

    const baselineTick = ticks.find((tick) => tick.isBaseline);
    if (baselineTick) {
      ctx.beginPath();
      ctx.strokeStyle = GRID_MAJOR_COLOR;
      ctx.moveTo(0, baselineTick.y);
      ctx.lineTo(canvas.width, baselineTick.y);
      ctx.stroke();
    }

    ctx.restore();
  };

  prices = (ctxPrice, canvasPrice) => {
    const chartState = this.chartStateRef.current;
    const ticks = getPriceAxisTicks(chartState, canvasPrice.height);

    ctxPrice.save();
    ctxPrice.font = '850 16px "Segoe UI"';
    ctxPrice.fillStyle = AXIS_TEXT_COLOR;
    ctxPrice.textAlign = 'center';
    ctxPrice.textBaseline = 'middle';

    ticks.forEach((tick) => {
      ctxPrice.fillStyle = tick.isBaseline ? AXIS_TEXT_COLOR : AXIS_MUTED_TEXT_COLOR;
      ctxPrice.fillText(
        formatAxisPrice(tick.price, tick.priceStep),
        canvasPrice.width / 2,
        tick.y
      );
    });

    ctxPrice.restore();
  };

  dates = (ctxDate, canvasDate) => {
    const chartState = this.chartStateRef.current;
    const gridIncrement = Math.max(chartState.viewport.xGridIncrement, 1);
    const { start, end } = this.getVisibleCandleRange(canvasDate.width, gridIncrement);
    const alignedStart = Math.max(gridIncrement, Math.ceil(start / gridIncrement) * gridIncrement);
    const tickTop = 4;
    const tickBottom = 11;
    const labelY = Math.floor(canvasDate.height * 0.64);

    ctxDate.save();
    ctxDate.font = '600 13px "Segoe UI"';
    ctxDate.fillStyle = AXIS_MUTED_TEXT_COLOR;
    ctxDate.textAlign = 'center';
    ctxDate.textBaseline = 'middle';
    ctxDate.strokeStyle = 'rgba(93, 118, 156, 0.24)';
    ctxDate.lineWidth = 1;

    for (let candleIndex = alignedStart; candleIndex <= end; candleIndex += gridIncrement) {
      const candle = chartState.candles.items[candleIndex - 1];
      const label = formatAxisDateLabel(
        candle?.candle_date,
        chartState.candles.completeWidth,
        gridIncrement
      );

      if (!label) {
        continue;
      }

      const x = getCanvasX(chartState, candleIndex);

      if (x < -24 || x > canvasDate.width + 24) {
        continue;
      }

      ctxDate.beginPath();
      ctxDate.moveTo(x, tickTop);
      ctxDate.lineTo(x, tickBottom);
      ctxDate.stroke();
      ctxDate.fillText(label, x, labelY);
    }

    ctxDate.restore();
  };

  trend_lines = (ctx, canvas, trendLineToggles = {}) => {
    const chartState = this.chartStateRef.current;
    const start = 1;
    const end = chartState.candles.items.length;

    TREND_LINE_SERIES.forEach((series) => {
      if (!trendLineToggles?.[series.key]) {
        return;
      }

      const values = chartState.candles.trendLines?.[series.key];

      if (!values?.length) {
        return;
      }

      ctx.save();
      ctx.beginPath();
      ctx.strokeStyle = series.color;
      ctx.lineWidth = series.width;

      let hasStartedPath = false;

      for (let candleIndex = start; candleIndex <= end; candleIndex += 1) {
        const value = values[candleIndex - 1];

        if (!Number.isFinite(value)) {
          hasStartedPath = false;
          continue;
        }

        const x = getCanvasX(chartState, candleIndex);
        const y = getCanvasY(chartState, value);

        if (!hasStartedPath) {
          ctx.moveTo(x, y);
          hasStartedPath = true;
          continue;
        }

        ctx.lineTo(x, y);
      }

      ctx.stroke();
      ctx.restore();
    });
  };

  candles = (ctx, activePattern = null, options = {}) => {
    const chartState = this.chartStateRef.current;
    const { start, end } = this.getVisibleCandleRange(chartState.canvas.width, 4);
    const patternPalette =
      activePattern?.market === 'Bearish' ? BEARISH_PALETTE : BULLISH_PALETTE;
    const reversalFocusOnly = Boolean(options?.reversalFocusOnly);
    const reversalIndexes = reversalFocusOnly
      ? getReversalCandleIndexes(activePattern, options?.activeReversalFilter)
      : null;
    const entryIndex = Number(activePattern?.entry);
    const shouldHighlightEntryCandle =
      Boolean(options?.highlightTradeCandles ?? options?.highlightExitCandle) &&
      Number.isFinite(entryIndex) &&
      entryIndex >= 1;
    const exitIndex = Number(activePattern?.exit_date);
    const shouldHighlightExitCandle =
      Boolean(options?.highlightTradeCandles ?? options?.highlightExitCandle) &&
      Number.isFinite(exitIndex) &&
      exitIndex >= 1;
    const entryColor = PROP_ENTRY_COLOR;
    const exitColor = getActualExitColor(activePattern);

    for (let candleIndex = start; candleIndex <= end; candleIndex += 1) {
      const candle = chartState.candles.items[candleIndex - 1];
      const x = getCanvasX(chartState, candleIndex);
      const isReversalSignalCandle = reversalFocusOnly && (reversalIndexes?.has(candleIndex) ?? false);
      const isEntryCandle = shouldHighlightEntryCandle && candleIndex === Math.round(entryIndex);
      const isExitCandle = shouldHighlightExitCandle && candleIndex === Math.round(exitIndex);

      this.drawCandle(ctx, candle, x, {
        isPatternSpan: isReversalSignalCandle,
        isPivot: false,
        isReversalSignalCandle,
        isEntryCandle,
        isExitCandle,
        entryColor,
        exitColor,
        palette: patternPalette,
      });
      this.drawWick(ctx, candle, x, {
        isPatternSpan: isReversalSignalCandle,
        isPivot: false,
        isReversalSignalCandle,
        isEntryCandle,
        isExitCandle,
        entryColor,
        exitColor,
        palette: patternPalette,
      });
    }
  };

  tradeEntryColumn = (ctx, activePattern = null) => {
    const chartState = this.chartStateRef.current;
    const entryIndex = Number(activePattern?.entry);

    if (
      !chartState?.canvas ||
      !Number.isFinite(entryIndex) ||
      entryIndex < 1
    ) {
      return;
    }

    const x = getCanvasX(chartState, entryIndex);
    const bandWidth = Math.max(26, (chartState.candles.completeWidth || 8) * 2.8);
    const leftX = x - bandWidth / 2;
    const label = 'ENTRY';

    ctx.save();
    ctx.fillStyle = 'rgba(56, 189, 248, 0.18)';
    ctx.fillRect(leftX, 0, bandWidth, chartState.canvas.height);

    ctx.strokeStyle = 'rgba(56, 189, 248, 0.96)';
    ctx.lineWidth = 2.4;
    ctx.setLineDash([7, 5]);
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, chartState.canvas.height);
    ctx.stroke();
    ctx.setLineDash([]);

    ctx.strokeStyle = 'rgba(56, 189, 248, 0.48)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(leftX, 0);
    ctx.lineTo(leftX, chartState.canvas.height);
    ctx.moveTo(leftX + bandWidth, 0);
    ctx.lineTo(leftX + bandWidth, chartState.canvas.height);
    ctx.stroke();

    ctx.font = '950 12px "Segoe UI"';
    const labelWidth = Math.max(58, ctx.measureText(label).width + 18);
    const labelHeight = 22;
    const labelX = clamp(x - labelWidth / 2, 8, chartState.canvas.width - labelWidth - 8);
    const labelY = Math.max(8, Math.min(chartState.canvas.height - labelHeight - 8, 68));

    ctx.beginPath();
    ctx.fillStyle = 'rgba(6, 24, 36, 0.92)';
    ctx.strokeStyle = TRADE_ENTRY_COLUMN_COLOR;
    ctx.lineWidth = 1;
    ctx.roundRect(labelX, labelY, labelWidth, labelHeight, 8);
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = '#dff7ff';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(label, labelX + labelWidth / 2, labelY + labelHeight / 2);
    ctx.restore();
  };

  tradeExitColumn = (ctx, activePattern = null) => {
    const chartState = this.chartStateRef.current;
    const exitIndex = Number(activePattern?.exit_date);

    if (
      !chartState?.canvas ||
      !Number.isFinite(exitIndex) ||
      exitIndex < 1
    ) {
      return;
    }

    const x = getCanvasX(chartState, exitIndex);
    const exitColor = getActualExitColor(activePattern);
    const label = getTradeResultLabel(activePattern);
    const bandWidth = Math.max(26, (chartState.candles.completeWidth || 8) * 2.8);
    const leftX = x - bandWidth / 2;
    const isLoss = exitColor === TRADE_STOP_COLOR;
    const isProfit = exitColor === TRADE_TARGET_COLOR;

    ctx.save();
    ctx.fillStyle = isLoss
      ? 'rgba(255, 47, 63, 0.16)'
      : isProfit
        ? 'rgba(83, 240, 167, 0.16)'
        : 'rgba(245, 158, 11, 0.16)';
    ctx.fillRect(leftX, 0, bandWidth, chartState.canvas.height);

    ctx.strokeStyle = exitColor;
    ctx.lineWidth = 2.4;
    ctx.setLineDash([7, 5]);
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, chartState.canvas.height);
    ctx.stroke();
    ctx.setLineDash([]);

    ctx.strokeStyle = isLoss
      ? 'rgba(255, 47, 63, 0.42)'
      : isProfit
        ? 'rgba(83, 240, 167, 0.42)'
        : 'rgba(245, 158, 11, 0.42)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(leftX, 0);
    ctx.lineTo(leftX, chartState.canvas.height);
    ctx.moveTo(leftX + bandWidth, 0);
    ctx.lineTo(leftX + bandWidth, chartState.canvas.height);
    ctx.stroke();

    ctx.font = '950 12px "Segoe UI"';
    const labelWidth = Math.max(58, ctx.measureText(label).width + 18);
    const labelHeight = 22;
    const labelX = clamp(x - labelWidth / 2, 8, chartState.canvas.width - labelWidth - 8);
    const labelY = Math.max(8, Math.min(chartState.canvas.height - labelHeight - 8, 96));

    ctx.beginPath();
    ctx.fillStyle = 'rgba(6, 10, 12, 0.92)';
    ctx.strokeStyle = exitColor;
    ctx.lineWidth = 1;
    ctx.roundRect(labelX, labelY, labelWidth, labelHeight, 8);
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = exitColor;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(label, labelX + labelWidth / 2, labelY + labelHeight / 2);
    ctx.restore();
  };

  drawCandle = (ctx, candle, x, highlight = {}) => {
    const { candleLeftX, candleTopY, candleHeight, candleRenderWidth } = this.getCandleGeometry(
      candle,
      x
    );
    const {
      isPatternSpan = false,
      isPivot = false,
      isReversalSignalCandle = false,
      isEntryCandle = false,
      isExitCandle = false,
      entryColor = null,
      exitColor = null,
      palette = BULLISH_PALETTE,
    } = highlight;
    const isTradeEndpointCandle = isEntryCandle || isExitCandle;
    const endpointColor = isExitCandle && exitColor ? exitColor : entryColor;

    ctx.save();
    ctx.fillStyle = isReversalSignalCandle
      ? REVERSAL_D_CANDLE_COLOR
      : isTradeEndpointCandle && endpointColor
      ? endpointColor
      : getCandleStrokeColor(candle);
    ctx.globalAlpha = isTradeEndpointCandle && !isReversalSignalCandle ? 0.96 : 1;
    ctx.fillRect(candleLeftX, candleTopY, candleRenderWidth, candleHeight);
    ctx.globalAlpha = 1;

    if (isPatternSpan || isTradeEndpointCandle) {
      ctx.strokeStyle = isReversalSignalCandle
        ? REVERSAL_D_CANDLE_COLOR
        : isTradeEndpointCandle && endpointColor
        ? endpointColor
        : isPivot
        ? palette.line
        : 'rgba(220, 234, 255, 0.36)';
      ctx.lineWidth = isTradeEndpointCandle ? 3.2 : isReversalSignalCandle ? 2 : isPivot ? 2 : 1;
      ctx.strokeRect(
        candleLeftX - 0.5,
        candleTopY - 0.5,
        candleRenderWidth + 1,
        candleHeight + 1
      );

      if (isTradeEndpointCandle) {
        ctx.globalAlpha = 0.22;
        ctx.lineWidth = 7;
        ctx.strokeRect(
          candleLeftX - 3,
          candleTopY - 3,
          candleRenderWidth + 6,
          candleHeight + 6
        );
        ctx.globalAlpha = 1;
      }
    }
    ctx.restore();
  };

  drawWick = (ctx, candle, x, highlight = {}) => {
    const { candleCenterX, highY, lowY, candleTopY, candleBottomY } = this.getCandleGeometry(
      candle,
      x
    );
    const {
      isPatternSpan = false,
      isPivot = false,
      isReversalSignalCandle = false,
      isEntryCandle = false,
      isExitCandle = false,
      entryColor = null,
      exitColor = null,
      palette = BULLISH_PALETTE,
    } = highlight;
    const isTradeEndpointCandle = isEntryCandle || isExitCandle;
    const endpointColor = isExitCandle && exitColor ? exitColor : entryColor;

    ctx.save();
    ctx.strokeStyle = isReversalSignalCandle
      ? REVERSAL_D_CANDLE_COLOR
      : isTradeEndpointCandle && endpointColor
      ? endpointColor
      : isPatternSpan && isPivot
      ? palette.line
      : getCandleStrokeColor(candle);
    ctx.lineWidth = isTradeEndpointCandle ? 2 : isReversalSignalCandle ? 2 : isPatternSpan && isPivot ? 1.5 : 1;

    if (highY < candleTopY) {
      ctx.beginPath();
      ctx.moveTo(candleCenterX, highY);
      ctx.lineTo(candleCenterX, candleTopY);
      ctx.stroke();
    }

    if (lowY > candleBottomY) {
      ctx.beginPath();
      ctx.moveTo(candleCenterX, candleBottomY);
      ctx.lineTo(candleCenterX, lowY);
      ctx.stroke();
    }

    if (isPivot) {
      ctx.beginPath();
      ctx.fillStyle = palette.node;
      ctx.arc(candleCenterX, highY - 6, 2.5, 0, Math.PI * 2);
      ctx.fill();
    }

    ctx.restore();
  };
}

export class ABCD {
  constructor(chartStateRef) {
    this.chartStateRef = chartStateRef;
  }

  getOverlayPalette = (pattern) =>
    pattern?.market === 'Bearish' ? BEARISH_PALETTE : BULLISH_PALETTE;

  drawPath = (ctx, coordinates, palette, includeFill = true) => {
    const { x, a, b, c, d, exit } = coordinates;

    if (includeFill) {
      ctx.beginPath();
      ctx.moveTo(x.x, x.y);
      ctx.lineTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.lineTo(c.x, c.y);
      ctx.lineTo(d.x, d.y);
      ctx.closePath();
      ctx.fillStyle = palette.fill;
      ctx.fill();
    }

    ctx.beginPath();
    ctx.moveTo(x.x, x.y);
    ctx.lineTo(a.x, a.y);
    ctx.lineTo(b.x, b.y);
    ctx.lineTo(c.x, c.y);
    ctx.lineTo(d.x, d.y);
    ctx.lineTo(exit.x, exit.y);
  };

  drawPatternNodes = (ctx, coordinates, palette) => {
    const orderedPoints = [coordinates.x, coordinates.a, coordinates.b, coordinates.c, coordinates.d];

    orderedPoints.forEach((point) => {
      ctx.beginPath();
      ctx.fillStyle = palette.node;
      ctx.arc(point.x, point.y, 4.5, 0, Math.PI * 2);
      ctx.fill();

      ctx.beginPath();
      ctx.strokeStyle = 'rgba(9, 15, 24, 0.9)';
      ctx.lineWidth = 2;
      ctx.arc(point.x, point.y, 4.5, 0, Math.PI * 2);
      ctx.stroke();
    });
  };

  drawSetupPath = (ctx, coordinates, palette, includeFill = true) => {
    const { x, a, b, c, d } = coordinates;

    if (!x || !a || !b || !c || !d) {
      return;
    }

    if (includeFill) {
      ctx.beginPath();
      ctx.moveTo(x.x, x.y);
      ctx.lineTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.lineTo(c.x, c.y);
      ctx.lineTo(d.x, d.y);
      ctx.closePath();
      ctx.fillStyle = palette.fill;
      ctx.fill();
    }

    ctx.beginPath();
    ctx.moveTo(x.x, x.y);
    ctx.lineTo(a.x, a.y);
    ctx.lineTo(b.x, b.y);
    ctx.lineTo(c.x, c.y);
    ctx.lineTo(d.x, d.y);
  };

  drawSetupNodes = (ctx, coordinates, palette, options = {}) => {
    const orderedPoints = [coordinates.x, coordinates.a, coordinates.b, coordinates.c, coordinates.d].filter(Boolean);
    const radius = options?.radius ?? 4.5;
    const haloRadius = options?.haloRadius ?? 0;

    orderedPoints.forEach((point) => {
      if (haloRadius > 0) {
        ctx.beginPath();
        ctx.fillStyle = options?.haloColor ?? palette.glow;
        ctx.arc(point.x, point.y, haloRadius, 0, Math.PI * 2);
        ctx.fill();
      }

      ctx.beginPath();
      ctx.fillStyle = palette.node;
      ctx.arc(point.x, point.y, radius, 0, Math.PI * 2);
      ctx.fill();

      ctx.beginPath();
      ctx.strokeStyle = options?.strokeStyle ?? 'rgba(9, 15, 24, 0.9)';
      ctx.lineWidth = options?.strokeWidth ?? 2;
      ctx.arc(point.x, point.y, radius, 0, Math.PI * 2);
      ctx.stroke();
    });
  };

  drawGraphLabel = (ctx, label, point) => {
    const pillHeight = 42;
    const pillPadding = 16;

    ctx.save();
    ctx.font = '950 22px "Segoe UI"';
    const pillWidth = Math.max(28, ctx.measureText(label).width + pillPadding * 2);
    const pillX = clamp(point.x - pillWidth / 2, 8, Math.max(8, ctx.canvas.width - pillWidth - 8));
    const preferredY = point.y - pillHeight - 20;
    const pillY = preferredY < 8 ? point.y + 20 : preferredY;

    ctx.beginPath();
    ctx.fillStyle = 'rgba(8, 15, 26, 0.9)';
    ctx.strokeStyle = 'rgba(125, 211, 252, 0.42)';
    ctx.lineWidth = 1;
    ctx.roundRect(pillX, pillY, pillWidth, pillHeight, 11);
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = '#f8fafc';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(label, pillX + pillWidth / 2, pillY + pillHeight / 2);
    ctx.restore();
  };

  drawPatternOverlay = (ctx, pattern, { withArrow = true } = {}) => {
    const coordinates = getPatternCoordinates(this.chartStateRef.current, pattern);
    const palette = this.getOverlayPalette(pattern);

    ctx.save();
    this.drawPath(ctx, coordinates, palette, true);
    ctx.strokeStyle = palette.glow;
    ctx.lineWidth = 10;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.stroke();

    this.drawPath(ctx, coordinates, palette, false);
    ctx.strokeStyle = palette.line;
    ctx.lineWidth = 4;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.stroke();
    this.drawPatternNodes(ctx, coordinates, palette);
    ctx.restore();

    if (withArrow) {
      this.drawArrowHead(ctx, coordinates.d, coordinates.exit, palette);
    }
  };

  drawSetupOverlay = (ctx, pattern, options = {}) => {
    const coordinates = getPatternCoordinates(this.chartStateRef.current, pattern);
    const isGraphPresentation = options?.presentationMode === 'graph';
    const palette = isGraphPresentation ? GRAPH_PALETTE : this.getOverlayPalette(pattern);
    const glowWidth = isGraphPresentation ? 18 : 10;
    const lineWidth = isGraphPresentation ? 4.5 : 4;
    const nodeRadius = isGraphPresentation ? 6.5 : 4.5;

    if (!coordinates?.x || !coordinates?.a || !coordinates?.b || !coordinates?.c || !coordinates?.d) {
      return;
    }

    ctx.save();
    this.drawSetupPath(ctx, coordinates, palette, true);
    ctx.strokeStyle = palette.glow;
    ctx.lineWidth = glowWidth;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.stroke();

    this.drawSetupPath(ctx, coordinates, palette, false);
    ctx.strokeStyle = palette.line;
    ctx.lineWidth = lineWidth;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.stroke();
    this.drawSetupNodes(ctx, coordinates, palette, {
      radius: nodeRadius,
      strokeWidth: isGraphPresentation ? 3 : 2,
      strokeStyle: isGraphPresentation ? 'rgba(8, 15, 26, 0.95)' : undefined,
      haloRadius: isGraphPresentation ? 12 : 0,
      haloColor: isGraphPresentation ? 'rgba(125, 211, 252, 0.2)' : undefined,
    });
    if (isGraphPresentation) {
      this.drawGraphLabel(ctx, 'X', coordinates.x);
      this.drawGraphLabel(ctx, 'A', coordinates.a);
      this.drawGraphLabel(ctx, 'B', coordinates.b);
      this.drawGraphLabel(ctx, 'C', coordinates.c);
      this.drawGraphLabel(ctx, 'D', coordinates.d);
    } else {
      this.drawLabel(ctx, 'X', coordinates.x);
      this.drawLabel(ctx, 'A', coordinates.a);
      this.drawLabel(ctx, 'B', coordinates.b);
      this.drawLabel(ctx, 'C', coordinates.c);
      this.drawLabel(ctx, 'D', coordinates.d);
    }
    ctx.restore();
  };

  bull_abcd = (ctx, pattern) => {
    this.drawPatternOverlay(ctx, pattern, { withArrow: true });
  };

  abcd = (ctx, pattern) => {
    this.drawPatternOverlay(ctx, pattern, { withArrow: true });
  };

  drawArrowHead = (ctx, fromPoint, toPoint, palette) => {
    const arrowLength = 20;
    const arrowAngle = Math.PI / 6;
    const angle = Math.atan2(toPoint.y - fromPoint.y, toPoint.x - fromPoint.x);

    ctx.save();
    ctx.beginPath();
    ctx.moveTo(toPoint.x, toPoint.y);
    ctx.lineTo(
      toPoint.x - arrowLength * Math.cos(angle - arrowAngle),
      toPoint.y - arrowLength * Math.sin(angle - arrowAngle)
    );
    ctx.lineTo(
      toPoint.x - arrowLength * Math.cos(angle + arrowAngle),
      toPoint.y - arrowLength * Math.sin(angle + arrowAngle)
    );
    ctx.lineTo(toPoint.x, toPoint.y);
    ctx.closePath();
    ctx.fillStyle = palette.line;
    ctx.fill();
    ctx.restore();
  };

  drawEventMarker = (
    ctx,
    pattern,
    { index, label, color, guideFromIndex, stackOrder = 0, showStem = true }
  ) => {
    const chartState = this.chartStateRef.current;
    const markerIndex = Number(index);

    if (!Number.isFinite(markerIndex) || markerIndex < 1 || !label) {
      return;
    }

    const x = getCanvasX(chartState, markerIndex);
    const candle = chartState?.candles?.items?.[markerIndex - 1];
    const candleGeometry = candle ? new Chart(this.chartStateRef).getCandleGeometry(candle, x) : null;
    const markerTop = Math.max(
      18,
      (candleGeometry ? Math.min(candleGeometry.highY, candleGeometry.candleTopY) : 44) - 44 - stackOrder * 30
    );
    const pillHeight = 24;
    const pillPadding = 18;

    ctx.save();
    ctx.font = '600 12px "Segoe UI"';
    const textWidth = ctx.measureText(label).width;
    const pillWidth = textWidth + pillPadding;
    const pillX = Math.min(
      Math.max(x - pillWidth / 2, 8),
      chartState.canvas.width - pillWidth - 8
    );

    if (Number.isFinite(guideFromIndex) && guideFromIndex >= 1 && guideFromIndex !== markerIndex) {
      const guideX = getCanvasX(chartState, Number(guideFromIndex));
      const guideY = markerTop + pillHeight / 2;
      ctx.beginPath();
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.5;
      ctx.setLineDash([4, 4]);
      ctx.moveTo(guideX, guideY);
      ctx.lineTo(x, guideY);
      ctx.stroke();
      ctx.setLineDash([]);
    }

    if (showStem && candleGeometry) {
      ctx.beginPath();
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.5;
      ctx.moveTo(x, markerTop + pillHeight);
      ctx.lineTo(x, Math.min(candleGeometry.highY, candleGeometry.candleTopY) - 6);
      ctx.stroke();
    }

    ctx.beginPath();
    ctx.fillStyle = LABEL_BACKGROUND;
    ctx.strokeStyle = color;
    ctx.lineWidth = 1;
    ctx.roundRect(pillX, markerTop, pillWidth, pillHeight, 12);
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = LABEL_TEXT_COLOR;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(label, pillX + pillWidth / 2, markerTop + pillHeight / 2);
    ctx.restore();
  };

  retracement = (ctx, pattern) => {
    const { x, a, b, c, d } = getPatternCoordinates(this.chartStateRef.current, pattern);

    ctx.save();
    ctx.beginPath();
    ctx.strokeStyle = RETRACEMENT_COLOR;
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 4]);
    ctx.moveTo(a.x, a.y);
    ctx.lineTo(c.x, c.y);
    ctx.moveTo(x.x, x.y);
    ctx.lineTo(b.x, b.y);
    ctx.moveTo(b.x, b.y);
    ctx.lineTo(d.x, d.y);
    ctx.moveTo(x.x, x.y);
    ctx.lineTo(d.x, d.y);
    ctx.stroke();

    this.drawLabel(ctx, 'X', x);
    this.drawLabel(ctx, 'A', a);
    this.drawLabel(ctx, 'B', b);
    this.drawLabel(ctx, 'C', c);
    this.drawLabel(ctx, 'D', d);
    this.drawLabelMid(ctx, 'AC', a, c);
    this.drawLabelMid(ctx, 'XB', x, b);
    this.drawLabelMid(ctx, 'BD', b, d);
    this.drawLabelMid(ctx, 'XD', x, d);
    ctx.restore();
  };

  route_logic_highlight = (ctx, pattern, activeKey) => {
    if (activeKey !== 'entry') {
      return;
    }

    const chartState = this.chartStateRef.current;
    const entryIndex = Number(pattern?.entry);
    const entryPrice = Number(pattern?.trade_enter_price);

    if (
      !chartState?.canvas ||
      !Number.isFinite(entryIndex) ||
      entryIndex < 1 ||
      !Number.isFinite(entryPrice)
    ) {
      return;
    }

    const x = getCanvasX(chartState, entryIndex);
    const y = getCanvasY(chartState, entryPrice);
    const candle = chartState?.candles?.items?.[Math.round(entryIndex) - 1];
    const geometry = candle ? new Chart(this.chartStateRef).getCandleGeometry(candle, x) : null;
    const bandWidth = Math.max(28, (chartState.candles.completeWidth || 8) * 1.8);
    const labelText = 'ROUTE ENTRY';

    ctx.save();
    ctx.fillStyle = 'rgba(94, 214, 255, 0.14)';
    ctx.fillRect(x - bandWidth / 2, 0, bandWidth, chartState.canvas.height);

    ctx.strokeStyle = 'rgba(94, 214, 255, 0.95)';
    ctx.lineWidth = 2;
    ctx.setLineDash([5, 5]);
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, chartState.canvas.height);
    ctx.stroke();
    ctx.setLineDash([]);

    if (geometry) {
      const top = Math.max(2, Math.min(geometry.highY, geometry.candleTopY) - 8);
      const height = Math.min(
        chartState.canvas.height - top - 2,
        Math.max(24, Math.max(geometry.lowY, geometry.candleBottomY) - top + 8)
      );

      ctx.strokeStyle = 'rgba(255, 255, 255, 0.94)';
      ctx.lineWidth = 2.5;
      ctx.strokeRect(
        geometry.candleLeftX - 3,
        top,
        geometry.candleRenderWidth + 6,
        height
      );
    }

    ctx.beginPath();
    ctx.fillStyle = '#ffffff';
    ctx.strokeStyle = 'rgba(7, 55, 71, 0.95)';
    ctx.lineWidth = 3;
    ctx.arc(x, y, 7, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();

    ctx.font = '900 13px "Segoe UI"';
    const labelWidth = Math.max(112, ctx.measureText(labelText).width + 24);
    const labelHeight = 28;
    const labelX = clamp(x - labelWidth / 2, 8, chartState.canvas.width - labelWidth - 8);
    const labelY = clamp(y - labelHeight - 18, 8, chartState.canvas.height - labelHeight - 8);

    ctx.beginPath();
    ctx.fillStyle = 'rgba(5, 8, 8, 0.86)';
    ctx.strokeStyle = 'rgba(94, 214, 255, 0.86)';
    ctx.roundRect(labelX, labelY, labelWidth, labelHeight, 10);
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = '#dff7ff';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(labelText, labelX + labelWidth / 2, labelY + labelHeight / 2);
    ctx.restore();
  };

  xa_scan_start_beam = (ctx, pattern) => {
    if (!pattern?.xa_canvas_mode) {
      return;
    }

    const chartState = this.chartStateRef.current;
    const scanStartIndex = Number(pattern?.xa_scan_start);

    if (!chartState?.canvas || !Number.isFinite(scanStartIndex) || scanStartIndex < 1) {
      return;
    }

    const x = getCanvasX(chartState, scanStartIndex);
    const candle = chartState?.candles?.items?.[Math.round(scanStartIndex) - 1];
    const geometry = candle ? new Chart(this.chartStateRef).getCandleGeometry(candle, x) : null;
    const bandWidth = Math.max(30, (chartState.candles.completeWidth || 8) * 2.15);
    const labelText = 'SCAN START';

    ctx.save();
    ctx.fillStyle = 'rgba(250, 204, 21, 0.14)';
    ctx.fillRect(x - bandWidth / 2, 0, bandWidth, chartState.canvas.height);

    ctx.strokeStyle = 'rgba(250, 204, 21, 0.96)';
    ctx.lineWidth = 2.4;
    ctx.setLineDash([6, 5]);
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, chartState.canvas.height);
    ctx.stroke();
    ctx.setLineDash([]);

    if (geometry) {
      const top = Math.max(2, Math.min(geometry.highY, geometry.candleTopY) - 9);
      const height = Math.min(
        chartState.canvas.height - top - 2,
        Math.max(28, Math.max(geometry.lowY, geometry.candleBottomY) - top + 9)
      );

      ctx.strokeStyle = 'rgba(255, 255, 255, 0.96)';
      ctx.lineWidth = 2.7;
      ctx.strokeRect(
        geometry.candleLeftX - 4,
        top,
        geometry.candleRenderWidth + 8,
        height
      );
    }

    ctx.font = '950 13px "Segoe UI"';
    const labelWidth = Math.max(112, ctx.measureText(labelText).width + 24);
    const labelHeight = 28;
    const labelX = clamp(x - labelWidth / 2, 8, chartState.canvas.width - labelWidth - 8);
    const labelY = 10;

    ctx.beginPath();
    ctx.fillStyle = 'rgba(20, 18, 7, 0.9)';
    ctx.strokeStyle = 'rgba(250, 204, 21, 0.86)';
    ctx.roundRect(labelX, labelY, labelWidth, labelHeight, 10);
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = '#fef9c3';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(labelText, labelX + labelWidth / 2, labelY + labelHeight / 2);
    ctx.restore();
  };

  reversal_signal = (ctx, pattern, activeReversalFilter) => {
    const matchedLabel = getMatchedReversalLabel(pattern, activeReversalFilter);

    if (!matchedLabel || !Number.isFinite(pattern?.d)) {
      return;
    }

    const chartState = this.chartStateRef.current;
    const dIndex = pattern.d;
    const candle = chartState?.candles?.items?.[dIndex - 1];

    if (!candle) {
      return;
    }

    const x = getCanvasX(chartState, dIndex);
    const { candleLeftX, candleTopY, candleRenderWidth, candleHeight, highY, lowY } =
      new Chart(this.chartStateRef).getCandleGeometry(candle, x);
    const palette = this.getOverlayPalette(pattern);
    const highlightTop = Math.min(highY, candleTopY) - 10;
    const highlightBottom = Math.max(lowY, candleTopY + candleHeight) + 10;
    const highlightHeight = Math.max(highlightBottom - highlightTop, candleHeight + 20);
    const highlightX = candleLeftX - 8;
    const highlightWidth = candleRenderWidth + 16;
    const labelY = Math.max(18, highlightTop - 12);

    ctx.save();
    ctx.beginPath();
    ctx.strokeStyle = palette.line;
    ctx.lineWidth = 2;
    ctx.setLineDash([6, 4]);
    ctx.roundRect(highlightX, highlightTop, highlightWidth, highlightHeight, 12);
    ctx.stroke();

    ctx.setLineDash([]);
    ctx.beginPath();
    ctx.fillStyle = palette.zone;
    ctx.roundRect(highlightX, highlightTop, highlightWidth, highlightHeight, 12);
    ctx.fill();

    ctx.font = '600 12px "Segoe UI"';
    const textWidth = ctx.measureText(matchedLabel).width;
    const pillWidth = textWidth + 18;
    const pillHeight = 24;
    const pillX = Math.min(
      Math.max(highlightX + highlightWidth / 2 - pillWidth / 2, 8),
      chartState.canvas.width - pillWidth - 8
    );

    ctx.beginPath();
    ctx.fillStyle = LABEL_BACKGROUND;
    ctx.strokeStyle = palette.line;
    ctx.lineWidth = 1;
    ctx.roundRect(pillX, labelY, pillWidth, pillHeight, 12);
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = LABEL_TEXT_COLOR;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(matchedLabel, pillX + pillWidth / 2, labelY + pillHeight / 2);
    ctx.restore();
  };

  trade_path = (ctx, pattern) => {
    const chartState = this.chartStateRef.current;
    const entryIndex = Number(pattern?.entry);
    const exitIndex = Number(pattern?.exit_date);
    const entryPrice = toFinitePrice(pattern?.trade_enter_price);
    const exitPrice = firstFinitePrice(pattern?.exit_price, pattern?.trade_current_price);

    if (
      !Number.isFinite(entryIndex) ||
      !Number.isFinite(exitIndex) ||
      entryIndex < 1 ||
      exitIndex < 1 ||
      !Number.isFinite(entryPrice) ||
      !Number.isFinite(exitPrice)
    ) {
      return;
    }

    const entryPoint = {
      x: getCanvasX(chartState, entryIndex),
      y: getCanvasY(chartState, entryPrice),
    };
    const exitPoint = {
      x: getCanvasX(chartState, exitIndex),
      y: getCanvasY(chartState, exitPrice),
    };
    const pathColor = getActualExitColor(pattern);

    ctx.save();
    ctx.beginPath();
    ctx.strokeStyle = pathColor;
    ctx.lineWidth = 3;
    ctx.setLineDash([8, 6]);
    ctx.moveTo(entryPoint.x, entryPoint.y);
    ctx.lineTo(exitPoint.x, exitPoint.y);
    ctx.stroke();
    ctx.setLineDash([]);

    [entryPoint, exitPoint].forEach((point, index) => {
      ctx.beginPath();
      ctx.fillStyle = index === 0 ? PROP_ENTRY_COLOR : pathColor;
      ctx.strokeStyle = LABEL_BACKGROUND;
      ctx.lineWidth = 2;
      ctx.arc(point.x, point.y, index === 0 ? 5 : 6, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    });
    ctx.restore();
  };

  prop_events = (ctx, pattern) => {
    const entryIndex = Number(pattern?.entry);
    const exitIndex = Number(pattern?.exit_date);

    if (Number.isFinite(entryIndex) && entryIndex >= 1) {
      this.drawEventMarker(ctx, pattern, {
        index: entryIndex,
        label: 'Entry',
        color: PROP_ENTRY_COLOR,
        stackOrder: 1,
      });
    }

    if (Number.isFinite(exitIndex) && exitIndex >= 1) {
      const exitColor = getActualExitColor(pattern);
      this.drawEventMarker(ctx, pattern, {
        index: exitIndex,
        label: getActualExitLabel(pattern),
        color: exitColor,
        stackOrder: 0,
        showStem: false,
      });
    }
  };

  trade_summary_badge = (ctx, canvas, pattern) => {
    const directionLabel = getTradeDirectionLabel(pattern);
    const resultLabel = getTradeResultLabel(pattern);
    const resultR = formatResultR(pattern?.result_r);
    const exitReason = getActualExitLabel(pattern);
    const accentColor = getActualExitColor(pattern);
    const entryPrice = toFinitePrice(pattern?.trade_enter_price);
    const exitPrice = firstFinitePrice(
      pattern?.exit_price,
      pattern?.target_close,
      pattern?.trade_current_price
    );
    const primaryText = [directionLabel, resultLabel, resultR].filter(Boolean).join('  ');
    const secondaryItems = [
      exitReason,
      Number.isFinite(entryPrice) ? `Entry ${entryPrice.toFixed(2)}` : '',
      Number.isFinite(exitPrice) ? `Exit ${exitPrice.toFixed(2)}` : '',
    ].filter(Boolean);
    const secondaryText = secondaryItems.join(' | ');

    if (!primaryText || !canvas?.width) {
      return;
    }

    ctx.save();
    ctx.font = '950 18px "Segoe UI"';
    const primaryWidth = ctx.measureText(primaryText).width;
    ctx.font = '800 12px "Segoe UI"';
    const secondaryWidth = secondaryText ? ctx.measureText(secondaryText).width : 0;
    const badgeWidth = Math.min(
      canvas.width - 24,
      Math.max(190, primaryWidth + 42, secondaryWidth + 28)
    );
    const badgeHeight = secondaryText ? 58 : 40;
    const x = 12;
    const y = 12;

    ctx.beginPath();
    ctx.fillStyle = 'rgba(6, 10, 12, 0.88)';
    ctx.strokeStyle = accentColor;
    ctx.lineWidth = 1.5;
    ctx.roundRect(x, y, badgeWidth, badgeHeight, 10);
    ctx.fill();
    ctx.stroke();

    ctx.beginPath();
    ctx.fillStyle = accentColor;
    ctx.roundRect(x + 10, y + 11, 6, badgeHeight - 22, 3);
    ctx.fill();

    ctx.fillStyle = '#f8fafc';
    ctx.textAlign = 'left';
    ctx.textBaseline = 'middle';
    ctx.font = '950 18px "Segoe UI"';
    ctx.fillText(primaryText, x + 24, y + 22);

    if (secondaryText) {
      ctx.fillStyle = 'rgba(226, 232, 240, 0.82)';
      ctx.font = '800 12px "Segoe UI"';
      ctx.fillText(secondaryText, x + 24, y + 43);
    }

    ctx.restore();
  };

  graph_retracements = (ctx, pattern) => {
    const coordinates = getPatternCoordinates(this.chartStateRef.current, pattern);

    if (!coordinates?.x || !coordinates?.a || !coordinates?.b || !coordinates?.c || !coordinates?.d) {
      return;
    }

    const formatRatio = (value) => {
      const numericValue = Number(value);

      if (!Number.isFinite(numericValue)) {
        return null;
      }

      return numericValue >= 8
        ? `${numericValue.toFixed(1)}%`
        : `${(numericValue * 100).toFixed(1)}%`;
    };
    const rows = [
      {
        key: 'ab',
        label: 'AB',
        ratio: formatRatio(pattern?.trade_ab_price_retracement),
        from: coordinates.x,
        to: coordinates.b,
      },
      {
        key: 'bc',
        label: 'BC',
        ratio: formatRatio(pattern?.trade_bc_price_retracement),
        from: coordinates.a,
        to: coordinates.c,
      },
      {
        key: 'cd-bc',
        label: 'CD/BC',
        ratio: formatRatio(pattern?.trade_cd_bc_price_retracement),
        from: coordinates.b,
        to: coordinates.d,
      },
      {
        key: 'cd-xa',
        label: 'CD/XA',
        ratio: formatRatio(pattern?.trade_cd_xa_price_retracement),
        from: coordinates.x,
        to: coordinates.d,
      },
    ].filter((row) => row.from && row.to);

    ctx.save();
    ctx.font = '900 20px "Segoe UI"';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';

    rows.forEach((row, index) => {
      const midX = (row.from.x + row.to.x) / 2;
      const midY = (row.from.y + row.to.y) / 2;
      const label = row.ratio ? `${row.label} ${row.ratio}` : row.label;
      const labelWidth = Math.max(86, ctx.measureText(label).width + 24);
      const labelHeight = 36;
      const offsetY = index % 2 === 0 ? -34 : 34;
      const labelX = clamp(midX - labelWidth / 2, 8, Math.max(8, ctx.canvas.width - labelWidth - 8));
      const labelY = clamp(midY + offsetY - labelHeight / 2, 8, Math.max(8, ctx.canvas.height - labelHeight - 8));

      ctx.beginPath();
      ctx.strokeStyle = 'rgba(226, 232, 240, 0.3)';
      ctx.lineWidth = 1.3;
      ctx.setLineDash([5, 7]);
      ctx.moveTo(row.from.x, row.from.y);
      ctx.lineTo(row.to.x, row.to.y);
      ctx.stroke();
      ctx.setLineDash([]);

      ctx.beginPath();
      ctx.fillStyle = 'rgba(8, 15, 26, 0.72)';
      ctx.strokeStyle = 'rgba(148, 163, 184, 0.28)';
      ctx.lineWidth = 1;
      ctx.roundRect(labelX, labelY, labelWidth, labelHeight, 10);
      ctx.fill();
      ctx.stroke();

      ctx.fillStyle = 'rgba(226, 232, 240, 0.86)';
      ctx.fillText(label, labelX + labelWidth / 2, labelY + labelHeight / 2);
    });

    ctx.restore();
  };

  graph_trade_levels = (ctx, canvas, pattern) => {
    const chartState = this.chartStateRef.current;
    const exitPrice = firstFinitePrice(
      pattern?.exit_price,
      pattern?.target_close,
      pattern?.trade_current_price,
      isTradeLoss(pattern)
        ? pattern?.trade_risk_exit_price
        : pattern?.trade_reward_exit_price
    );
    const actualExitColor = getActualExitColor(pattern);
    const actualExitLabel = getActualExitLabel(pattern);
    const levels = pattern?.xa_canvas_mode
      ? [
          {
            key: 'entry',
            label: 'D',
            price: toFinitePrice(pattern?.xa_start_price),
            color: 'rgba(248, 250, 252, 0.86)',
            dash: [3, 5],
          },
          {
            key: 'target',
            label: 'Rev XA',
            price: toFinitePrice(pattern?.xa_reversal_limit_price),
            color: TRADE_TARGET_COLOR,
            dash: [7, 7],
          },
          {
            key: 'stop',
            label: 'Cont XA',
            price: toFinitePrice(pattern?.xa_continuation_limit_price),
            color: TRADE_STOP_COLOR,
            labelTextColor: '#fecaca',
            labelBackground: 'rgba(69, 10, 10, 0.9)',
            dash: [7, 7],
          },
        ]
      : [
          {
            key: 'entry',
            label: 'ENTRY',
            price: toFinitePrice(pattern?.trade_enter_price),
            color: 'rgba(248, 250, 252, 0.86)',
            dash: [3, 5],
          },
          {
            key: 'stop',
            label: 'SL',
            price: toFinitePrice(pattern?.trade_risk_exit_price),
            color: TRADE_STOP_COLOR,
            labelTextColor: '#fecaca',
            labelBackground: 'rgba(69, 10, 10, 0.9)',
            dash: [],
            alpha: 0.78,
            lineWidth: 2.1,
          },
          {
            key: 'target',
            label: 'TP',
            price: toFinitePrice(pattern?.trade_reward_exit_price),
            color: TRADE_TARGET_COLOR,
            dash: [7, 7],
          },
          {
            key: 'exit',
            label: actualExitLabel,
            price: exitPrice,
            color: actualExitColor,
            labelTextColor: actualExitColor,
            labelBackground:
              actualExitColor === TRADE_EXIT_COLOR ? 'rgba(69, 36, 8, 0.9)' : undefined,
            dash: isStopExit(pattern) ? [7, 7] : [5, 5],
            lineWidth: isStopExit(pattern) ? 2.5 : 3,
          },
        ];
    if (!chartState?.canvas) {
      return;
    }

    const visibleLevels = dedupeTradeLevelsByY(
      levels
        .filter((level) => Number.isFinite(level.price))
        .map((level) => ({ ...level, y: getCanvasY(chartState, level.price) }))
        .filter((level) => Number.isFinite(level.y))
    );

    if (!visibleLevels.length) {
      return;
    }

    ctx.save();
    const leftX = 22;
    const rightX = canvas.width - 22;

    visibleLevels.forEach((level) => {
      if (level.y < -40 || level.y > canvas.height + 40) {
        return;
      }

      ctx.beginPath();
      ctx.strokeStyle = level.color;
      ctx.globalAlpha = level.alpha ?? 1;
      ctx.lineWidth = level.lineWidth ?? (level.key === 'entry' ? 2.2 : 2.4);
      ctx.setLineDash(level.dash);
      ctx.moveTo(leftX, level.y);
      ctx.lineTo(rightX, level.y);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;

      if (level.key === 'entry' || level.key === 'stop' || level.key === 'target' || level.key === 'exit') {
        const label = level.label;
        ctx.font = '900 12px "Segoe UI"';
        const labelWidth = Math.max(66, ctx.measureText(label).width + 20);
        const labelHeight = 24;
        const labelX = level.key === 'target'
          ? clamp(rightX - labelWidth - 8, 8, canvas.width - labelWidth - 8)
          : clamp(leftX + 8, 8, canvas.width - labelWidth - 8);
        const preferredY = level.y - labelHeight - 6;
        const labelY =
          preferredY < 8 ? Math.min(level.y + 8, canvas.height - labelHeight - 8) : preferredY;

        ctx.beginPath();
        ctx.fillStyle = level.labelBackground ?? 'rgba(8, 15, 26, 0.88)';
        ctx.strokeStyle = level.color;
        ctx.lineWidth = 1;
        ctx.roundRect(labelX, labelY, labelWidth, labelHeight, 8);
        ctx.fill();
        ctx.stroke();

        ctx.fillStyle = level.labelTextColor ?? level.color;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(label, labelX + labelWidth / 2, labelY + labelHeight / 2);
      }
    });

    ctx.restore();
  };

  price_levels = (ctxPrice, ctx, canvas, pattern, options = {}) => {
    const chartState = this.chartStateRef.current;
    const showRays = options?.showRays !== false;
    const showTags = options?.showTags !== false;
    const showExit = Boolean(options?.showExit);
    const exitPrice = firstFinitePrice(
      pattern?.exit_price,
      pattern?.target_close,
      pattern?.trade_current_price,
      isTradeLoss(pattern)
        ? pattern?.trade_risk_exit_price
        : pattern?.trade_reward_exit_price
    );
    const levels = pattern?.xa_canvas_mode
      ? [
          {
            key: 'stop',
            label: 'Cont XA',
            price: toFinitePrice(pattern?.xa_continuation_limit_price),
            color: TRADE_STOP_COLOR,
            dash: [7, 6],
          },
          {
            key: 'entry',
            label: 'D',
            price: toFinitePrice(pattern?.xa_start_price),
            color: '#ffffff',
            dash: [3, 5],
          },
          {
            key: 'target',
            label: 'Rev XA',
            price: toFinitePrice(pattern?.xa_reversal_limit_price),
            color: TRADE_TARGET_COLOR,
            dash: [7, 6],
          },
        ]
      : [
          {
            key: 'stop',
            label: 'SL',
            price: toFinitePrice(pattern?.trade_risk_exit_price),
            color: TRADE_STOP_COLOR,
            dash: [],
            alpha: 0.78,
          },
          {
            key: 'entry',
            label: 'ENT',
            price: toFinitePrice(pattern?.trade_enter_price),
            color: '#ffffff',
            dash: [3, 5],
          },
          {
            key: 'target',
            label: 'TP',
            price: toFinitePrice(pattern?.trade_reward_exit_price),
            color: TRADE_TARGET_COLOR,
            dash: [7, 6],
          },
          ...(showExit
            ? [
                {
                  key: 'exit',
                  label: getActualExitLabel(pattern),
                  price: exitPrice,
                  color: getActualExitColor(pattern),
                  dash: isStopExit(pattern) ? [7, 6] : [5, 5],
                  lineWidth: isStopExit(pattern) ? 2.2 : 2.8,
                },
              ]
            : []),
        ];
    const visibleLevelsInput = levels.filter((level) => Number.isFinite(level.price));

    if (!visibleLevelsInput.length || !chartState?.canvas) {
      return;
    }

    const visibleYs = visibleLevelsInput
      .map((level) => ({ ...level, y: getCanvasY(chartState, level.price) }))
      .filter((level) => Number.isFinite(level.y));
    const drawableLevels = dedupeTradeLevelsByY(visibleYs);
    const entryLevel = visibleYs.find((level) => level.key === 'entry');
    const stopLevel = visibleYs.find((level) => level.key === 'stop');
    const targetLevel = visibleYs.find((level) => level.key === 'target');
    const entryIndex = pattern?.xa_canvas_mode
      ? Number.isFinite(Number(pattern?.d_confirm))
        ? Number(pattern.d_confirm)
        : Number(pattern?.d)
      : Number.isFinite(Number(pattern?.entry))
        ? Number(pattern.entry)
        : Number(pattern?.d);
    const entryCandle = Number.isFinite(entryIndex)
      ? chartState?.candles?.items?.[Math.round(entryIndex) - 1]
      : null;
    const entryCandleGeometry = entryCandle
      ? new Chart(this.chartStateRef).getCandleGeometry(entryCandle, getCanvasX(chartState, entryIndex))
      : null;
    const leftX = Number.isFinite(entryCandleGeometry?.candleLeftX)
      ? entryCandleGeometry.candleLeftX
      : Math.max(0, chartState.canvas.width * 0.04);
    const rightX = chartState.canvas.width - 1;

    if (showRays) {
      ctx.save();

      if (entryLevel && targetLevel) {
        const top = Math.min(entryLevel.y, targetLevel.y);
        const height = Math.abs(entryLevel.y - targetLevel.y);
        if (height > 0) {
          ctx.fillStyle = TRADE_PROFIT_ZONE;
          ctx.fillRect(leftX, top, rightX - leftX, height);
        }
      }

      if (entryLevel && stopLevel) {
        const top = Math.min(entryLevel.y, stopLevel.y);
        const height = Math.abs(entryLevel.y - stopLevel.y);
        if (height > 0) {
          ctx.fillStyle = TRADE_LOSS_ZONE;
          ctx.fillRect(leftX, top, rightX - leftX, height);
        }
      }

      drawableLevels.forEach((level) => {
        ctx.beginPath();
        ctx.strokeStyle = level.color;
        ctx.globalAlpha = level.alpha ?? 1;
        ctx.lineWidth = level.lineWidth ?? (level.key === 'entry' ? 2.6 : 2.2);
        ctx.setLineDash(level.dash);
        ctx.moveTo(leftX, level.y);
        ctx.lineTo(rightX, level.y);
        ctx.stroke();
        ctx.globalAlpha = 1;
      });

      ctx.restore();
    }

    if (!showTags) {
      void canvas;
      return;
    }

    ctxPrice.save();
    ctxPrice.font = '950 16px "Segoe UI"';
    ctxPrice.textAlign = 'center';
    ctxPrice.textBaseline = 'middle';
    const tagHeight = 32;
    const tagWidth = Math.max(58, ctxPrice.canvas.width - 6);
    const tagX = Math.max(4, (ctxPrice.canvas.width - tagWidth) / 2);
    const tagGap = 4;
    const sortedTags = [...drawableLevels]
      .sort((left, right) => left.y - right.y)
      .map((level) => ({
        ...level,
        tagY: Math.min(
          Math.max(level.y - tagHeight / 2, 4),
          ctxPrice.canvas.height - tagHeight - 4
        ),
      }));

    sortedTags.forEach((level, index) => {
      if (index === 0) {
        return;
      }

      const previous = sortedTags[index - 1];
      level.tagY = Math.max(level.tagY, previous.tagY + tagHeight + tagGap);
    });

    const overflow = sortedTags.length
      ? sortedTags[sortedTags.length - 1].tagY + tagHeight + 4 - ctxPrice.canvas.height
      : 0;
    if (overflow > 0) {
      for (let index = sortedTags.length - 1; index >= 0; index -= 1) {
        sortedTags[index].tagY -= overflow;
      }
    }

    sortedTags.forEach((level) => {
      const tagCenterY = level.tagY + tagHeight / 2;

      ctxPrice.beginPath();
      ctxPrice.strokeStyle = level.color;
      ctxPrice.lineWidth = 1;
      ctxPrice.setLineDash([3, 3]);
      ctxPrice.moveTo(4, level.y);
      ctxPrice.lineTo(tagX, tagCenterY);
      ctxPrice.stroke();
      ctxPrice.setLineDash([]);

      ctxPrice.beginPath();
      ctxPrice.fillStyle = LABEL_BACKGROUND;
      ctxPrice.strokeStyle = level.color;
      ctxPrice.lineWidth = 1;
      ctxPrice.roundRect(tagX, level.tagY, tagWidth, tagHeight, 7);
      ctxPrice.fill();
      ctxPrice.stroke();

      ctxPrice.fillStyle = level.color;
      ctxPrice.font = '950 16px "Segoe UI"';
      ctxPrice.fillText(level.price.toFixed(2), tagX + tagWidth / 2, level.tagY + tagHeight / 2);
    });

    ctxPrice.restore();
    void canvas;
  };

  drawLabelMid = (ctx, label, point1, point2, offsetX = 0, offsetY = -5) => {
    const midX = (point1.x + point2.x) / 2;
    const midY = (point1.y + point2.y) / 2;

    ctx.save();
    ctx.font = '12px Arial';
    const textWidth = ctx.measureText(label).width;
    const textHeight = 12;
    ctx.fillStyle = LABEL_BACKGROUND;
    ctx.fillRect(midX + offsetX - 2, midY + offsetY - textHeight + 2, textWidth + 4, textHeight + 4);
    ctx.strokeStyle = LABEL_BORDER_COLOR;
    ctx.lineWidth = 1;
    ctx.strokeRect(midX + offsetX - 2, midY + offsetY - textHeight + 2, textWidth + 4, textHeight + 4);
    ctx.fillStyle = LABEL_TEXT_COLOR;
    ctx.fillText(label, midX + offsetX, midY + offsetY);
    ctx.restore();
  };

  drawLabel = (ctx, label, point, offsetX = -10, offsetY = -15) => {
    ctx.save();
    ctx.font = '20px Arial';
    const textWidth = ctx.measureText(label).width;
    const textHeight = 20;
    ctx.fillStyle = LABEL_BACKGROUND;
    ctx.fillRect(point.x + offsetX - 2, point.y + offsetY - textHeight + 2, textWidth + 4, textHeight + 4);
    ctx.strokeStyle = LABEL_BORDER_COLOR;
    ctx.lineWidth = 1;
    ctx.strokeRect(point.x + offsetX - 2, point.y + offsetY - textHeight + 2, textWidth + 4, textHeight + 4);
    ctx.fillStyle = LABEL_TEXT_COLOR;
    ctx.fillText(label, point.x + offsetX, point.y + offsetY);
    ctx.restore();
  };
}
