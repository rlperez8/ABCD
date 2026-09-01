import * as utilities from './utilities.js';
import { setCandleSpacing } from './chartState.js';
import { clamp, getPatternBounds, getPriceScale } from './geometry.js';

const CHART_FIT_PADDING = {
  horizontal: 52,
  verticalTop: 34,
  verticalBottom: 28,
};
const MIN_FIT_PRICE_SPAN_RATIO = 0.001;
const PROP_TRADE_MIN_FIT_PRICE_SPAN_RATIO = 0.00005;
const GRAPH_MIN_FIT_PRICE_SPAN_RATIO = 0.00001;
const MIN_FIT_PRICE_SPAN_ABSOLUTE = 0.01;
const RAW_TRADE_MIN_PRICE_SPAN_ABSOLUTE = 0.002;
const MIN_FIT_COMPLETE_CANDLE_WIDTH = 0.18;
const MIN_RAW_ALL_FIT_COMPLETE_CANDLE_WIDTH = 0.001;
const MAX_FIT_COMPLETE_CANDLE_WIDTH = 180;
const MOBILE_RAW_TRADE_BREAKPOINT = 720;
const MOBILE_RAW_TRADE_MIN_WINDOW_CANDLES = 24;
const DESKTOP_RAW_TRADE_MIN_WINDOW_CANDLES = 52;

const getMinimumFitPriceSpan = (
  referencePrice,
  ratio = MIN_FIT_PRICE_SPAN_RATIO,
  absolute = MIN_FIT_PRICE_SPAN_ABSOLUTE
) => Math.max(Math.abs(referencePrice) * ratio, absolute);

const toFinitePrice = (value) => {
  if (value === null || value === undefined || value === '') {
    return NaN;
  }

  const numericValue = Number(value);
  return Number.isFinite(numericValue) ? numericValue : NaN;
};

const finitePrices = (...values) => values.map(toFinitePrice).filter((value) => Number.isFinite(value));

const getBoundedCandleIndex = (value, candleCount) => {
  const parsed = Number(value);

  if (!Number.isFinite(parsed)) {
    return null;
  }

  const index = Math.round(parsed);
  return index >= 1 && index <= candleCount ? index : null;
};

