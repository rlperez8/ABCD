import { commitInteractionState, setCandleSpacing } from './chartState.js';
import { clamp, getCanvasX } from './geometry.js';

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
  const widthStep = 0.8 * zoomAmount;
  const spacingStep = 0.35 * zoomAmount;
  const nextWidth = clamp(chartState.candles.width + direction * widthStep, 2, 72);
  const nextSpacing = clamp(chartState.candles.spacing + direction * spacingStep, 1, 20);

  setCandleSpacing(chartState, nextWidth, nextSpacing);

  chartState.viewport.xOrigin =
    -chartState.candles.completeWidth * anchorIndex +
    chartState.candles.completeWidth / 2 -
    anchorScreenX;
  chartState.viewport.prevXOrigin = chartState.viewport.xOrigin;
};
