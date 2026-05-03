import * as utilities from './utilities.js';
import { setCandleSpacing } from './chartState.js';
import { clamp, getPatternBounds, getPriceScale } from './geometry.js';

const CHART_FIT_PADDING = {
  horizontal: 52,
  verticalTop: 34,
  verticalBottom: 28,
};
const MIN_FIT_PRICE_SPAN_RATIO = 0.001;
const MIN_FIT_PRICE_SPAN_ABSOLUTE = 0.01;

const getMinimumFitPriceSpan = (referencePrice) =>
  Math.max(Math.abs(referencePrice) * MIN_FIT_PRICE_SPAN_RATIO, MIN_FIT_PRICE_SPAN_ABSOLUTE);

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

const getMatchedReversalSignalKey = (pattern, activeReversalFilter) => {
  const normalizedFilter =
    activeReversalFilter?.mode && activeReversalFilter?.value ? activeReversalFilter : null;

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

  return REVERSAL_TYPE_TO_SIGNAL_KEY[String(pattern?.reversal_type ?? 'None')] ?? null;
};

const getReversalCandleIndexes = (pattern, activeReversalFilter) => {
  const dIndex = Number(pattern?.d);
  const signalKey = getMatchedReversalSignalKey(pattern, activeReversalFilter);

  if (!Number.isFinite(dIndex) || !signalKey) {
    return [];
  }

  if (signalKey === 'morning_star' || signalKey === 'evening_star') {
    return [dIndex - 1, dIndex, dIndex + 1].filter((index) => index >= 1);
  }

  if (signalKey === 'bullish_engulfing' || signalKey === 'bearish_engulfing') {
    return [dIndex - 1, dIndex].filter((index) => index >= 1);
  }

  if (signalKey === 'three_white_soldiers') {
    return [dIndex - 2, dIndex - 1, dIndex].filter((index) => index >= 1);
  }

  if (signalKey === 'three_black_crows') {
    return [dIndex - 2, dIndex - 1, dIndex].filter((index) => index >= 1);
  }

  return [dIndex];
};

const getReversalFocusBounds = (chartStateRef, rustPattern, activeReversalFilter) => {
  const chartState = chartStateRef.current;
  const indexes = getReversalCandleIndexes(rustPattern, activeReversalFilter);
  const dIndex = Number(rustPattern?.d);

  if (!indexes.length) {
    return null;
  }

  const candles = indexes
    .map((index) => ({
      index,
      candle: chartState?.candles?.items?.[index - 1],
    }))
    .filter(({ candle }) => candle);

  if (!candles.length) {
    return null;
  }

  const minPrice = Math.min(...candles.map(({ candle }) => candle.candle_low));
  const maxPrice = Math.max(...candles.map(({ candle }) => candle.candle_high));
  const minIndex = Math.max(1, Math.min(...candles.map(({ index }) => index)) - 2);
  const maxIndex = Math.min(
    chartState.candles.items.length,
    Math.max(...candles.map(({ index }) => index)) + 2
  );
  const pricePadding = Math.max((maxPrice - minPrice) * 0.18, getMinimumFitPriceSpan(maxPrice));

  return {
    minPrice: minPrice - pricePadding,
    maxPrice: maxPrice + pricePadding,
    minIndex,
    maxIndex,
    anchorIndex:
      indexes.length >= 3
        ? indexes[Math.floor(indexes.length / 2)]
        : Number.isFinite(dIndex)
          ? dIndex
          : undefined,
  };
};

const getPropFocusBounds = (chartStateRef, rustPattern) => {
  const chartState = chartStateRef.current;
  const baseBounds = getPatternBounds(rustPattern);
  const dConfirmIndex = Number(rustPattern?.d_confirm);
  const reversalDetectIndex = Number(rustPattern?.reversal_detect);
  const entryIndex = Number(rustPattern?.entry);
  const exitIndex = Number(rustPattern?.exit_date);

  if (!baseBounds) {
    return baseBounds ?? null;
  }

  const eventIndexes = [dConfirmIndex, reversalDetectIndex, entryIndex, exitIndex].filter(
    (value) => Number.isFinite(value) && value >= 1
  );
  const focusIndexes = [baseBounds.minIndex, baseBounds.maxIndex, ...eventIndexes];
  const eventPrices = eventIndexes.flatMap((index) => {
    const candle = chartState?.candles?.items?.[index - 1];

    return candle ? [candle.candle_low, candle.candle_high] : [];
  });
  const tradeLevelPrices = [
    Number(rustPattern?.trade_enter_price),
    Number(rustPattern?.trade_risk_exit_price),
    Number(rustPattern?.trade_reward_exit_price),
    Number(rustPattern?.exit_price),
  ].filter((value) => Number.isFinite(value));
  const focusPrices = [
    baseBounds.minPrice,
    baseBounds.maxPrice,
    ...eventPrices,
    ...tradeLevelPrices,
  ];
  const minFocusIndex = Math.max(1, Math.min(...focusIndexes));
  const maxFocusIndex = Math.min(
    chartState?.candles?.items?.length ?? Math.max(...focusIndexes),
    Math.max(...focusIndexes)
  );
  const minFocusPrice = Math.min(...focusPrices);
  const maxFocusPrice = Math.max(...focusPrices);
  const pricePadding = Math.max(
    (maxFocusPrice - minFocusPrice) * 0.18,
    getMinimumFitPriceSpan(maxFocusPrice)
  );

  return {
    minPrice: minFocusPrice - pricePadding,
    maxPrice: maxFocusPrice + pricePadding,
    minIndex: Math.max(1, minFocusIndex - 2),
    maxIndex: maxFocusIndex + 2,
    anchorIndex: (minFocusIndex + maxFocusIndex) / 2,
  };
};

