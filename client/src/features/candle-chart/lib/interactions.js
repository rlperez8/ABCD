import { commitInteractionState, setCandleSpacing } from './chartState.js';
import { clamp, getCanvasX } from './geometry.js';

const MIN_COMPLETE_CANDLE_WIDTH = 0.18;
const MAX_COMPLETE_CANDLE_WIDTH = 260;

const getCandleSlotParts = (completeWidth) => {
  if (completeWidth <= 1.1) {
    return { width: completeWidth, spacing: 0 };
  }

  const spacing = Math.min(completeWidth * 0.16, 56);
  return {
    width: Math.max(completeWidth - spacing, Math.min(completeWidth, 0.12)),
    spacing,
  };
};

export const setMousePosition = (chartState, canvasElement, event) => {
  const rect = canvasElement.getBoundingClientRect();
  chartState.mouse.pos.x = event.clientX - rect.left;
  chartState.mouse.pos.y = event.clientY - rect.top;
};

export const startChartDrag = (chartState) => {
  chartState.mouse.isPressed = true;
  chartState.mouse.down.x = chartState.mouse.pos.x;
  chartState.mouse.down.y = chartState.mouse.pos.y;
};

export const startPriceDrag = (chartState) => {
  chartState.mouse.isPressedOnPrices = true;
  chartState.mouse.down.x = chartState.mouse.pos.x;
  chartState.mouse.down.y = chartState.mouse.pos.y;
};

export const endChartDrag = (chartState) => {
  chartState.mouse.isPressed = false;
  commitInteractionState(chartState);
};

export const endPriceDrag = (chartState) => {
  chartState.mouse.isPressedOnPrices = false;
  commitInteractionState(chartState);
};

export const zoomCandleWidth = (chartState, deltaY, hoveredIndex, fallbackIndex = 1) => {
  const anchorIndex = Number.isFinite(hoveredIndex) && hoveredIndex > 0 ? hoveredIndex : fallbackIndex;
  const anchorScreenX = getCanvasX(chartState, anchorIndex);
  const direction = deltaY < 0 ? 1 : -1;
  const zoomAmount = clamp(Math.abs(deltaY) / 120, 0.65, 2.4);
  const currentCompleteWidth = chartState.candles.completeWidth || 1;
  const zoomRatio = Math.pow(1.16, zoomAmount * direction);
  const nextCompleteWidth = clamp(
    currentCompleteWidth * zoomRatio,
    MIN_COMPLETE_CANDLE_WIDTH,
    MAX_COMPLETE_CANDLE_WIDTH
  );
  const { width, spacing } = getCandleSlotParts(nextCompleteWidth);

  setCandleSpacing(chartState, width, spacing);

  chartState.viewport.xOrigin =
    -chartState.candles.completeWidth * anchorIndex +
    chartState.candles.completeWidth / 2 -
    anchorScreenX;
  chartState.viewport.prevXOrigin = chartState.viewport.xOrigin;
};
