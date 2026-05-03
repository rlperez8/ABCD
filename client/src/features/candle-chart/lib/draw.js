import {
  formatCandleDate,
  getCanvasX,
  getCanvasY,
  getHoveredCandleIndex,
  getPatternCoordinates,
  getPriceAtCanvasY,
} from './geometry.js';

const BULLISH_COLOR = 'rgba(0, 180, 180, 1)';
const BEARISH_COLOR = 'rgb(214, 112, 112)';
const GRID_COLOR = 'rgba(102, 124, 157, 0.14)';
const GRID_MAJOR_COLOR = 'rgba(124, 150, 190, 0.24)';
const CROSSHAIR_COLOR = 'rgba(223, 233, 247, 0.7)';
const PRICE_PANEL_BACKGROUND = 'rgba(15, 21, 34, 0.94)';
const DATE_PANEL_BACKGROUND = 'rgba(15, 21, 34, 0.96)';
const TAG_BORDER_COLOR = 'rgba(91, 118, 158, 0.46)';
const AXIS_TEXT_COLOR = 'rgba(189, 205, 229, 0.78)';
const AXIS_MUTED_TEXT_COLOR = 'rgba(149, 170, 201, 0.62)';
const RETRACEMENT_COLOR = 'rgba(226, 234, 245, 0.62)';
const LABEL_BACKGROUND = 'rgba(18, 29, 45, 0.92)';
const LABEL_TEXT_COLOR = '#f8fbff';
const LABEL_BORDER_COLOR = 'rgba(114, 148, 206, 0.3)';
const REVERSAL_D_CANDLE_COLOR = '#f3d33b';
const PROP_CONFIRM_COLOR = '#6dc0ff';
const PROP_REVERSAL_COLOR = '#f3d33b';
const PROP_ENTRY_COLOR = '#ffffff';
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

const parseAxisDate = (value) => {
  if (!value) {
    return null;
  }

  const normalized = `${value}`.split(' ')[0];
  const parsed = new Date(`${normalized}T00:00:00`);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
};

