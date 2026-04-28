import React, { useEffect, useRef } from 'react';
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

const hasRenderablePattern = (chartData) =>
  Boolean(chartData?.candles?.length && chartData?.rust_patterns);

export const CandleChart = ({
  chartData,
  is_price_levels,
  is_retracement,
  is_abcd_pattern,
  is_reversal_focus,
  trend_line_toggles,
  focusMode = 'pattern',
  market,
  activeReversalFilter,
  set_hovered_candle,
}) => {
  const canvasDatesRef = useRef(null);
  const canvasPriceRef = useRef(null);
  const canvasChartRef = useRef(null);
  const hoveredCandleIndexRef = useRef(1);
  const chartStateRef = useRef(null);
  const selectedPattern = chartData?.rust_patterns ?? null;
  const hasCandles = Boolean(chartData?.candles?.length);
  const effectiveFocusMode = focusMode === 'prop' ? 'prop' : is_reversal_focus ? 'reversal' : 'pattern';

  useEffect(() => {
    if (!hasRenderablePattern(chartData)) {
      chartStateRef.current = null;
      return undefined;
    }

    const initializeChartState = () => {
      if (!canvasChartRef.current || !canvasPriceRef.current || !canvasDatesRef.current) {
        return;
      }

      canvasTools.reset_candle_canvas(canvasChartRef);
      canvasTools.reset_price_canvas(canvasPriceRef);
      canvasTools.reset_date_canvas(canvasDatesRef);

      chartStateRef.current = createChartState({
        canvasWidth: canvasChartRef.current.offsetWidth,
        canvasHeight: canvasChartRef.current.offsetHeight,
        candles: chartData.candles,
      });
      resize.reposition_candles(chartStateRef, chartData.rust_patterns, {
        focusMode: effectiveFocusMode,
        activeReversalFilter,
      });
    };

    initializeChartState();
    window.addEventListener('resize', initializeChartState);

    return () => {
      window.removeEventListener('resize', initializeChartState);
    };
  }, [activeReversalFilter, chartData, effectiveFocusMode]);

  useEffect(() => {
    if (!hasCandles || !selectedPattern || !chartStateRef.current) {
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

    const mouseLayer = new Mouse(chartStateRef);
    const chartLayer = new Chart(chartStateRef);
    const patternLayer = new ABCD(chartStateRef);
    const showPatternOverlay = is_abcd_pattern;
    const showRetracementOverlay = is_retracement;
    const showPriceLevelsOverlay = is_price_levels;

    let animationFrameId = null;

    const requestDraw = () => {
      if (!animationFrameId) {
        animationFrameId = requestAnimationFrame(draw);
      }
    };

    const draw = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx_price.clearRect(0, 0, cp.width, cp.height);
      ctx_date.clearRect(0, 0, canvas_date.width, canvas_date.height);

      chartLayer.candles(ctx, chartData.rust_patterns, {
        reversalFocusOnly: is_reversal_focus,
        activeReversalFilter,
      });
      chartLayer.prices(ctx_price, cp);
      chartLayer.dates(ctx_date, canvas_date);
      chartLayer.grid_X(ctx, canvas);
      chartLayer.grid_Y(ctx, canvas);

      if (
        trend_line_toggles?.threeMonth ||
        trend_line_toggles?.sixMonth ||
        trend_line_toggles?.twelveMonth
      ) {
        chartLayer.trend_lines(ctx, canvas, trend_line_toggles);
      }

      mouseLayer.mouse_Y(canvas, ctx);
      mouseLayer.mouse_X(canvas, ctx, hoveredCandleIndexRef, set_hovered_candle);
      mouseLayer.price_background(cp, ctx_price);
      mouseLayer.mouse_price(cp, ctx_price);
      mouseLayer.date_background(ctx_date, canvas_date);
      mouseLayer.mouse_date(canvas_date, ctx_date);

      if (focusMode === 'prop') {
        patternLayer.drawSetupOverlay(ctx, chartData.rust_patterns);
        patternLayer.prop_events(ctx, chartData.rust_patterns);
        patternLayer.target_candle(ctx, chartData.rust_patterns);
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

      if (showPriceLevelsOverlay) {
        patternLayer.price_levels(ctx_price, ctx, canvas, chartData.rust_patterns);
      }

      animationFrameId = null;
    };

    const handleMouseMove = (event) => {
      setMousePosition(chartStateRef.current, canvas, event);

      if (chartStateRef.current.mouse.isPressed) {
        const pixelsMovedX =
          chartStateRef.current.mouse.down.x - chartStateRef.current.mouse.pos.x;

        chartStateRef.current.viewport.xOrigin =
          chartStateRef.current.viewport.prevXOrigin + pixelsMovedX;
        resize.chart_Y_movement(chartStateRef);
      }

      requestDraw();
    };

    const handlePriceZoom = (event) => {
      event.preventDefault();

      const threshold = Math.floor(chartStateRef.current.price.startingPixelsPerGrid * 0.5);
      const expandThreshold = Math.floor(chartStateRef.current.price.startingPixelsPerGrid * 1.5);

      if (event.deltaY < 0) {
        resize.chart_zoom_in(chartStateRef, expandThreshold);
      } else if (event.deltaY > 0) {
        resize.chart_zoom_out(chartStateRef, threshold);
      }

      requestDraw();
    };

    const handleWidthZoom = (event) => {
      event.preventDefault();
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

    canvas.addEventListener('mousemove', handleMouseMove);
    canvas.addEventListener('wheel', handleWidthZoom, { passive: false });
    cp.addEventListener('wheel', handlePriceZoom, { passive: false });
    window.addEventListener('resize', handleWindowResize);

    draw();

    return () => {
      if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
      }

      canvas.removeEventListener('mousemove', handleMouseMove);
      canvas.removeEventListener('wheel', handleWidthZoom);
      cp.removeEventListener('wheel', handlePriceZoom);
      window.removeEventListener('resize', handleWindowResize);
    };
  }, [
    chartData,
    is_abcd_pattern,
    is_price_levels,
    is_retracement,
    is_reversal_focus,
    trend_line_toggles,
    focusMode,
    effectiveFocusMode,
    market,
    activeReversalFilter,
    set_hovered_candle,
  ]);

  const handleChartMouseDown = () => {
    if (!chartStateRef.current) {
      return;
    }

    startChartDrag(chartStateRef.current);
  };

  const handleChartMouseUp = () => {
    if (!chartStateRef.current) {
      return;
    }

    endChartDrag(chartStateRef.current);
  };

  const handlePriceMouseDown = () => {
    if (!chartStateRef.current) {
      return;
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