const applyHorizontalFit = (chartStateRef, minIndex, maxIndex, options = {}) => {
  const chartState = chartStateRef.current;
  const horizontalPadding = Math.min(
    CHART_FIT_PADDING.horizontal,
    Math.max(chartState.canvas.width * 0.08, 12)
  );
  const drawableWidth = Math.max(chartState.canvas.width - horizontalPadding * 2, 1);
  const edgeBufferCandles = options.focusMode === 'reversal' ? 2 : 4;
  const spanInCandles = Math.max(maxIndex - minIndex + 1 + edgeBufferCandles * 2, 1);
  const completeWidth = clamp(drawableWidth / spanInCandles, 0.35, 72);
  const spacing = completeWidth <= 1.1 ? 0 : Math.min(completeWidth * 0.16, 3);
  const candleWidth = Math.max(completeWidth - spacing, Math.min(completeWidth, 0.2));

  setCandleSpacing(chartState, candleWidth, spacing);

  const desiredCenterIndex = Number.isFinite(options.anchorIndex)
    ? options.anchorIndex
    : (minIndex + maxIndex) / 2;
  const minVisibleX = horizontalPadding;
  const maxVisibleX = chartState.canvas.width - horizontalPadding;
  const lowerBoundXOrigin =
    -chartState.candles.completeWidth * minIndex +
    chartState.candles.completeWidth / 2 -
    maxVisibleX;
  const upperBoundXOrigin =
    -chartState.candles.completeWidth * maxIndex +
    chartState.candles.completeWidth / 2 -
    minVisibleX;
  const centeredXOrigin =
    -chartState.candles.completeWidth * desiredCenterIndex +
    chartState.candles.completeWidth / 2 -
    chartState.canvas.width / 2;
  chartState.viewport.xOrigin =
    lowerBoundXOrigin <= upperBoundXOrigin
      ? clamp(centeredXOrigin, lowerBoundXOrigin, upperBoundXOrigin)
      : centeredXOrigin;
  chartState.viewport.prevXOrigin = chartState.viewport.xOrigin;
};

const applyVerticalFit = (chartStateRef, minPrice, maxPrice) => {
  const chartState = chartStateRef.current;
  const priceSpan = Math.max(maxPrice - minPrice, getMinimumFitPriceSpan(maxPrice));
  const verticalTopPadding = Math.min(
    CHART_FIT_PADDING.verticalTop,
    Math.max(chartState.canvas.height * 0.12, 18)
  );
  const verticalBottomPadding = Math.min(
    CHART_FIT_PADDING.verticalBottom,
    Math.max(chartState.canvas.height * 0.1, 16)
  );
  const availableHeight = Math.max(
    chartState.canvas.height - verticalTopPadding - verticalBottomPadding,
    1
  );
  const basePixelsPerGrid = Math.max(chartState.canvas.height / 10, 1);
  const targetPriceScale = availableHeight / priceSpan;

  chartState.price.unitAmount = Math.max(
    basePixelsPerGrid / targetPriceScale,
    0.0001
  );
  chartState.price.pixelsPerGrid = basePixelsPerGrid;
  chartState.price.prevPixelsPerGrid = basePixelsPerGrid;
  chartState.price.startingPixelsPerGrid = basePixelsPerGrid;
  chartState.price.priceUnitPixelSize = basePixelsPerGrid;

  const priceScale = getPriceScale(chartState);
  chartState.viewport.baselineY = verticalTopPadding + maxPrice * priceScale;
  chartState.viewport.prevBaselineY = chartState.viewport.baselineY;

  const midPrice = utilities.get_mid_price(chartStateRef);
  chartState.price.currentMidPrice = midPrice;
  chartState.price.prevMidPrice = midPrice;
};

