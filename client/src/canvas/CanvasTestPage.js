import React, { useEffect, useState } from 'react';
import { CandleChart } from '../features/candle-chart/CandleChart';
import { getCandles } from '../services/patternApi';

const TEST_SYMBOL = 'HON6';
const TEST_ROOT_SYMBOL = 'HO';
const TEST_TIMEFRAME = '2m';

const buildRawCandleChartData = (candles) => ({
  candles,
  rust_patterns: {
    raw_candle_view: true,
    raw_visible_candles: 240,
    raw_focus_index: candles.length,
    raw_follow_latest: true,
    market: 'Bullish',
    symbol: TEST_SYMBOL,
    root_symbol: TEST_ROOT_SYMBOL,
    source_timeframe: TEST_TIMEFRAME,
  },
  snr_lines: [],
});

const CanvasTestPage = () => {
  const [chartData, setChartData] = useState(() => buildRawCandleChartData([]));
  const [loadState, setLoadState] = useState({ status: 'loading', message: 'Loading HON6 candles...' });

  useEffect(() => {
    let isMounted = true;

    const loadCandles = async () => {
      setLoadState({ status: 'loading', message: 'Loading HON6 candles...' });

      try {
        const candles = await getCandles(TEST_SYMBOL, {
          rootSymbol: TEST_ROOT_SYMBOL,
          sourceTimeframe: TEST_TIMEFRAME,
          limit: 1500,
        });

        if (!isMounted) {
          return;
        }

        setChartData(buildRawCandleChartData(candles));
        setLoadState({
          status: candles.length ? 'ready' : 'empty',
          message: candles.length ? `Loaded ${candles.length} HON6 candles.` : 'No HON6 candles returned.',
        });
      } catch (error) {
        if (!isMounted) {
          return;
        }

        setLoadState({
          status: 'error',
          message: error?.message || 'Failed to load HON6 candles.',
        });
      }
    };

    loadCandles();

    return () => {
      isMounted = false;
    };
  }, []);

  return (
    <div className="ctp" style={{ minHeight: '100vh', border: '5px solid white', boxSizing: 'border-box' }}>
        <div style={{ padding: '10px 14px', color: 'white', background: '#111827' }}>
          <strong>Canvas test:</strong> {loadState.message}
        </div>
        <CandleChart
          chartData={chartData}
          set_hovered_candle={() => {}}
          // is_price_levels={isPriceLevels}
          // is_retracement={isRetracement}
          // is_abcd_pattern={isAbcdPattern}
          // is_reversal_focus={isReversalFocus}
          // trend_line_toggles={trendLineToggles}
          // focusMode={focusMode}
          // propFocusScope={propFocusScope}
          // market={market}
          // activeReversalFilter={activeReversalFilter}
          // set_hovered_candle={handleHoveredCandleChange}
          // showCandles={showCandles}
          // presentationMode={presentationMode}
          // routeLogicHover={routeLogicHover}
          // onRawCandleViewportEdge={onRawCandleViewportEdge}
        />
    </div>
    // <CandleChart/>

  );
};

export default CanvasTestPage;
