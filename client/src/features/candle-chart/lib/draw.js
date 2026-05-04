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
const TARGET_PRICE_GRID_PX = 58;
const RETRACEMENT_COLOR = 'rgba(226, 234, 245, 0.62)';
const LABEL_BACKGROUND = 'rgba(18, 29, 45, 0.92)';
const LABEL_TEXT_COLOR = '#f8fbff';
const LABEL_BORDER_COLOR = 'rgba(114, 148, 206, 0.3)';
const REVERSAL_D_CANDLE_COLOR = '#f3d33b';
const PROP_ENTRY_COLOR = '#ffffff';
const TRADE_TARGET_COLOR = '#53f0a7';
const TRADE_STOP_COLOR = '#ff5f6d';
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
    const exitIndex = Number(activePattern?.exit_date);
    const shouldHighlightExitCandle =
      Boolean(options?.highlightExitCandle) && Number.isFinite(exitIndex) && exitIndex >= 1;
    const exitColor = Number(activePattern?.trade_result) === 2 ? TRADE_STOP_COLOR : TRADE_TARGET_COLOR;

    for (let candleIndex = start; candleIndex <= end; candleIndex += 1) {
      const candle = chartState.candles.items[candleIndex - 1];
      const x = getCanvasX(chartState, candleIndex);
      const isReversalSignalCandle = reversalFocusOnly && (reversalIndexes?.has(candleIndex) ?? false);
      const isExitCandle = shouldHighlightExitCandle && candleIndex === Math.round(exitIndex);

      this.drawCandle(ctx, candle, x, {
        isPatternSpan: isReversalSignalCandle,
        isPivot: false,
        isReversalSignalCandle,
        isExitCandle,
        exitColor,
        palette: patternPalette,
      });
      this.drawWick(ctx, candle, x, {
        isPatternSpan: isReversalSignalCandle,
        isPivot: false,
        isReversalSignalCandle,
        isExitCandle,
        exitColor,
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
      isExitCandle = false,
      exitColor = null,
      palette = BULLISH_PALETTE,
    } = highlight;

    ctx.save();
    ctx.fillStyle = isReversalSignalCandle
      ? REVERSAL_D_CANDLE_COLOR
      : isExitCandle && exitColor
      ? exitColor
      : getCandleStrokeColor(candle);
    ctx.globalAlpha = isExitCandle && !isReversalSignalCandle ? 0.96 : 1;
    ctx.fillRect(candleLeftX, candleTopY, candleRenderWidth, candleHeight);
    ctx.globalAlpha = 1;

    if (isPatternSpan || isExitCandle) {
      ctx.strokeStyle = isReversalSignalCandle
        ? REVERSAL_D_CANDLE_COLOR
        : isExitCandle && exitColor
        ? exitColor
        : isPivot
        ? palette.line
        : 'rgba(220, 234, 255, 0.36)';
      ctx.lineWidth = isExitCandle ? 3.2 : isReversalSignalCandle ? 2 : isPivot ? 2 : 1;
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
      isExitCandle = false,
      exitColor = null,
      palette = BULLISH_PALETTE,
    } = highlight;

    ctx.save();
    ctx.strokeStyle = isReversalSignalCandle
      ? REVERSAL_D_CANDLE_COLOR
      : isExitCandle && exitColor
      ? exitColor
      : isPatternSpan && isPivot
      ? palette.line
      : getCandleStrokeColor(candle);
    ctx.lineWidth = isExitCandle ? 2 : isReversalSignalCandle ? 2 : isPatternSpan && isPivot ? 1.5 : 1;

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
    const entryIndex = Number(pattern?.entry);

    if (Number.isFinite(entryIndex) && entryIndex >= 1) {
      this.drawEventMarker(ctx, pattern, {
        index: entryIndex,
        label: 'Entry',
        color: PROP_ENTRY_COLOR,
        stackOrder: 1,
      });
    }
  };

  price_levels = (ctxPrice, ctx, canvas, pattern, options = {}) => {
    const chartState = this.chartStateRef.current;
    const showRays = options?.showRays !== false;
    const showTags = options?.showTags !== false;
    const levels = [
      {
        key: 'stop',
        label: 'SL',
        price: Number(pattern?.trade_risk_exit_price),
        color: TRADE_STOP_COLOR,
        dash: [7, 6],
      },
      {
        key: 'entry',
        label: 'ENT',
        price: Number(pattern?.trade_enter_price),
        color: '#ffffff',
        dash: [3, 5],
      },
      {
        key: 'target',
        label: 'TP',
        price: Number(pattern?.trade_reward_exit_price),
        color: TRADE_TARGET_COLOR,
        dash: [7, 6],
      },
    ].filter((level) => Number.isFinite(level.price));

    if (!levels.length || !chartState?.canvas) {
      return;
    }

    const visibleYs = levels
      .map((level) => ({ ...level, y: getCanvasY(chartState, level.price) }))
      .filter((level) => Number.isFinite(level.y));
    const entryLevel = visibleYs.find((level) => level.key === 'entry');
    const stopLevel = visibleYs.find((level) => level.key === 'stop');
    const targetLevel = visibleYs.find((level) => level.key === 'target');
    const entryIndex = Number.isFinite(Number(pattern?.entry)) ? Number(pattern.entry) : Number(pattern?.d);
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

      visibleYs.forEach((level) => {
        ctx.beginPath();
        ctx.strokeStyle = level.color;
        ctx.lineWidth = level.key === 'entry' ? 2.6 : 2.2;
        ctx.setLineDash(level.dash);
        ctx.moveTo(leftX, level.y);
        ctx.lineTo(rightX, level.y);
        ctx.stroke();
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
    const sortedTags = [...visibleYs]
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

      ctxPrice.fillStyle = LABEL_TEXT_COLOR;
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