const syncBaselineToMidPrice = (chartStateRef) => {
  const chartState = chartStateRef.current;
  chartState.viewport.baselineY =
    chartState.viewport.startingBaselineY / 2 +
    chartState.price.currentMidPrice * getPriceScale(chartState);
  chartState.viewport.prevBaselineY = chartState.viewport.baselineY;
};

export const chart_Y_movement = (chartStateRef) => {
  const chartState = chartStateRef.current;
  const pixelsMouseMoved = chartState.mouse.down.y - chartState.mouse.pos.y;
  chartState.viewport.baselineY = chartState.viewport.prevBaselineY - pixelsMouseMoved;

  const convertedPixels = pixelsMouseMoved / getPriceScale(chartState);
  chartState.price.currentMidPrice = chartState.price.prevMidPrice - convertedPixels;
};

export const push_price_to_middle_screen = (chartStateRef, rustPattern) => {
  const chartState = chartStateRef.current;
  const targetPrice = parseFloat(rustPattern?.trade_enter_price ?? rustPattern?.a_high);

  if (!Number.isFinite(targetPrice)) {
    return;
  }

  const pricePixelLocation = utilities.get_pixel_location_of_a_price(chartStateRef, targetPrice);
  chartState.viewport.baselineY = pricePixelLocation + chartState.viewport.startingBaselineY / 2;
  chartState.viewport.prevBaselineY = chartState.viewport.baselineY;

  const midPrice = utilities.get_mid_price(chartStateRef);
  chartState.price.currentMidPrice = midPrice;
  chartState.price.prevMidPrice = midPrice;
};

export const chart_zoom_out = (chartStateRef, threshold) => {
  const chartState = chartStateRef.current;
  chartState.price.pixelsPerGrid -= 1;
  chartState.price.priceUnitPixelSize = Math.floor(chartState.price.priceUnitPixelSize - 1);

  if (chartState.price.priceUnitPixelSize === threshold) {
    chartState.price.pixelsPerGrid *= 2;
    chartState.price.prevPixelsPerGrid *= 2;
    chartState.price.priceUnitPixelSize = chartState.price.pixelsPerGrid;
    chartState.price.unitAmount *= 2;
  }

  syncBaselineToMidPrice(chartStateRef);
};

export const chart_zoom_in = (chartStateRef, threshold) => {
  const chartState = chartStateRef.current;
  chartState.price.pixelsPerGrid += 1;
  chartState.price.priceUnitPixelSize = Math.floor(chartState.price.priceUnitPixelSize + 1);

  if (chartState.price.priceUnitPixelSize === threshold) {
    chartState.price.pixelsPerGrid /= 2;
    chartState.price.prevPixelsPerGrid /= 2;
    chartState.price.priceUnitPixelSize = chartState.price.pixelsPerGrid;
    chartState.price.unitAmount /= 2;
  }

  syncBaselineToMidPrice(chartStateRef);
};

export const reposition_candles = (chartStateRef, rustPattern, options = {}) => {
  const bounds =
    options.focusMode === 'reversal'
      ? getReversalFocusBounds(chartStateRef, rustPattern, options.activeReversalFilter) ??
        getPatternBounds(rustPattern)
      : options.focusMode === 'prop'
        ? getPropFocusBounds(chartStateRef, rustPattern) ?? getPatternBounds(rustPattern)
      : getPatternBounds(rustPattern);
  const reversalAnchorIndex = Number(rustPattern?.d);
  const propAnchorIndex = [
    Number(rustPattern?.exit_date),
    Number(rustPattern?.entry),
    Number(rustPattern?.reversal_detect),
    Number(rustPattern?.d_confirm),
    Number(rustPattern?.d),
  ].find((value) => Number.isFinite(value));

  if (!bounds) {
    return;
  }

  applyVerticalFit(chartStateRef, bounds.minPrice, bounds.maxPrice);
  applyHorizontalFit(chartStateRef, bounds.minIndex, bounds.maxIndex, {
    ...options,
    anchorIndex:
      bounds.anchorIndex ??
      (options.focusMode === 'reversal' && Number.isFinite(reversalAnchorIndex)
        ? reversalAnchorIndex
        : options.focusMode === 'prop' && Number.isFinite(propAnchorIndex)
          ? propAnchorIndex
        : undefined),
  });
  chartStateRef.current.pattern.length = rustPattern?.pattern_ABCD_bar_length ?? 0;
};
