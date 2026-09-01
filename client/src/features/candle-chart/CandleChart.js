import React, { useEffect, useRef, useState } from 'react';
import * as canvasTools from './lib/canvasTools.js';
import { Mouse, Chart, ABCD } from './lib/draw.js';
import * as resize from './lib/resize.js';
import { createChartState } from './lib/chartState.js';
import {
  endChartDrag,
  endPriceDrag,
  setMousePosition,
  startChartDrag,
  startPriceDrag,
  zoomCandleWidth,
} from './lib/interactions.js';
import { getCanvasX, getCanvasY } from './lib/geometry.js';

const hasRenderablePattern = (chartData) =>
  Boolean(chartData?.candles?.length && chartData?.rust_patterns);

const getChartLayoutKey = ({ canvasWidth, canvasHeight, priceWidth, priceHeight, dateWidth, dateHeight }) =>
  [
    canvasWidth,
    canvasHeight,
    priceWidth,
    priceHeight,
    dateWidth,
    dateHeight,
  ].join('x');

const getChartStateLayoutKey = (chartState) =>
  chartState
    ? getChartLayoutKey({
        canvasWidth: chartState.canvas?.width,
        canvasHeight: chartState.canvas?.height,
        priceWidth: chartState.canvas?.priceWidth,
        priceHeight: chartState.canvas?.priceHeight,
        dateWidth: chartState.canvas?.dateWidth,
        dateHeight: chartState.canvas?.dateHeight,
      })
    : '';

const restoreRawViewport = (nextState, previousState, { prependCandleCount = 0 } = {}) => {
  if (!nextState || !previousState) {
    return;
  }

  nextState.candles.width = previousState.candles.width;
  nextState.candles.spacing = previousState.candles.spacing;
  nextState.candles.completeWidth = previousState.candles.completeWidth;
  nextState.viewport.xGridIncrement = previousState.viewport.xGridIncrement;
  nextState.viewport.xGridWidth = previousState.viewport.xGridWidth;
  nextState.viewport.xOrigin = previousState.viewport.xOrigin;
  nextState.viewport.prevXOrigin = previousState.viewport.prevXOrigin;
  nextState.viewport.gridWidth = previousState.viewport.gridWidth;
  nextState.viewport.baselineY = previousState.viewport.baselineY;
  nextState.viewport.prevBaselineY = previousState.viewport.prevBaselineY;
  nextState.viewport.startingBaselineY = previousState.viewport.startingBaselineY;
  nextState.price.unitAmount = previousState.price.unitAmount;
  nextState.price.startingPixelsPerGrid = previousState.price.startingPixelsPerGrid;
  nextState.price.pixelsPerGrid = previousState.price.pixelsPerGrid;
  nextState.price.priceUnitPixelSize = previousState.price.priceUnitPixelSize;
  nextState.price.prevPixelsPerGrid = previousState.price.prevPixelsPerGrid;
  nextState.price.currentMidPrice = previousState.price.currentMidPrice;
  nextState.price.prevMidPrice = previousState.price.prevMidPrice;
  nextState.price.staticMidPrice = previousState.price.staticMidPrice;

  const prependCount = Number(prependCandleCount);
  if (Number.isFinite(prependCount) && prependCount > 0) {
    const xOffset = nextState.candles.completeWidth * prependCount;
    nextState.viewport.xOrigin -= xOffset;
    nextState.viewport.prevXOrigin -= xOffset;
  }
};