const getCenteredIndexWindow = (minCoreIndex, maxCoreIndex, desiredWindow, candleCount) => {
  const boundedWindow = Math.max(1, Math.min(candleCount, Math.round(desiredWindow)));
  const centerIndex = (minCoreIndex + maxCoreIndex) / 2;
  let minIndex = Math.round(centerIndex - boundedWindow / 2);
  let maxIndex = minIndex + boundedWindow - 1;

  if (minIndex < 1) {
    maxIndex += 1 - minIndex;
    minIndex = 1;
  }

  if (maxIndex > candleCount) {
    minIndex -= maxIndex - candleCount;
    maxIndex = candleCount;
  }

  minIndex = Math.max(1, Math.min(minIndex, minCoreIndex));
  maxIndex = Math.min(candleCount, Math.max(maxIndex, maxCoreIndex));

  return { minIndex, maxIndex };
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
  const tradeLevelPrices = rustPattern?.xa_canvas_mode
    ? finitePrices(
        rustPattern?.xa_start_price,
        rustPattern?.xa_reversal_limit_price,
        rustPattern?.xa_continuation_limit_price
      )
    : finitePrices(
        rustPattern?.trade_enter_price,
        rustPattern?.trade_risk_exit_price,
        rustPattern?.trade_reward_exit_price,
        rustPattern?.exit_price,
        rustPattern?.source_exit_price
      );
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
    (maxFocusPrice - minFocusPrice) * 0.06,
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

const getPropTradeFocusBounds = (chartStateRef, rustPattern) => {
  const chartState = chartStateRef.current;
  const entryIndex = Number(rustPattern?.entry);
  const exitIndex = Number(rustPattern?.exit_date);
  const canvasEndIndex = Number(rustPattern?.canvas_end);

  if (!Number.isFinite(entryIndex) || !Number.isFinite(exitIndex)) {
    return null;
  }

  const windowEndIndex =
    Number.isFinite(canvasEndIndex) && canvasEndIndex >= 1
      ? Math.max(entryIndex, exitIndex, canvasEndIndex)
      : Math.max(entryIndex, exitIndex);
  const minTradeIndex = Math.max(1, Math.min(entryIndex, exitIndex));
  const maxTradeIndex = Math.min(
    chartState?.candles?.items?.length ?? windowEndIndex,
    windowEndIndex
  );
  const minIndex = Math.max(1, minTradeIndex - 3);
  const maxIndex = Math.min(
    chartState?.candles?.items?.length ?? maxTradeIndex + 3,
    maxTradeIndex + 3
  );
  const visibleTradeCandles = [];

  for (let index = minIndex; index <= maxIndex; index += 1) {
    const candle = chartState?.candles?.items?.[index - 1];

    if (candle) {
      visibleTradeCandles.push(candle);
    }
  }

  const tradeLevelPrices = rustPattern?.xa_canvas_mode
    ? finitePrices(
        rustPattern?.xa_start_price,
        rustPattern?.xa_reversal_limit_price,
        rustPattern?.xa_continuation_limit_price
      )
    : finitePrices(
        rustPattern?.trade_enter_price,
        rustPattern?.trade_risk_exit_price,
        rustPattern?.trade_reward_exit_price,
        rustPattern?.exit_price,
        rustPattern?.source_exit_price
      );
  const endpointIndexes = new Set(
    [entryIndex - 1, entryIndex, entryIndex + 1, exitIndex - 1, exitIndex, exitIndex + 1]
      .map((value) => Math.round(value))
      .filter((value) => value >= minIndex && value <= maxIndex)
  );
  const endpointBodyPrices = [...endpointIndexes]
    .map((index) => chartState?.candles?.items?.[index - 1])
    .filter(Boolean)
    .flatMap((candle) => [Number(candle.candle_open), Number(candle.candle_close)])
    .filter((value) => Number.isFinite(value));
  const candlePrices = visibleTradeCandles.flatMap((candle) => [candle.candle_low, candle.candle_high]);
  const focusPrices = tradeLevelPrices.length
    ? [...tradeLevelPrices, ...endpointBodyPrices]
    : candlePrices;

  if (!focusPrices.length) {
    return null;
  }

  const minFocusPrice = Math.min(...focusPrices);
  const maxFocusPrice = Math.max(...focusPrices);
  const pricePadding = Math.max(
    (maxFocusPrice - minFocusPrice) * 0.035,
    getMinimumFitPriceSpan(maxFocusPrice, PROP_TRADE_MIN_FIT_PRICE_SPAN_RATIO)
  );

  return {
    minPrice: minFocusPrice - pricePadding,
    maxPrice: maxFocusPrice + pricePadding,
    minIndex,
    maxIndex,
    anchorIndex: (entryIndex + exitIndex) / 2,
  };
};

const getGraphFocusBounds = (chartStateRef, rustPattern) => {
  const chartState = chartStateRef.current;
  const baseBounds = getPatternBounds(rustPattern);

  if (!baseBounds) {
    return null;
  }

  const xaHitIndex = Number(rustPattern?.exit_date);
  const xaFocusIndexes =
    rustPattern?.xa_canvas_mode && Number.isFinite(xaHitIndex) && xaHitIndex >= 1
      ? [
          baseBounds.minIndex,
          baseBounds.maxIndex,
          Math.min(xaHitIndex, chartState?.candles?.items?.length ?? xaHitIndex),
        ]
      : [baseBounds.minIndex, baseBounds.maxIndex];
  const xaHitCandle =
    rustPattern?.xa_canvas_mode && Number.isFinite(xaHitIndex)
      ? chartState?.candles?.items?.[Math.round(xaHitIndex) - 1]
      : null;
  const graphPrices = [
    baseBounds.minPrice,
    baseBounds.maxPrice,
    ...(xaHitCandle ? [Number(xaHitCandle.candle_low), Number(xaHitCandle.candle_high)] : []),
    ...(rustPattern?.xa_canvas_mode
      ? finitePrices(
          rustPattern?.xa_start_price,
          rustPattern?.xa_reversal_limit_price,
          rustPattern?.xa_continuation_limit_price
        )
      : finitePrices(
          rustPattern?.trade_enter_price,
          rustPattern?.trade_risk_exit_price,
          rustPattern?.trade_reward_exit_price,
          rustPattern?.exit_price,
          rustPattern?.source_exit_price,
          rustPattern?.target_close,
          rustPattern?.trade_current_price
        )),
  ].filter((value) => Number.isFinite(value));
  const minFocusPrice = Math.min(...graphPrices);
  const maxFocusPrice = Math.max(...graphPrices);
  const pricePadding = Math.max(
    (maxFocusPrice - minFocusPrice) * 0.2,
    getMinimumFitPriceSpan(maxFocusPrice, GRAPH_MIN_FIT_PRICE_SPAN_RATIO)
  );

  return {
    minPrice: minFocusPrice - pricePadding,
    maxPrice: maxFocusPrice + pricePadding,
    minIndex: Math.max(1, Math.min(...xaFocusIndexes)),
    maxIndex: Math.min(
      chartState?.candles?.items?.length ?? Math.max(...xaFocusIndexes),
      Math.max(...xaFocusIndexes)
    ),
    anchorIndex:
      rustPattern?.xa_canvas_mode && Number.isFinite(xaHitIndex)
        ? (baseBounds.minIndex + Math.min(xaHitIndex, chartState?.candles?.items?.length ?? xaHitIndex)) / 2
        : (baseBounds.minIndex + baseBounds.maxIndex) / 2,
  };
};

const getRawCandleFocusBounds = (chartStateRef, rustPattern) => {
  const chartState = chartStateRef.current;
  const candles = chartState?.candles?.items ?? [];
  const rawVisibleCandles = rustPattern?.raw_visible_candles;
  const showAllCandles = String(rawVisibleCandles ?? '').trim().toLowerCase() === 'all';
  const visibleCount = showAllCandles
    ? candles.length
    : Math.max(
        20,
        Math.min(candles.length, Number(rawVisibleCandles) || 240)
      );

  if (!candles.length || visibleCount <= 0) {
    return null;
  }

  const rawFocusIndex = Number(rustPattern?.raw_focus_index);
  const hasFocusIndex =
    Number.isFinite(rawFocusIndex) && rawFocusIndex >= 1 && rawFocusIndex <= candles.length;
  const focusCandle = hasFocusIndex ? candles[Math.round(rawFocusIndex) - 1] : null;
  const focusPrice = toFinitePrice(
    focusCandle?.candle_close ??
      focusCandle?.close ??
      focusCandle?.candle_open ??
      focusCandle?.open
  );
  const selectedTradeIndexes = [
    getBoundedCandleIndex(rustPattern?.entry, candles.length),
    getBoundedCandleIndex(rustPattern?.exit_date, candles.length),
    getBoundedCandleIndex(rustPattern?.trend_confirm_index, candles.length),
  ].filter((index) => index !== null);
  const isMobileRawTradeFit = chartState.canvas.width <= MOBILE_RAW_TRADE_BREAKPOINT;
  const isSelectedTradeFit =
    Boolean(rustPattern?.raw_selected_trend) &&
    !showAllCandles &&
    selectedTradeIndexes.length > 0;

  if (isSelectedTradeFit) {
    const hasExitIndex = getBoundedCandleIndex(rustPattern?.exit_date, candles.length) !== null;
    const coreIndexes = hasExitIndex
      ? selectedTradeIndexes
      : [...selectedTradeIndexes, 1];
    const minCoreIndex = Math.max(1, Math.min(...coreIndexes));
    const maxCoreIndex = Math.min(candles.length, Math.max(...coreIndexes));
    const coreSpan = Math.max(maxCoreIndex - minCoreIndex + 1, 1);
    const contextPadding = isMobileRawTradeFit
      ? coreSpan <= 8
        ? 10
        : coreSpan <= 40
          ? 14
          : Math.min(80, Math.ceil(coreSpan * 0.2))
      : coreSpan <= 8
        ? 18
        : coreSpan <= 40
          ? 24
          : Math.min(140, Math.ceil(coreSpan * 0.24));
    const desiredWindow = Math.max(
      isMobileRawTradeFit ? MOBILE_RAW_TRADE_MIN_WINDOW_CANDLES : DESKTOP_RAW_TRADE_MIN_WINDOW_CANDLES,
      coreSpan + contextPadding * 2
    );
    const focusedWindow = getCenteredIndexWindow(
      minCoreIndex,
      maxCoreIndex,
      desiredWindow,
      candles.length
    );
    let minSelectedPrice = Number.POSITIVE_INFINITY;
    let maxSelectedPrice = Number.NEGATIVE_INFINITY;
    let selectedPriceCount = 0;
    const addSelectedTradePrice = (value) => {
      const price = toFinitePrice(value);

      if (!Number.isFinite(price) || price <= 0) {
        return;
      }

      minSelectedPrice = Math.min(minSelectedPrice, price);
      maxSelectedPrice = Math.max(maxSelectedPrice, price);
      selectedPriceCount += 1;
    };

    for (let index = focusedWindow.minIndex; index <= focusedWindow.maxIndex; index += 1) {
      const candle = candles[index - 1];
      addSelectedTradePrice(candle?.candle_low);
      addSelectedTradePrice(candle?.candle_high);
    }

    [
      rustPattern?.trade_enter_price,
      rustPattern?.trade_risk_exit_price,
      rustPattern?.trade_reward_exit_price,
      rustPattern?.trade_exit_price,
      rustPattern?.trade_current_price,
      rustPattern?.target_close,
      rustPattern?.exit_price,
    ].forEach(addSelectedTradePrice);

    if (selectedPriceCount) {
      const minimumSpan = getMinimumFitPriceSpan(
        maxSelectedPrice,
        PROP_TRADE_MIN_FIT_PRICE_SPAN_RATIO,
        RAW_TRADE_MIN_PRICE_SPAN_ABSOLUTE
      );
      const pricePadding = Math.max(
        (maxSelectedPrice - minSelectedPrice) * 0.14,
        minimumSpan * 0.25
      );

      return {
        minPrice: minSelectedPrice - pricePadding,
        maxPrice: maxSelectedPrice + pricePadding,
        minIndex: focusedWindow.minIndex,
        maxIndex: focusedWindow.maxIndex,
        anchorIndex: (minCoreIndex + maxCoreIndex) / 2,
        centerFocusAtMidpoint: true,
        minPriceSpanRatio: PROP_TRADE_MIN_FIT_PRICE_SPAN_RATIO,
        minPriceSpanAbsolute: RAW_TRADE_MIN_PRICE_SPAN_ABSOLUTE,
      };
    }
  }
  const halfWindow = Math.floor(visibleCount / 2);
  let minIndex = 1;
  let maxIndex = visibleCount;

  if (hasFocusIndex && !showAllCandles) {
    minIndex = Math.max(1, Math.round(rawFocusIndex) - halfWindow);
    maxIndex = Math.min(candles.length, minIndex + visibleCount - 1);
    minIndex = Math.max(1, maxIndex - visibleCount + 1);
  }

  let minPrice = Number.POSITIVE_INFINITY;
  let maxPrice = Number.NEGATIVE_INFINITY;
  let priceCount = 0;

  const addFocusPrice = (value) => {
    const price = toFinitePrice(value);
    if (!Number.isFinite(price)) {
      return;
    }

    minPrice = Math.min(minPrice, price);
    maxPrice = Math.max(maxPrice, price);
    priceCount += 1;
  };

  for (let index = minIndex; index <= maxIndex; index += 1) {
    const candle = candles[index - 1];
    addFocusPrice(candle?.candle_low);
    addFocusPrice(candle?.candle_high);
  }

  [
    rustPattern?.trade_enter_price,
    rustPattern?.trade_risk_exit_price,
    rustPattern?.trade_reward_exit_price,
    rustPattern?.trade_exit_price,
    rustPattern?.exit_price,
  ].forEach(addFocusPrice);

  if (!priceCount) {
    return null;
  }

  const pricePadding = Math.max(
    (maxPrice - minPrice) * 0.06,
    getMinimumFitPriceSpan(maxPrice, PROP_TRADE_MIN_FIT_PRICE_SPAN_RATIO)
  );

  return {
    minPrice: minPrice - pricePadding,
    maxPrice: maxPrice + pricePadding,
    minIndex,
    maxIndex,
    anchorIndex: showAllCandles
      ? (minIndex + maxIndex) / 2
      : hasFocusIndex
        ? rawFocusIndex
        : Math.max(1, visibleCount / 2),
    centerFocusAtMidpoint: Boolean(rustPattern?.raw_center_focus_at_midpoint && hasFocusIndex),
    centerPriceAtMidpoint: Boolean(
      rustPattern?.raw_center_focus_at_midpoint && Number.isFinite(focusPrice)
    ),
    anchorPrice: focusPrice,
    minCompleteCandleWidth: showAllCandles ? MIN_RAW_ALL_FIT_COMPLETE_CANDLE_WIDTH : undefined,
  };
};

const applyHorizontalFit = (chartStateRef, minIndex, maxIndex, options = {}) => {
  const chartState = chartStateRef.current;
  const isGraphFocus = options.focusMode === 'graph';
  const horizontalPadding = isGraphFocus
    ? Math.max(chartState.canvas.width * 0.025, 8)
    : Math.min(
        CHART_FIT_PADDING.horizontal,
        Math.max(chartState.canvas.width * 0.08, 12)
      );
  const drawableWidth = Math.max(chartState.canvas.width - horizontalPadding * 2, 1);
  const coreSpanInCandles = Math.max(maxIndex - minIndex + 1, 1);
  const edgeBufferCandles = isGraphFocus
    ? Math.max(16, Math.min(54, coreSpanInCandles * 0.35))
    : options.focusMode === 'reversal'
      ? 2
      : 4;
  const spanInCandles = Math.max(coreSpanInCandles + edgeBufferCandles * 2, 1);
  const completeWidth = clamp(
    drawableWidth / spanInCandles,
    options.minCompleteCandleWidth ?? MIN_FIT_COMPLETE_CANDLE_WIDTH,
    MAX_FIT_COMPLETE_CANDLE_WIDTH
  );
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
    options.centerFocusAtMidpoint
      ? centeredXOrigin
      : lowerBoundXOrigin <= upperBoundXOrigin
        ? clamp(centeredXOrigin, lowerBoundXOrigin, upperBoundXOrigin)
        : centeredXOrigin;
  chartState.viewport.prevXOrigin = chartState.viewport.xOrigin;
};

const applyVerticalFit = (chartStateRef, minPrice, maxPrice, options = {}) => {
  const chartState = chartStateRef.current;
  const isPropTradeFocus = options.focusMode === 'propTrade';
  const isGraphFocus = options.focusMode === 'graph';
  const fitPriceSpanRatio =
    options.minPriceSpanRatio ??
    (isGraphFocus
      ? GRAPH_MIN_FIT_PRICE_SPAN_RATIO
      : isPropTradeFocus
        ? PROP_TRADE_MIN_FIT_PRICE_SPAN_RATIO
        : MIN_FIT_PRICE_SPAN_RATIO);
  const priceSpan = Math.max(
    maxPrice - minPrice,
    getMinimumFitPriceSpan(
      maxPrice,
      fitPriceSpanRatio,
      options.minPriceSpanAbsolute ?? MIN_FIT_PRICE_SPAN_ABSOLUTE
    )
  );
  const verticalTopPadding = Math.min(
    isGraphFocus ? 12 : isPropTradeFocus ? 18 : CHART_FIT_PADDING.verticalTop,
    Math.max(chartState.canvas.height * (isGraphFocus ? 0.015 : isPropTradeFocus ? 0.055 : 0.12), 3)
  );
  const verticalBottomPadding = Math.min(
    isGraphFocus ? 12 : isPropTradeFocus ? 16 : CHART_FIT_PADDING.verticalBottom,
    Math.max(chartState.canvas.height * (isGraphFocus ? 0.015 : isPropTradeFocus ? 0.05 : 0.1), 3)
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
  const anchorPrice = toFinitePrice(options.anchorPrice);
  chartState.viewport.baselineY =
    options.centerPriceAtMidpoint && Number.isFinite(anchorPrice)
      ? chartState.canvas.height / 2 + anchorPrice * priceScale
      : isGraphFocus
        ? verticalTopPadding + availableHeight / 2 + ((minPrice + maxPrice) / 2) * priceScale
        : verticalTopPadding + maxPrice * priceScale;
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

const applyVerticalZoomMultiplier = (chartStateRef, multiplier) => {
  const chartState = chartStateRef.current;
  const parsedMultiplier = Number(multiplier);

  if (!chartState || !Number.isFinite(parsedMultiplier) || parsedMultiplier <= 1) {
    return;
  }

  const zoomMultiplier = Math.min(parsedMultiplier, 48);
  chartState.price.pixelsPerGrid *= zoomMultiplier;
  chartState.price.prevPixelsPerGrid *= zoomMultiplier;
  chartState.price.startingPixelsPerGrid *= zoomMultiplier;
  chartState.price.priceUnitPixelSize *= zoomMultiplier;
  syncBaselineToMidPrice(chartStateRef);
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

export const scale_price_axis = (chartStateRef, multiplier) => {
  const chartState = chartStateRef.current;
  const parsedMultiplier = Number(multiplier);

  if (!chartState || !Number.isFinite(parsedMultiplier) || parsedMultiplier <= 0) {
    return;
  }

  const nextPixelsPerGrid = clamp(
    chartState.price.pixelsPerGrid * parsedMultiplier,
    Math.max(chartState.price.startingPixelsPerGrid * 0.12, 2),
    Math.max(chartState.price.startingPixelsPerGrid * 420, 420)
  );
  const appliedMultiplier = nextPixelsPerGrid / chartState.price.pixelsPerGrid;

  chartState.price.pixelsPerGrid = nextPixelsPerGrid;
  chartState.price.prevPixelsPerGrid *= appliedMultiplier;
  chartState.price.priceUnitPixelSize = Math.max(1, chartState.price.priceUnitPixelSize * appliedMultiplier);

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
    rustPattern?.raw_candle_view
      ? getRawCandleFocusBounds(chartStateRef, rustPattern)
    : options.focusMode === 'reversal'
      ? getReversalFocusBounds(chartStateRef, rustPattern, options.activeReversalFilter) ??
        getPatternBounds(rustPattern)
      : options.focusMode === 'graph'
        ? getGraphFocusBounds(chartStateRef, rustPattern) ?? getPatternBounds(rustPattern)
      : options.focusMode === 'propTrade'
        ? getPropTradeFocusBounds(chartStateRef, rustPattern) ??
          getPropFocusBounds(chartStateRef, rustPattern) ??
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

  applyVerticalFit(chartStateRef, bounds.minPrice, bounds.maxPrice, {
    ...options,
    centerPriceAtMidpoint: bounds.centerPriceAtMidpoint,
    anchorPrice: bounds.anchorPrice,
    minPriceSpanRatio: bounds.minPriceSpanRatio,
    minPriceSpanAbsolute: bounds.minPriceSpanAbsolute,
  });
  if (rustPattern?.raw_candle_view) {
    applyVerticalZoomMultiplier(chartStateRef, rustPattern?.raw_vertical_zoom);
  }
  applyHorizontalFit(chartStateRef, bounds.minIndex, bounds.maxIndex, {
    ...options,
    minCompleteCandleWidth: bounds.minCompleteCandleWidth,
    centerFocusAtMidpoint: bounds.centerFocusAtMidpoint,
    anchorIndex:
      bounds.anchorIndex ??
      (options.focusMode === 'reversal' && Number.isFinite(reversalAnchorIndex)
        ? reversalAnchorIndex
        : (options.focusMode === 'prop' || options.focusMode === 'propTrade') &&
            Number.isFinite(propAnchorIndex)
          ? propAnchorIndex
        : undefined),
  });
  chartStateRef.current.pattern.length = rustPattern?.pattern_ABCD_bar_length ?? 0;
};