const formatAxisDateLabel = (value, completeWidth) => {
  const parsedDate = parseAxisDate(value);

  if (!parsedDate) {
    return formatCandleDate(value);
  }

  if (completeWidth >= 24) {
    return DAY_MONTH_YEAR_FORMATTER.format(parsedDate);
  }

  if (completeWidth >= 10) {
    return DAY_MONTH_FORMATTER.format(parsedDate);
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

    ctx.save();
    ctx.beginPath();
    ctx.strokeStyle = GRID_COLOR;
    ctx.lineWidth = 1;

    for (let y = chartState.viewport.baselineY; y >= 0; y -= chartState.price.pixelsPerGrid) {
      const snappedY = Math.round(y) + 0.5;
      ctx.moveTo(0, snappedY);
      ctx.lineTo(canvas.width, snappedY);
    }

    for (
      let y = chartState.viewport.baselineY + chartState.price.pixelsPerGrid;
      y <= canvas.height;
      y += chartState.price.pixelsPerGrid
    ) {
      const snappedY = Math.round(y) + 0.5;
      ctx.moveTo(0, snappedY);
      ctx.lineTo(canvas.width, snappedY);
    }

    ctx.stroke();
    ctx.restore();
  };

  prices = (ctxPrice, canvasPrice) => {
    const chartState = this.chartStateRef.current;
    const startY =
      chartState.viewport.baselineY -
      Math.ceil(chartState.viewport.baselineY / chartState.price.pixelsPerGrid) *
        chartState.price.pixelsPerGrid;

    ctxPrice.save();
    ctxPrice.font = '600 13px "Segoe UI"';
    ctxPrice.fillStyle = AXIS_TEXT_COLOR;
    ctxPrice.textAlign = 'center';
    ctxPrice.textBaseline = 'middle';

    for (let y = startY; y <= canvasPrice.height; y += chartState.price.pixelsPerGrid) {
      if (y < 0) {
        continue;
      }

      const snappedY = Math.round(y) + 0.5;
      const price = getPriceAtCanvasY(chartState, snappedY);
      const isBaseline = Math.abs(snappedY - chartState.viewport.baselineY) < 1;

      ctxPrice.fillStyle = isBaseline ? AXIS_TEXT_COLOR : AXIS_MUTED_TEXT_COLOR;
      ctxPrice.fillText(
        formatAxisPrice(price, chartState.price.unitAmount),
        canvasPrice.width / 2,
        snappedY
      );
    }

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
      const label = formatAxisDateLabel(candle?.candle_date, chartState.candles.completeWidth);

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

    for (let candleIndex = start; candleIndex <= end; candleIndex += 1) {
      const candle = chartState.candles.items[candleIndex - 1];
      const x = getCanvasX(chartState, candleIndex);
      const isReversalSignalCandle = reversalFocusOnly && (reversalIndexes?.has(candleIndex) ?? false);

      this.drawCandle(ctx, candle, x, {
        isPatternSpan: isReversalSignalCandle,
        isPivot: false,
        isReversalSignalCandle,
        palette: patternPalette,
      });
      this.drawWick(ctx, candle, x, {
        isPatternSpan: isReversalSignalCandle,
        isPivot: false,
        isReversalSignalCandle,
        palette: patternPalette,
      });
    }
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
      palette = BULLISH_PALETTE,
    } = highlight;

    ctx.save();
    ctx.fillStyle = isReversalSignalCandle ? REVERSAL_D_CANDLE_COLOR : getCandleStrokeColor(candle);
    ctx.fillRect(candleLeftX, candleTopY, candleRenderWidth, candleHeight);

    if (isPatternSpan) {
      ctx.strokeStyle = isReversalSignalCandle
        ? REVERSAL_D_CANDLE_COLOR
        : isPivot
        ? palette.line
        : 'rgba(220, 234, 255, 0.36)';
      ctx.lineWidth = isReversalSignalCandle ? 2 : isPivot ? 2 : 1;
      ctx.strokeRect(
        candleLeftX - 0.5,
        candleTopY - 0.5,
        candleRenderWidth + 1,
        candleHeight + 1
      );
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
      palette = BULLISH_PALETTE,
    } = highlight;

    ctx.save();
    ctx.strokeStyle = isReversalSignalCandle
      ? REVERSAL_D_CANDLE_COLOR
      : isPatternSpan && isPivot
      ? palette.line
      : getCandleStrokeColor(candle);
    ctx.lineWidth = isReversalSignalCandle ? 2 : isPatternSpan && isPivot ? 1.5 : 1;

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

  drawSetupNodes = (ctx, coordinates, palette) => {
    const orderedPoints = [coordinates.x, coordinates.a, coordinates.b, coordinates.c, coordinates.d].filter(Boolean);

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

  drawSetupOverlay = (ctx, pattern) => {
    const coordinates = getPatternCoordinates(this.chartStateRef.current, pattern);
    const palette = this.getOverlayPalette(pattern);

    if (!coordinates?.x || !coordinates?.a || !coordinates?.b || !coordinates?.c || !coordinates?.d) {
      return;
    }

    ctx.save();
    this.drawSetupPath(ctx, coordinates, palette, true);
    ctx.strokeStyle = palette.glow;
    ctx.lineWidth = 10;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.stroke();

    this.drawSetupPath(ctx, coordinates, palette, false);
    ctx.strokeStyle = palette.line;
    ctx.lineWidth = 4;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.stroke();
    this.drawSetupNodes(ctx, coordinates, palette);
    this.drawLabel(ctx, 'X', coordinates.x);
    this.drawLabel(ctx, 'A', coordinates.a);
    this.drawLabel(ctx, 'B', coordinates.b);
    this.drawLabel(ctx, 'C', coordinates.c);
    this.drawLabel(ctx, 'D', coordinates.d);
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

  drawEventMarker = (ctx, pattern, { index, label, color, guideFromIndex, stackOrder = 0 }) => {
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

    if (candleGeometry) {
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

  prop_events = (ctx, pattern) => {
    const dIndex = Number(pattern?.d);
    const dConfirmIndex = Number(pattern?.d_confirm);
    const reversalDetectIndex = Number(pattern?.reversal_detect);
    const entryIndex = Number(pattern?.entry);
    const isPropReversalFocus =
      pattern?.prop_outcome_mode === 'reversal' || Boolean(pattern?.reversal_detect_date);

    if (Number.isFinite(dConfirmIndex) && dConfirmIndex >= 1) {
      this.drawEventMarker(ctx, pattern, {
        index: dConfirmIndex,
        label: 'D Confirm',
        color: PROP_CONFIRM_COLOR,
        guideFromIndex: Number.isFinite(dIndex) ? dIndex : undefined,
      });
    }

    if (Number.isFinite(entryIndex) && entryIndex >= 1) {
      this.drawEventMarker(ctx, pattern, {
        index: entryIndex,
        label: 'Entry',
        color: PROP_ENTRY_COLOR,
        guideFromIndex:
          isPropReversalFocus && Number.isFinite(reversalDetectIndex)
            ? reversalDetectIndex
            : Number.isFinite(dConfirmIndex)
              ? dConfirmIndex
              : dIndex,
        stackOrder: 1,
      });
    }

    if (isPropReversalFocus) {
      const hasDistinctReversalDetect =
        Number.isFinite(reversalDetectIndex) &&
        reversalDetectIndex >= 1 &&
        reversalDetectIndex !== dConfirmIndex;

      this.drawEventMarker(ctx, pattern, {
        index:
          Number.isFinite(reversalDetectIndex) && reversalDetectIndex >= 1
            ? reversalDetectIndex
            : dConfirmIndex,
        label: hasDistinctReversalDetect ? 'Reversal Detect' : 'D + Reversal',
        color: PROP_REVERSAL_COLOR,
        guideFromIndex: Number.isFinite(dConfirmIndex) ? dConfirmIndex : dIndex,
        stackOrder: hasDistinctReversalDetect ? 0 : 1,
      });
    }
  };

  price_levels = (ctxPrice, ctx, canvas, pattern) => {
    const chartState = this.chartStateRef.current;
    const palette = this.getOverlayPalette(pattern);
    const entryIndex = Number.isFinite(Number(pattern?.entry)) ? Number(pattern.entry) : Number(pattern?.d);
    const exitIndex = Number(pattern?.exit_date);
    if (!Number.isFinite(entryIndex) || entryIndex < 1 || !Number.isFinite(exitIndex) || exitIndex < 1) {
      return;
    }

    const startX = getCanvasX(chartState, entryIndex);
    const endX = getCanvasX(chartState, exitIndex);
    const stopLossY = getCanvasY(chartState, pattern.trade_risk_exit_price);
    const takeProfitY = getCanvasY(chartState, pattern.trade_reward_exit_price);
    const enteredPriceY = getCanvasY(chartState, pattern.trade_enter_price);

    if (![startX, endX, stopLossY, takeProfitY, enteredPriceY].every(Number.isFinite)) {
      return;
    }

    ctx.save();

    ctx.beginPath();
    ctx.strokeStyle = '#ef5350';
    ctx.lineWidth = 3;
    ctx.moveTo(startX, stopLossY);
    ctx.lineTo(endX, stopLossY);
    ctx.stroke();

    ctx.beginPath();
    ctx.strokeStyle = palette.line;
    ctx.lineWidth = 3;
    ctx.moveTo(startX, takeProfitY);
    ctx.lineTo(endX, takeProfitY);
    ctx.stroke();

    ctx.beginPath();
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 3;
    ctx.moveTo(startX, enteredPriceY);
    ctx.lineTo(endX, enteredPriceY);
    ctx.stroke();

    const profitZoneTop = Math.min(takeProfitY, enteredPriceY);
    const profitZoneHeight = Math.abs(enteredPriceY - takeProfitY);
    if (profitZoneHeight > 0) {
      ctx.fillStyle = palette.zone;
      ctx.fillRect(startX, profitZoneTop, endX - startX, profitZoneHeight);
    }

    const lossZoneTop = Math.min(stopLossY, enteredPriceY);
    const lossZoneHeight = Math.abs(stopLossY - enteredPriceY);
    if (lossZoneHeight > 0) {
      ctx.fillStyle = 'rgba(239, 83, 80, 0.2)';
      ctx.fillRect(startX, lossZoneTop, endX - startX, lossZoneHeight);
    }

    ctx.restore();

    ctxPrice.beginPath();
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
