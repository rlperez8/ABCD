const HORIZONTAL_GRID_TARGET_PX = 108;
const MIN_VISIBLE_CANDLE_GAP = 0;
const TREND_SMA_PERIODS = {
  threeMonth: 63,
  sixMonth: 126,
  twelveMonth: 252,
};

const getNiceCandleIncrement = (rawIncrement) => {
  if (!Number.isFinite(rawIncrement) || rawIncrement <= 1) {
    return 1;
  }

  const magnitude = 10 ** Math.floor(Math.log10(rawIncrement));
  const normalized = rawIncrement / magnitude;

  if (normalized <= 1) return magnitude;
  if (normalized <= 2) return magnitude * 2;
  if (normalized <= 5) return magnitude * 5;
  return magnitude * 10;
};

const buildTrendSeries = (candles, period) => {
  const values = Array(candles.length).fill(null);
  const chronologicalCandles = [...candles].reverse();
  let rollingSum = 0;

  chronologicalCandles.forEach((candle, index) => {
    rollingSum += candle?.candle_close ?? 0;

    if (index >= period) {
      rollingSum -= chronologicalCandles[index - period]?.candle_close ?? 0;
    }

    if (index >= period - 1) {
      values[candles.length - 1 - index] = rollingSum / period;
    }
  });

  return values;
};

export const syncHorizontalGrid = (chartState) => {
  const completeWidth = Math.max(chartState.candles.completeWidth, 0.01);
  const targetGridPx = completeWidth >= 42 ? 42 : HORIZONTAL_GRID_TARGET_PX;
  const rawGridIncrement = targetGridPx / completeWidth;

  chartState.viewport.xGridIncrement = getNiceCandleIncrement(rawGridIncrement);
  chartState.viewport.xGridWidth =
    chartState.candles.completeWidth * chartState.viewport.xGridIncrement;
};

export const setCandleSpacing = (chartState, width, spacing) => {
  const normalizedSpacing = Math.max(spacing, MIN_VISIBLE_CANDLE_GAP);
  chartState.candles.width = width;
  chartState.candles.spacing = normalizedSpacing;
  chartState.candles.completeWidth = width + normalizedSpacing;
  syncHorizontalGrid(chartState);
};

export const commitInteractionState = (chartState) => {
  chartState.viewport.prevXOrigin = chartState.viewport.xOrigin;
  chartState.viewport.prevBaselineY = chartState.viewport.baselineY;
  chartState.price.prevMidPrice = chartState.price.currentMidPrice;
  chartState.price.prevPixelsPerGrid = chartState.price.pixelsPerGrid;
};

export const createChartState = ({ canvasWidth, canvasHeight, candles }) => {
  const initialCandleWidth = 11;
  const initialSpacing = 5;
  const initialPrice = candles[0]?.candle_open ?? 0;
  const pixelsPerGrid = Math.max(canvasHeight / 10, 1);

  const chartState = {
    canvas: {
      width: canvasWidth,
      height: canvasHeight,
    },
    mouse: {
      down: { x: 0, y: 0 },
      pos: { x: 0, y: 0 },
      isPressed: false,
      isPressedOnPrices: false,
    },
    candles: {
      items: candles,
      trendLines: {
        threeMonth: buildTrendSeries(candles, TREND_SMA_PERIODS.threeMonth),
        sixMonth: buildTrendSeries(candles, TREND_SMA_PERIODS.sixMonth),
        twelveMonth: buildTrendSeries(candles, TREND_SMA_PERIODS.twelveMonth),
      },
      width: initialCandleWidth,
      spacing: initialSpacing,
      completeWidth: initialCandleWidth + initialSpacing,
    },
    price: {
      unitAmount: 1,
      startingPixelsPerGrid: pixelsPerGrid,
      pixelsPerGrid,
      priceUnitPixelSize: pixelsPerGrid,
      prevPixelsPerGrid: pixelsPerGrid,
      currentMidPrice: initialPrice,
      prevMidPrice: initialPrice,
      staticMidPrice: initialPrice,
    },
    viewport: {
      baselineY: canvasHeight / 2 + initialPrice * pixelsPerGrid,
      prevBaselineY: canvasHeight / 2 + initialPrice * pixelsPerGrid,
      startingBaselineY: canvasHeight,
      xOrigin: 0,
      prevXOrigin: 0,
      gridWidth: canvasWidth,
      xGridIncrement: 10,
      xGridWidth: (initialCandleWidth + initialSpacing) * 10,
    },
    pattern: {
      length: 0,
    },
  };

  syncHorizontalGrid(chartState);
  return chartState;
};