export const CandleChart = ({
  chartData,
  is_price_levels,
  is_retracement,
  is_abcd_pattern,
  is_reversal_focus,
  trend_line_toggles,
  focusMode = 'pattern',
  propFocusScope = 'trade',
  market,
  activeReversalFilter,
  set_hovered_candle,
  showCandles = true,
  presentationMode = 'chart',
  routeLogicHover = null,
  onRawCandleViewportEdge = null,
}) => {
  const canvasDatesRef = useRef(null);
  const canvasPriceRef = useRef(null);
  const canvasChartRef = useRef(null);
  const hoveredCandleIndexRef = useRef(1);
  const chartStateRef = useRef(null);
  const rawViewportKeyRef = useRef('');
  const manualRawViewportRef = useRef(false);
  const manualRawCursorRef = useRef(false);
  const touchGestureRef = useRef({ distance: 0 });
  const priceAxisGestureRef = useRef({ y: 0, carry: 0 });
  const [chartReadyVersion, setChartReadyVersion] = useState(0);
  const selectedPattern = chartData?.rust_patterns ?? null;
  const isRawCandleView = Boolean(selectedPattern?.raw_candle_view);
  const hasCandles = Boolean(chartData?.candles?.length);
  const hasPatternPivots = ['x', 'a', 'b', 'c', 'd'].every((key) => {
    const value = Number(selectedPattern?.[key]);
    return Number.isFinite(value) && value >= 1;
  });
  const canDrawPatternGeometry = hasPatternPivots && !isRawCandleView;
  const effectiveFocusMode =
    presentationMode === 'graph' && canDrawPatternGeometry
      ? 'graph'
      : focusMode === 'prop'
      ? propFocusScope === 'trade'
        ? 'propTrade'
        : 'prop'
      : is_reversal_focus
        ? 'reversal'
        : 'pattern';
  console.log(chartData)
  useEffect(() => {
    if (!hasRenderablePattern(chartData)) {
      chartStateRef.current = null;
      return undefined;
    }

    let lastInitializedLayoutKey = '';

    const initializeChartState = ({ force = false } = {}) => {
      if (!canvasChartRef.current || !canvasPriceRef.current || !canvasDatesRef.current) {
        return false;
      }

      const canvasBounds = canvasChartRef.current.getBoundingClientRect();
      const canvasWidth = Math.round(canvasBounds.width || canvasChartRef.current.offsetWidth || 0);
      const canvasHeight = Math.round(canvasBounds.height || canvasChartRef.current.offsetHeight || 0);
      const priceBounds = canvasPriceRef.current.getBoundingClientRect();
      const dateBounds = canvasDatesRef.current.getBoundingClientRect();
      const priceWidth = Math.round(priceBounds.width || canvasPriceRef.current.offsetWidth || 0);
      const priceHeight = Math.round(priceBounds.height || canvasPriceRef.current.offsetHeight || 0);
      const dateWidth = Math.round(dateBounds.width || canvasDatesRef.current.offsetWidth || 0);
      const dateHeight = Math.round(dateBounds.height || canvasDatesRef.current.offsetHeight || 0);

      if (
        canvasWidth <= 0 ||
        canvasHeight <= 0 ||
        priceWidth <= 0 ||
        priceHeight <= 0 ||
        dateWidth <= 0 ||
        dateHeight <= 0
      ) {
        return false;
      }

      const layoutKey = getChartLayoutKey({
        canvasWidth,
        canvasHeight,
        priceWidth,
        priceHeight,
        dateWidth,
        dateHeight,
      });

      if (!force && layoutKey === lastInitializedLayoutKey) {
        return true;
      }

      lastInitializedLayoutKey = layoutKey;
      const previousChartState = chartStateRef.current;
      const rawViewportKey = selectedPattern?.raw_viewport_key ?? '';
      const rawViewportKeyChanged = rawViewportKeyRef.current !== rawViewportKey;
      if (rawViewportKeyChanged) {
        manualRawViewportRef.current = false;
        manualRawCursorRef.current = false;
      }
      const candleCountMatches =
        previousChartState?.candles?.items?.length === chartData.candles.length;
      const canPreserveRawViewportAcrossCountChange =
        Boolean(selectedPattern?.raw_preserve_viewport_on_count_change);
      const shouldFollowLatest =
        Boolean(selectedPattern?.raw_follow_latest) && !manualRawViewportRef.current;
      const shouldRestoreRawViewport =
        isRawCandleView &&
        !shouldFollowLatest &&
        getChartStateLayoutKey(previousChartState) === layoutKey &&
        (candleCountMatches || canPreserveRawViewportAcrossCountChange) &&
        rawViewportKeyRef.current === rawViewportKey;

      canvasTools.reset_candle_canvas(canvasChartRef);
      canvasTools.reset_price_canvas(canvasPriceRef);
      canvasTools.reset_date_canvas(canvasDatesRef);

      chartStateRef.current = createChartState({
        canvasWidth,
        canvasHeight,
        candles: chartData.candles,
      });
      chartStateRef.current.theme = isRawCandleView ? 'win95' : 'dark';
      chartStateRef.current.canvas.priceWidth = priceWidth;
      chartStateRef.current.canvas.priceHeight = priceHeight;
      chartStateRef.current.canvas.dateWidth = dateWidth;
      chartStateRef.current.canvas.dateHeight = dateHeight;

      if (manualRawCursorRef.current && previousChartState?.mouse) {
        chartStateRef.current.mouse.pos = { ...previousChartState.mouse.pos };
        chartStateRef.current.mouse.down = { ...previousChartState.mouse.down };
      }

      if (shouldRestoreRawViewport) {
        restoreRawViewport(chartStateRef.current, previousChartState, {
          prependCandleCount: selectedPattern?.raw_prepend_candle_count,
        });
      } else {
        resize.reposition_candles(chartStateRef, chartData.rust_patterns, {
          focusMode: effectiveFocusMode,
          activeReversalFilter,
        });
      }
      rawViewportKeyRef.current = rawViewportKey;
      setChartReadyVersion((current) => current + 1);
      return true;
    };

    let animationFrameId = null;
    const requestInitializeChartState = () => {
      if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
      }

      animationFrameId = requestAnimationFrame(() => {
        animationFrameId = null;
        initializeChartState();
      });
    };

    initializeChartState({ force: true });
    requestInitializeChartState();

    const ResizeObserverClass = window.ResizeObserver;
    const resizeObserver = ResizeObserverClass
      ? new ResizeObserverClass(requestInitializeChartState)
      : null;

    if (resizeObserver) {
      resizeObserver.observe(canvasChartRef.current);
      resizeObserver.observe(canvasPriceRef.current);
      resizeObserver.observe(canvasDatesRef.current);
    }

    window.addEventListener('resize', requestInitializeChartState);

    return () => {
      if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
      }

      resizeObserver?.disconnect();
      window.removeEventListener('resize', requestInitializeChartState);
    };
  }, [
    activeReversalFilter,
    chartData,
    effectiveFocusMode,
    isRawCandleView,
    selectedPattern?.raw_prepend_candle_count,
    selectedPattern?.raw_preserve_viewport_on_count_change,
    selectedPattern?.raw_follow_latest,
    selectedPattern?.raw_viewport_key,
  ]);

  useEffect(() => {
    if (!hasCandles || !selectedPattern || !chartStateRef.current) {
      return;
    }
    if (selectedPattern.raw_candle_view) {
      return;
    }

    resize.reposition_candles(chartStateRef, selectedPattern, {
      focusMode: effectiveFocusMode,
      activeReversalFilter,
    });
  }, [activeReversalFilter, effectiveFocusMode, hasCandles, selectedPattern]);

  useEffect(() => {
    if (!hasRenderablePattern(chartData) || !chartStateRef.current) {
      return undefined;
    }

    const candleReset = canvasTools.reset_candle_canvas(canvasChartRef);
    const priceReset = canvasTools.reset_price_canvas(canvasPriceRef);
    const dateReset = canvasTools.reset_date_canvas(canvasDatesRef);

    if (!candleReset || !priceReset || !dateReset) {
      return undefined;
    }

    const { canvas, ctx } = candleReset;
    const { cp, ctx_price } = priceReset;
    const { canvas_date, ctx_date } = dateReset;
    const canvasElement = canvas.element ?? canvasChartRef.current;
    const priceElement = cp.element ?? canvasPriceRef.current;

    const mouseLayer = new Mouse(chartStateRef);
    const chartLayer = new Chart(chartStateRef);
    const patternLayer = new ABCD(chartStateRef);
    const showPatternOverlay = is_abcd_pattern && canDrawPatternGeometry;
    const showRetracementOverlay = is_retracement && canDrawPatternGeometry;
    const showPriceLevelRays = is_price_levels && !isRawCandleView;
    const showPriceLevelTags = (focusMode === 'prop' || is_price_levels) && !isRawCandleView;
    const isGraphPresentation = presentationMode === 'graph';

    let animationFrameId = null;

    const requestDraw = () => {
      if (!animationFrameId) {
        animationFrameId = requestAnimationFrame(draw);
      }
    };

    const markManualRawViewport = () => {
      if (isRawCandleView && chartData.rust_patterns?.raw_follow_latest) {
        manualRawViewportRef.current = true;
      }
    };

    const markManualRawCursor = () => {
      if (isRawCandleView && chartData.rust_patterns?.raw_follow_latest) {
        manualRawCursorRef.current = true;
      }
    };

    const markManualRawChartInteraction = () => {
      markManualRawViewport();
      markManualRawCursor();
    };

    const pinRawCursorToLatestCandle = () => {
      const chartState = chartStateRef.current;

      if (
        !chartState ||
        !isRawCandleView ||
        !chartData.rust_patterns?.raw_follow_latest ||
        manualRawCursorRef.current
      ) {
        return;
      }

      const latestIndex = 1;
      const latestCandle = chartState.candles.items[latestIndex - 1];
      const latestPrice = Number(
        latestCandle?.candle_close ??
          latestCandle?.close ??
          latestCandle?.candle_open ??
          latestCandle?.open
      );

      chartState.mouse.pos.x = getCanvasX(chartState, latestIndex);
      chartState.mouse.pos.y = Number.isFinite(latestPrice)
        ? getCanvasY(chartState, latestPrice)
        : chartState.canvas.height / 2;
    };

    const notifyRawCandleViewportEdge = () => {
      if (!isRawCandleView || !onRawCandleViewportEdge || !chartStateRef.current) {
        return;
      }

      const chartLayer = new Chart(chartStateRef);
      const range = chartLayer.getVisibleCandleRange(canvas.width, 0);
      const total = chartStateRef.current.candles.items.length;
      const threshold = Math.max(80, Math.floor((range.end - range.start + 1) * 0.18));

      if (range.end >= total - threshold) {
        onRawCandleViewportEdge({
          edge: 'older',
          start: range.start,
          end: range.end,
          total,
        });
      }
    };

    const draw = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx_price.clearRect(0, 0, cp.width, cp.height);
      ctx_date.clearRect(0, 0, canvas_date.width, canvas_date.height);

      if (isGraphPresentation) {
        chartLayer.graphBackground(ctx, canvas);
      }

      if (focusMode === 'prop') {
        chartLayer.tradeEntryColumn(ctx, chartData.rust_patterns);
        chartLayer.tradeSourceExitColumn(ctx, chartData.rust_patterns);
        chartLayer.tradeExitColumn(ctx, chartData.rust_patterns);
      }

      if (showCandles) {
        chartLayer.candles(ctx, chartData.rust_patterns, {
          reversalFocusOnly: is_reversal_focus,
          activeReversalFilter,
          highlightTradeCandles: focusMode === 'prop',
        });
      }
      chartLayer.prices(ctx_price, cp);
      chartLayer.dates(ctx_date, canvas_date, chartData.rust_patterns);
      chartLayer.grid_X(ctx, canvas, chartData.rust_patterns);
      chartLayer.grid_Y(ctx, canvas);

      if (
        trend_line_toggles?.threeMonth ||
        trend_line_toggles?.sixMonth ||
        trend_line_toggles?.twelveMonth
      ) {
        chartLayer.trend_lines(ctx, canvas, trend_line_toggles);
      }

      if (isGraphPresentation) {
        patternLayer.graph_retracements(ctx, chartData.rust_patterns);
        patternLayer.graph_trade_levels(ctx, canvas, chartData.rust_patterns);
        patternLayer.xa_scan_start_beam(ctx, chartData.rust_patterns);
      }

      pinRawCursorToLatestCandle();
      mouseLayer.mouse_Y(canvas, ctx);
      mouseLayer.mouse_X(canvas, ctx, hoveredCandleIndexRef, set_hovered_candle);
      mouseLayer.price_background(cp, ctx_price);
      mouseLayer.mouse_price(cp, ctx_price);
      mouseLayer.date_background(ctx_date, canvas_date);
      mouseLayer.mouse_date(canvas_date, ctx_date);

      if (focusMode === 'prop') {
        patternLayer.drawSetupOverlay(ctx, chartData.rust_patterns, {
          presentationMode,
        });
        patternLayer.route_logic_highlight(ctx, chartData.rust_patterns, routeLogicHover);
        patternLayer.trade_path(ctx, chartData.rust_patterns);
        patternLayer.prop_events(ctx, chartData.rust_patterns);
      }

      if (showPatternOverlay && focusMode !== 'prop') {
        if (market === 'Bullish') {
          patternLayer.bull_abcd(ctx, chartData.rust_patterns);
        } else if (market === 'Bearish') {
          patternLayer.abcd(ctx, chartData.rust_patterns);
        }
      }

      if (showRetracementOverlay) {
        patternLayer.retracement(ctx, chartData.rust_patterns);
      }

      if (is_reversal_focus || activeReversalFilter) {
        patternLayer.reversal_signal(ctx, chartData.rust_patterns, activeReversalFilter);
      }

      if (isGraphPresentation) {
        patternLayer.price_levels(ctx_price, ctx, canvas, chartData.rust_patterns, {
          showRays: false,
          showTags: true,
        });
      } else if (showPriceLevelRays || showPriceLevelTags) {
        patternLayer.price_levels(ctx_price, ctx, canvas, chartData.rust_patterns, {
          showRays: showPriceLevelRays,
          showTags: showPriceLevelTags,
        });
      }

      if (isRawCandleView) {
        patternLayer.raw_live_time_slot(ctx, chartData.rust_patterns);
        patternLayer.raw_scanner_checks(ctx, chartData.rust_patterns);
        patternLayer.raw_stage2_window(ctx, chartData.rust_patterns);
        patternLayer.raw_watching_trends(ctx, chartData.rust_patterns);
      }

      chartLayer.last_price_line(ctx, canvas, chartData.rust_patterns);
      chartLayer.last_price_axis(ctx_price, cp, chartData.rust_patterns);

      if (isRawCandleView) {
        patternLayer.raw_trend_event(ctx, ctx_price, chartData.rust_patterns);
      }

      notifyRawCandleViewportEdge();

      animationFrameId = null;
    };

    const handleMouseMove = (event) => {
      setMousePosition(chartStateRef.current, canvasElement, event);
      markManualRawCursor();

      if (chartStateRef.current.mouse.isPressed) {
        markManualRawChartInteraction();
        const pixelsMovedX =
          chartStateRef.current.mouse.down.x - chartStateRef.current.mouse.pos.x;

        chartStateRef.current.viewport.xOrigin =
          chartStateRef.current.viewport.prevXOrigin + pixelsMovedX;
        resize.chart_Y_movement(chartStateRef);
      }

      requestDraw();
    };

    const updateDragFromPoint = (point) => {
      setMousePosition(chartStateRef.current, canvasElement, point);
      markManualRawCursor();

      if (chartStateRef.current.mouse.isPressed) {
        markManualRawChartInteraction();
        const pixelsMovedX =
          chartStateRef.current.mouse.down.x - chartStateRef.current.mouse.pos.x;

        chartStateRef.current.viewport.xOrigin =
          chartStateRef.current.viewport.prevXOrigin + pixelsMovedX;
        resize.chart_Y_movement(chartStateRef);
      }

      requestDraw();
    };

    const getTouchDistance = (touches) => {
      if (!touches || touches.length < 2) {
        return 0;
      }

      return Math.hypot(
        touches[0].clientX - touches[1].clientX,
        touches[0].clientY - touches[1].clientY
      );
    };

    const getTouchMidpoint = (touches) => ({
      clientX: (touches[0].clientX + touches[1].clientX) / 2,
      clientY: (touches[0].clientY + touches[1].clientY) / 2,
    });

    const handleCanvasTouchStart = (event) => {
      if (!chartStateRef.current) {
        return;
      }

      if (event.touches.length === 1) {
        event.preventDefault();
        markManualRawChartInteraction();
        touchGestureRef.current = { distance: 0 };
        setMousePosition(chartStateRef.current, canvasElement, event.touches[0]);
        startChartDrag(chartStateRef.current);
        requestDraw();
        return;
      }

      if (event.touches.length >= 2) {
        event.preventDefault();
        markManualRawChartInteraction();
        endChartDrag(chartStateRef.current);
        touchGestureRef.current = { distance: getTouchDistance(event.touches) };
        setMousePosition(
          chartStateRef.current,
          canvasElement,
          getTouchMidpoint(event.touches)
        );
        requestDraw();
      }
    };

    const handleCanvasTouchMove = (event) => {
      if (!chartStateRef.current) {
        return;
      }

      if (event.touches.length >= 2) {
        event.preventDefault();
        const nextDistance = getTouchDistance(event.touches);
        const previousDistance = touchGestureRef.current.distance || nextDistance;
        const distanceDelta = nextDistance - previousDistance;

        setMousePosition(
          chartStateRef.current,
          canvasElement,
          getTouchMidpoint(event.touches)
        );

        if (Math.abs(distanceDelta) >= 3) {
          markManualRawChartInteraction();
          zoomCandleWidth(
            chartStateRef.current,
            -distanceDelta,
            hoveredCandleIndexRef.current,
            chartData.rust_patterns?.target ??
              chartData.rust_patterns?.reversal_detect ??
              chartData.rust_patterns?.d_confirm ??
              chartData.rust_patterns?.a ??
              1
          );
          touchGestureRef.current.distance = nextDistance;
        }

        requestDraw();
        return;
      }

      if (event.touches.length === 1) {
        event.preventDefault();
        markManualRawChartInteraction();
        updateDragFromPoint(event.touches[0]);
      }
    };

    const handleCanvasTouchEnd = (event) => {
      if (!chartStateRef.current) {
        return;
      }

      if (event.touches.length === 0) {
        touchGestureRef.current = { distance: 0 };
        endChartDrag(chartStateRef.current);
        requestDraw();
        return;
      }

      if (event.touches.length === 1) {
        touchGestureRef.current = { distance: 0 };
        markManualRawChartInteraction();
        setMousePosition(chartStateRef.current, canvasElement, event.touches[0]);
        startChartDrag(chartStateRef.current);
        requestDraw();
      }
    };

    const handlePriceZoom = (event) => {
      event.preventDefault();
      markManualRawViewport();

      const threshold = Math.floor(chartStateRef.current.price.startingPixelsPerGrid * 0.5);
      const expandThreshold = Math.floor(chartStateRef.current.price.startingPixelsPerGrid * 1.5);
      const stepCount = Math.min(Math.max(Math.ceil(Math.abs(event.deltaY) / 70), 1), 3);

      for (let step = 0; step < stepCount; step += 1) {
        if (event.deltaY < 0) {
          resize.chart_zoom_in(chartStateRef, expandThreshold);
        } else if (event.deltaY > 0) {
          resize.chart_zoom_out(chartStateRef, threshold);
        }
      }

      requestDraw();
    };

    const applyPriceAxisDragZoom = (deltaY) => {
      if (!chartStateRef.current || !Number.isFinite(deltaY)) {
        return;
      }

      markManualRawViewport();
      if (Math.abs(deltaY) < 0.2) {
        return;
      }

      const dragSensitivity = 1 / 28;
      const rawMultiplier = Math.exp(-deltaY * dragSensitivity);
      const zoomMultiplier = Math.min(Math.max(rawMultiplier, 0.45), 2.4);
      resize.scale_price_axis(chartStateRef, zoomMultiplier);

      requestDraw();
    };

    const handlePriceTouchStart = (event) => {
      if (!chartStateRef.current || !event.touches.length) {
        return;
      }

      event.preventDefault();
      markManualRawViewport();
      const touch = event.touches[0];
      startPriceDrag(chartStateRef.current);
      priceAxisGestureRef.current = { y: touch.clientY, carry: 0 };
      requestDraw();
    };

    const handlePriceTouchMove = (event) => {
      if (!chartStateRef.current || !event.touches.length) {
        return;
      }

      event.preventDefault();
      markManualRawViewport();
      const touch = event.touches[0];
      const previousY = priceAxisGestureRef.current.y || touch.clientY;
      const deltaY = touch.clientY - previousY;
      priceAxisGestureRef.current.y = touch.clientY;
      applyPriceAxisDragZoom(deltaY);
    };

    const handlePriceTouchEnd = (event) => {
      if (!chartStateRef.current) {
        return;
      }

      if (!event.touches.length) {
        priceAxisGestureRef.current = { y: 0, carry: 0 };
        endPriceDrag(chartStateRef.current);
        requestDraw();
      }
    };

    const handleWidthZoom = (event) => {
      event.preventDefault();
      markManualRawChartInteraction();
      zoomCandleWidth(
        chartStateRef.current,
        event.deltaY,
        hoveredCandleIndexRef.current,
        chartData.rust_patterns?.target ??
          chartData.rust_patterns?.reversal_detect ??
          chartData.rust_patterns?.d_confirm ??
          chartData.rust_patterns?.a ??
          1
      );
      requestDraw();
    };

    const handleWindowResize = () => {
      if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
      }

      animationFrameId = requestAnimationFrame(draw);
    };

    canvasElement.addEventListener('mousemove', handleMouseMove);
    canvasElement.addEventListener('wheel', handleWidthZoom, { passive: false });
    canvasElement.addEventListener('touchstart', handleCanvasTouchStart, { passive: false });
    canvasElement.addEventListener('touchmove', handleCanvasTouchMove, { passive: false });
    canvasElement.addEventListener('touchend', handleCanvasTouchEnd, { passive: false });
    canvasElement.addEventListener('touchcancel', handleCanvasTouchEnd, { passive: false });
    priceElement.addEventListener('wheel', handlePriceZoom, { passive: false });
    priceElement.addEventListener('touchstart', handlePriceTouchStart, { passive: false });
    priceElement.addEventListener('touchmove', handlePriceTouchMove, { passive: false });
    priceElement.addEventListener('touchend', handlePriceTouchEnd, { passive: false });
    priceElement.addEventListener('touchcancel', handlePriceTouchEnd, { passive: false });
    window.addEventListener('resize', handleWindowResize);

    draw();

    return () => {
      if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
      }

      canvasElement.removeEventListener('mousemove', handleMouseMove);
      canvasElement.removeEventListener('wheel', handleWidthZoom);
      canvasElement.removeEventListener('touchstart', handleCanvasTouchStart);
      canvasElement.removeEventListener('touchmove', handleCanvasTouchMove);
      canvasElement.removeEventListener('touchend', handleCanvasTouchEnd);
      canvasElement.removeEventListener('touchcancel', handleCanvasTouchEnd);
      priceElement.removeEventListener('wheel', handlePriceZoom);
      priceElement.removeEventListener('touchstart', handlePriceTouchStart);
      priceElement.removeEventListener('touchmove', handlePriceTouchMove);
      priceElement.removeEventListener('touchend', handlePriceTouchEnd);
      priceElement.removeEventListener('touchcancel', handlePriceTouchEnd);
      window.removeEventListener('resize', handleWindowResize);
    };
  }, [
    chartData,
    canDrawPatternGeometry,
    is_abcd_pattern,
    is_price_levels,
    isRawCandleView,
    is_retracement,
    is_reversal_focus,
    trend_line_toggles,
    focusMode,
    propFocusScope,
    effectiveFocusMode,
    presentationMode,
    market,
    activeReversalFilter,
    set_hovered_candle,
    showCandles,
    routeLogicHover,
    onRawCandleViewportEdge,
    chartReadyVersion,
  ]);

  const handleChartMouseDown = (event) => {
    if (!chartStateRef.current) {
      return;
    }

    if (isRawCandleView && selectedPattern?.raw_follow_latest) {
      manualRawViewportRef.current = true;
      manualRawCursorRef.current = true;
    }
    setMousePosition(chartStateRef.current, canvasChartRef.current, event);
    startChartDrag(chartStateRef.current);
  };

  const handleChartMouseUp = () => {
    if (!chartStateRef.current) {
      return;
    }

    endChartDrag(chartStateRef.current);
  };

  const handlePriceMouseDown = (event) => {
    if (!chartStateRef.current) {
      return;
    }

    if (isRawCandleView && selectedPattern?.raw_follow_latest) {
      manualRawViewportRef.current = true;
    }
    startPriceDrag(chartStateRef.current);
  };

  const handlePriceMouseUp = () => {
    if (!chartStateRef.current) {
      return;
    }

    endPriceDrag(chartStateRef.current);
  };

  return (
    <div className="candle_chart_container">
      <div className="candle_chart_wrapper">
        <div className="canvas">
          <canvas
            ref={canvasChartRef}
            onMouseDown={handleChartMouseDown}
            onMouseUp={handleChartMouseUp}
          />
        </div>

        <div className="canvas_dates">
          <canvas ref={canvasDatesRef} />
        </div>
      </div>

      <div className="canvas_prices">
        <div className="pricesb">
          <canvas
            ref={canvasPriceRef}
            onMouseDown={handlePriceMouseDown}
            onMouseUp={handlePriceMouseUp}
          />
        </div>
      </div>
    </div>
  );
};
