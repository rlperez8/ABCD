const GRID_LEVELS = [
  { minWidth: 24, increaser: 5 },
  { minWidth: 16, increaser: 8 },
  { minWidth: 10, increaser: 10 },
  { minWidth: 6, increaser: 15 },
  { minWidth: 0, increaser: 20 },
];
const MIN_VISIBLE_CANDLE_GAP = 0;
const TREND_SMA_PERIODS = {
  threeMonth: 63,
  sixMonth: 126,
  twelveMonth: 252,
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
  const matchingGridLevel = GRID_LEVELS.find(
    (level) => chartState.candles.completeWidth >= level.minWidth
  );

  chartState.viewport.xGridIncrement = matchingGridLevel?.increaser ?? 10;
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
