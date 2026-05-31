import { useCallback, useEffect, useMemo, useState } from 'react';
import { CandleChart } from './CandleChart';
import PatternTable from '../../components/PatternTable';
import Section from '../../components/Section';

const formatDebugDate = (value) => {
  if (!value) {
    return '--';
  }

  if (typeof value === 'string') {
    const match = value.match(/^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2})/);
    if (match) {
      return `${match[1]} ${match[2]}`;
    }
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return String(value);
  }

  const year = parsed.getFullYear();
  const month = `${parsed.getMonth() + 1}`.padStart(2, '0');
  const day = `${parsed.getDate()}`.padStart(2, '0');
  const hour = `${parsed.getHours()}`.padStart(2, '0');
  const minute = `${parsed.getMinutes()}`.padStart(2, '0');
  return `${year}-${month}-${day} ${hour}:${minute}`;
};

const formatDebugPrice = (value) => {
  const numericValue = Number(value);

  if (!Number.isFinite(numericValue)) {
    return '--';
  }

  return numericValue.toFixed(2);
};

const formatDebugInteger = (value) => {
  const numericValue = Number(value);
  return Number.isFinite(numericValue) ? `${numericValue}` : '--';
};

const formatHoverPrice = (value) => {
  if (value === null || value === undefined || value === '') {
    return '--';
  }

  const numericValue = Number(value);

  if (!Number.isFinite(numericValue)) {
    return '--';
  }

  const absoluteValue = Math.abs(numericValue);
  if (absoluteValue < 10) {
    return numericValue.toFixed(4);
  }

  if (absoluteValue < 100) {
    return numericValue.toFixed(3);
  }

  return numericValue.toFixed(2);
};

const formatHoverVolume = (value) => {
  if (value === null || value === undefined || value === '') {
    return '--';
  }

  const numericValue = Number(value);
  return Number.isFinite(numericValue) ? numericValue.toLocaleString() : '--';
};

const splitDebugDateTime = (value) => {
  if (!value || value === '--') {
    return { date: '--', time: '--' };
  }

  const [date, time] = String(value).split(' ');
  return {
    date: date || '--',
    time: time || '--',
  };
};

const resolvePivotPrice = (pattern, pivot) => {
  const directValue = pattern?.[`${pivot}_price`];
  if (Number.isFinite(directValue)) {
    return directValue;
  }

  const market = pattern?.market;
  const bullishField = `${pivot}_low`;
  const bearishField = `${pivot}_high`;
  const fallbackField = market === 'Bearish' ? bearishField : bullishField;
  const fallbackValue = parseFloat(pattern?.[fallbackField]);
  return Number.isFinite(fallbackValue) ? fallbackValue : null;
};

const getTrendDisplayMeta = (value) => {
  if (value === true) {
    return { valueLabel: 'BULL', className: 'header_two header_two--trend-bullish' };
  }

  if (value === false) {
    return { valueLabel: 'BEAR', className: 'header_two header_two--trend-bearish' };
  }

  return { valueLabel: '--', className: 'header_two header_two--trend-neutral' };
};

const PatternDataStack = ({ rows, labelKey = 'key', ariaLabel }) => (
  <div className="pattern-data-stack" role="list" aria-label={ariaLabel}>
    {rows.map((row) => {
      const dateTime = splitDebugDateTime(row.date);

      return (
        <div className="pattern-data-row" role="listitem" key={row.key}>
          <div className="pattern-data-row__title">{row[labelKey]}</div>
          <div className="pattern-data-row__cells">
            <div className="pattern-data-row__cell">
              <span>Idx</span>
              <strong>{row.index}</strong>
            </div>
            <div className="pattern-data-row__cell">
              <span>Date</span>
              <strong>{dateTime.date}</strong>
            </div>
            <div className="pattern-data-row__cell">
              <span>Time</span>
              <strong>{dateTime.time}</strong>
            </div>
            <div className="pattern-data-row__cell">
              <span>Price</span>
              <strong>{row.price}</strong>
            </div>
          </div>
        </div>
      );
    })}
  </div>
);

const DetailMetricGrid = ({ items, className = '' }) => (
  <div className={['pattern-identity-metric-grid', className].filter(Boolean).join(' ')}>
    {items.map((item) => (
      <div className="pattern-identity-metric" key={item.label}>
        <span>{item.label}</span>
        <strong className={item.valueClassName} style={item.valueStyle}>
          {item.value}
        </strong>
      </div>
    ))}
  </div>
);

const CandleChartPanel = ({
  chartData,
  isSectionsExpanded,
  setSectionsExpanded,
  market,
  focusMode = 'pattern',
  activeReversalFilter = null,
  overlayTopOffset = 0,
  overlayTableProps,
  showCandles = true,
  presentationMode = 'chart',
  routeLogicHover = null,
  onHoveredCandleChange = null,
}) => {
  const isPropFocus = focusMode === 'prop';
  const [isAbcdPattern, setAbcdPattern] = useState(
    focusMode !== 'reversal' && focusMode !== 'prop'
  );
  const [isPriceLevels, setPriceLevels] = useState(focusMode !== 'reversal');
  const [isRetracement, setRetracement] = useState(
    focusMode !== 'reversal' && focusMode !== 'prop'
  );
  const [isReversalFocus, setReversalFocus] = useState(focusMode === 'reversal');
  const [isTrend3M, setTrend3M] = useState(false);
  const [isTrend6M, setTrend6M] = useState(false);
  const [isTrend12M, setTrend12M] = useState(false);
  const [isExpandedChart, setExpandedChart] = useState(false);
  const [propFocusScope, setPropFocusScope] = useState('trade');
  const [hoveredCandle, setHoveredCandle] = useState({
    date: null,
    high: null,
    close: null,
    open: null,
    low: null,
    volume: null,
    threeMonth: null,
    sixMonth: null,
    twelveMonth: null,
    color: 'white',
  });
  const handleHoveredCandleChange = useCallback((nextValueOrUpdater) => {
    setHoveredCandle(nextValueOrUpdater);
  }, []);
  const marketTone = market === 'Bearish' ? 'chart-market-bearish' : 'chart-market-bullish';
  const selectedPattern = chartData?.rust_patterns ?? null;
  const trendLineToggles = useMemo(
    () => ({
      threeMonth: isTrend3M,
      sixMonth: isTrend6M,
      twelveMonth: isTrend12M,
    }),
    [isTrend3M, isTrend6M, isTrend12M]
  );
  const hoveredPriceStats = [
    { label: 'O', value: formatHoverPrice(hoveredCandle.open), color: hoveredCandle.color },
    { label: 'H', value: formatHoverPrice(hoveredCandle.high), color: hoveredCandle.color },
    { label: 'C', value: formatHoverPrice(hoveredCandle.close), color: hoveredCandle.color },
    { label: 'L', value: formatHoverPrice(hoveredCandle.low), color: hoveredCandle.color },
    { label: 'V', value: formatHoverVolume(hoveredCandle.volume), color: hoveredCandle.color },
  ];
  const legBarStats = [
    { label: 'X Bars', value: selectedPattern?.x_length },
    { label: 'A Bars', value: selectedPattern?.a_length },
    { label: 'B Bars', value: selectedPattern?.b_length },
    { label: 'C Bars', value: selectedPattern?.c_length },
    {
      label: 'Total',
      value:
        [selectedPattern?.x_length, selectedPattern?.a_length, selectedPattern?.b_length, selectedPattern?.c_length]
          .filter((value) => Number.isFinite(value))
          .reduce((sum, value) => sum + value, 0) || null,
    },
  ];
  const hoveredTrendStats = [
    { label: '3M Trend', ...getTrendDisplayMeta(hoveredCandle.threeMonth) },
    { label: '6M Trend', ...getTrendDisplayMeta(hoveredCandle.sixMonth) },
    { label: '12M Trend', ...getTrendDisplayMeta(hoveredCandle.twelveMonth) },
    { label: 'D 3M', ...getTrendDisplayMeta(selectedPattern?.three_month) },
    { label: 'D 6M', ...getTrendDisplayMeta(selectedPattern?.six_month) },
    { label: 'D 12M', ...getTrendDisplayMeta(selectedPattern?.twelve_month) },
  ];
  const headerStats = [
    ...hoveredPriceStats.map((item) => ({
      label: item.label,
      value: item.value ?? '--',
      valueClassName: 'header_two',
      valueStyle: { color: item.color },
    })),
    ...legBarStats.map((item) => ({
      label: item.label,
      value: Number.isFinite(item.value) ? item.value : '--',
      valueClassName: 'header_two',
    })),
    ...hoveredTrendStats.map((item) => ({
      label: item.label,
      value: item.valueLabel,
      valueClassName: item.className,
    })),
  ];
  const pivotRows = ['x', 'a', 'b', 'c', 'd'].map((pivot) => {
    const upperPivot = pivot.toUpperCase();

    return {
      key: upperPivot,
      index: formatDebugInteger(selectedPattern?.[pivot]),
      date: formatDebugDate(selectedPattern?.[`${pivot}_date`]),
      price: formatDebugPrice(resolvePivotPrice(selectedPattern, pivot)),
    };
  });
  const isPropReversalFocus =
    selectedPattern?.prop_outcome_mode === 'reversal' || Boolean(selectedPattern?.reversal_detect_date);
  const dConfirmDate = selectedPattern?.d_confirm_date ?? null;
  const reversalDetectDate = selectedPattern?.reversal_detect_date ?? null;
  const entryDate = selectedPattern?.entry_date ?? null;
  const hasDistinctReversalEvent =
    isPropReversalFocus &&
    (selectedPattern?.reversal_detect ?? null) !== (selectedPattern?.d_confirm ?? null);
  const eventRows = selectedPattern?.xa_canvas_mode
    ? [
        {
          key: 'DC',
          label: 'D Confirm',
          index: formatDebugInteger(selectedPattern?.d_confirm),
          date: formatDebugDate(dConfirmDate),
          price: formatDebugPrice(selectedPattern?.xa_start_price),
        },
        {
          key: 'XA',
          label: 'XA Hit',
          index: formatDebugInteger(selectedPattern?.exit_date),
          date: formatDebugDate(selectedPattern?.xa_outcome_hit_date),
          price: formatDebugPrice(selectedPattern?.xa_outcome_price),
        },
      ]
    : isPropReversalFocus
      ? [
        {
          key: 'DC',
          label: 'D Confirm',
          index: formatDebugInteger(selectedPattern?.d_confirm),
          date: formatDebugDate(dConfirmDate),
          price: '--',
        },
        {
          key: 'RV',
          label: hasDistinctReversalEvent ? 'Reversal' : 'D + Reversal',
          index: formatDebugInteger(selectedPattern?.reversal_detect ?? selectedPattern?.d_confirm),
          date: formatDebugDate(reversalDetectDate ?? dConfirmDate),
          price: '--',
        },
        {
          key: 'EN',
          label: 'Entry',
          index: formatDebugInteger(selectedPattern?.entry),
          date: formatDebugDate(entryDate),
          price: formatDebugPrice(selectedPattern?.trade_enter_price),
        },
        {
          key: 'TG',
          label: 'Exit',
          index: formatDebugInteger(selectedPattern?.exit_date),
          date: formatDebugDate(selectedPattern?.target_date),
          price: formatDebugPrice(selectedPattern?.exit_price),
        },
      ]
    : [
        {
          key: 'DC',
          label: 'Confirm',
          index: formatDebugInteger(selectedPattern?.d_confirm),
          date: formatDebugDate(dConfirmDate),
          price: '--',
        },
        {
          key: 'EN',
          label: 'Entry',
          index: formatDebugInteger(selectedPattern?.entry),
          date: formatDebugDate(entryDate),
          price: formatDebugPrice(selectedPattern?.trade_enter_price),
        },
        {
          key: 'TG',
          label: 'Exit',
          index: formatDebugInteger(selectedPattern?.exit_date),
          date: formatDebugDate(selectedPattern?.target_date),
          price: formatDebugPrice(selectedPattern?.exit_price),
        },
      ];
  const identityFields = [
    { label: 'Pattern Group', value: selectedPattern?.pattern_group_id ?? '--', wide: true },
    { label: 'Pattern ID', value: selectedPattern?.pattern_id ?? '--', wide: true },
    { label: 'Strategy ID', value: selectedPattern?.prop_strategy_id ?? '--', wide: true },
    { label: 'Symbol', value: selectedPattern?.symbol ?? '--' },
    { label: 'Pattern', value: selectedPattern?.harmonic_type ?? '--' },
    { label: 'Market', value: selectedPattern?.market ?? '--' },
    { label: 'Loaded D', value: formatDebugDate(selectedPattern?.d_date) },
    { label: 'D Confirm', value: formatDebugDate(dConfirmDate) },
    ...(selectedPattern?.xa_canvas_mode
      ? [{ label: 'XA Hit', value: formatDebugDate(selectedPattern?.xa_outcome_hit_date) }]
      : [{ label: 'Entry', value: formatDebugDate(entryDate) }]),
    ...(isPropReversalFocus
      ? [{ label: 'Reversal Detect', value: formatDebugDate(reversalDetectDate ?? dConfirmDate) }]
      : []),
  ];
  const tradeLevelFields = selectedPattern?.xa_canvas_mode
    ? [
        { label: 'D', value: formatDebugPrice(selectedPattern?.xa_start_price), tone: 'entry' },
        { label: 'Continuation XA', value: formatDebugPrice(selectedPattern?.xa_continuation_limit_price), tone: 'stop' },
        { label: 'Reversal XA', value: formatDebugPrice(selectedPattern?.xa_reversal_limit_price), tone: 'target' },
      ]
      : [
        { label: 'Entry', value: formatDebugPrice(selectedPattern?.trade_enter_price), tone: 'entry' },
        { label: 'Stop', value: formatDebugPrice(selectedPattern?.trade_risk_exit_price), tone: 'stop' },
        { label: 'Target', value: formatDebugPrice(selectedPattern?.trade_reward_exit_price), tone: 'target' },
      ];
  const liveCandleMetrics = hoveredPriceStats.map((item) => ({
    label: item.label,
    value: item.value ?? '--',
    valueStyle: { color: item.color },
  }));
  const barMetrics = legBarStats.map((item) => ({
    label: item.label,
    value: Number.isFinite(item.value) ? item.value : '--',
  }));
  const trendMetrics = hoveredTrendStats.map((item) => ({
    label: item.label,
    value: item.valueLabel,
    valueClassName: item.className,
  }));
  const patternIdentityPanel = selectedPattern ? (
    <div className="pattern-identity-card">
      <div className="pattern-identity-header">
        <div>
          <div className="pattern-identity-kicker">Trade Details</div>
          <div className="pattern-identity-title">
            {selectedPattern?.symbol ?? '--'} / {selectedPattern?.harmonic_type ?? 'Pattern'}
          </div>
        </div>
        <div className="pattern-identity-pill-row">
          <span>{selectedPattern?.market ?? '--'}</span>
          <span>{selectedPattern?.reversal_type ?? 'None'}</span>
        </div>
        {isPropFocus ? (
          <div className="pattern-focus-toggle" aria-label="Canvas focus mode">
            <button
              type="button"
              className={propFocusScope === 'trade' ? 'pattern-focus-toggle__button pattern-focus-toggle__button--active' : 'pattern-focus-toggle__button'}
              onClick={() => setPropFocusScope('trade')}
            >
              Trade
            </button>
            <button
              type="button"
              className={propFocusScope === 'pattern' ? 'pattern-focus-toggle__button pattern-focus-toggle__button--active' : 'pattern-focus-toggle__button'}
              onClick={() => setPropFocusScope('pattern')}
            >
              Pattern
            </button>
          </div>
        ) : null}
      </div>

      <div className="pattern-trade-levels" aria-label="Trade levels">
        {tradeLevelFields.map((field) => (
          <div className={`pattern-trade-level pattern-trade-level--${field.tone}`} key={field.label}>
            <span>{field.label}</span>
            <strong>{field.value}</strong>
          </div>
        ))}
      </div>

      <div className="pattern-identity-section">
        <div className="pattern-identity-section-title">Hovered candle</div>
        <DetailMetricGrid items={liveCandleMetrics} />
      </div>

      <div className="pattern-identity-section">
        <div className="pattern-identity-section-title">Structure</div>
        <DetailMetricGrid items={barMetrics} />
      </div>

      <div className="pattern-identity-section">
        <div className="pattern-identity-section-title">Trend state</div>
        <DetailMetricGrid items={trendMetrics} className="pattern-identity-metric-grid--trend" />
      </div>

      <div className="pattern-identity-section">
        <div className="pattern-identity-section-title">Setup</div>
        <div className="pattern-identity-grid">
          {identityFields.map((field) => (
            <div
              key={field.label}
              className={field.wide ? 'pattern-identity-field pattern-identity-field--wide' : 'pattern-identity-field'}
            >
              <div className="pattern-identity-label">{field.label}</div>
              <div className="pattern-identity-value">{field.value}</div>
            </div>
          ))}
        </div>
      </div>

      <div className="pattern-identity-section">
        <div className="pattern-identity-section-title">Timeline</div>
        <PatternDataStack rows={eventRows} labelKey="label" ariaLabel="Pattern checkpoints" />
      </div>

      <div className="pattern-identity-section">
        <div className="pattern-identity-section-title">XABCD pivots</div>
        <PatternDataStack rows={pivotRows} ariaLabel="Pattern pivot points" />
      </div>
    </div>
  ) : null;

  useEffect(() => {
    onHoveredCandleChange?.(hoveredCandle);
  }, [hoveredCandle, onHoveredCandleChange]);

  useEffect(() => {
    if (focusMode === 'reversal') {
      setAbcdPattern(false);
      setPriceLevels(false);
      setRetracement(false);
      setReversalFocus(true);
      return;
    }

    if (focusMode === 'prop') {
      setPropFocusScope('trade');
      setAbcdPattern(false);
      setPriceLevels(true);
      setRetracement(false);
      setReversalFocus(false);
    }
  }, [focusMode]);
  const controlItems = [
    ...(isPropFocus
      ? [
          {
            active: isPriceLevels,
            onClick: () => setPriceLevels(!isPriceLevels),
            icon: '/images/prices.png',
            label: 'Rays',
          },
        ]
      : []),
    ...(!isPropFocus
      ? [
          {
            active: isAbcdPattern,
            onClick: () => setAbcdPattern(!isAbcdPattern),
            icon: '/images/dropdown.png',
            label: 'Pattern',
          },
          {
            active: isPriceLevels,
            onClick: () => setPriceLevels(!isPriceLevels),
            icon: '/images/prices.png',
            label: 'Levels',
          },
          {
            active: isRetracement,
            onClick: () => setRetracement(!isRetracement),
            icon: '/images/retracement.png',
            label: 'Retrace',
          },
          {
            active: isReversalFocus,
            onClick: () => setReversalFocus(!isReversalFocus),
            icon: '/images/abcd.png',
            label: 'Reversal',
          },
        ]
      : []),
    {
      active: isTrend3M,
      onClick: () => setTrend3M(!isTrend3M),
      icon: '/images/prices.png',
      label: '3M',
    },
    {
      active: isTrend6M,
      onClick: () => setTrend6M(!isTrend6M),
      icon: '/images/prices.png',
      label: '6M',
    },
    {
      active: isTrend12M,
      onClick: () => setTrend12M(!isTrend12M),
      icon: '/images/prices.png',
      label: '12M',
    },
    {
      active: isSectionsExpanded,
      onClick: () => setSectionsExpanded(!isSectionsExpanded),
      icon: '/images/abcd.png',
      label: 'Panels',
    },
  ];

  const renderChartShell = (isOverlay = false) => (
    <div
      className={[
        'chart-panel-shell',
        isOverlay ? 'chart-panel-shell--overlay' : '',
        patternIdentityPanel ? 'chart-panel-shell--with-detail' : '',
      ].filter(Boolean).join(' ')}
    >
      <div className="chart-header-wrapper">
        <div className="chart-panel-topline">
          <div className="chart-panel-copy">
            <div className="chart-window-dots" aria-hidden="true">
              <span className="chart-window-dot chart-window-dot--warm" />
              <span className="chart-window-dot chart-window-dot--neutral" />
              <span className="chart-window-dot chart-window-dot--cool" />
            </div>
            <div className="chart-kicker">XABCD Terminal</div>
            <div className="chart-title-row">
              <h3 className="chart-panel-title">
                {isPropFocus ? 'Trade Workspace' : 'Pattern Workspace'}
              </h3>
              <div className={`chart-market-pill ${marketTone}`}>{market}</div>
            </div>
          </div>

          <div className="header-buttons-wrapper">
            {controlItems.map((item) => (
              <button
                key={item.label}
                type="button"
                className={item.active ? 'chart-tool-button chart-tool-button--active' : 'chart-tool-button'}
                onClick={item.onClick}
              >
                <span className="chart-tool-icon-wrap">
                  <img className="abcd_img" src={item.icon} alt="" />
                </span>
                <span className="chart-control-copy">
                  <span className="chart-control-label">{item.label}</span>
                  <span className="chart-control-state">{item.active ? 'On' : 'Off'}</span>
                </span>
                <span className="chart-tool-indicator" aria-hidden="true" />
              </button>
            ))}

            {!isOverlay && (
              <button className="chart-action-button" onClick={() => setExpandedChart(true)}>
                Expand View
              </button>
            )}

            {isOverlay && (
              <button className="chart-action-button" onClick={() => setExpandedChart(false)}>
                Close View
              </button>
            )}
          </div>
        </div>

        {!patternIdentityPanel ? (
          <div className="header-bar">
            {headerStats.map((item) => (
              <div className="header_slot" key={item.label}>
                <div className="header_one">{item.label}</div>
                <div className={item.valueClassName} style={item.valueStyle}>
                  {item.value}
                </div>
              </div>
            ))}
          </div>
        ) : null}
      </div>

      {chartData.candles.length > 0 && (
        <div className={patternIdentityPanel ? 'chart-workspace chart-workspace--with-detail' : 'chart-workspace'}>
          {patternIdentityPanel ? <aside className="chart-detail-rail">{patternIdentityPanel}</aside> : null}

          <div className="chart-canvas-stage">
            <CandleChart
              chartData={chartData}
              is_price_levels={isPriceLevels}
              is_retracement={isRetracement}
              is_abcd_pattern={isAbcdPattern}
              is_reversal_focus={isReversalFocus}
              trend_line_toggles={trendLineToggles}
              focusMode={focusMode}
              propFocusScope={propFocusScope}
              market={market}
              activeReversalFilter={activeReversalFilter}
              set_hovered_candle={handleHoveredCandleChange}
              showCandles={showCandles}
              presentationMode={presentationMode}
              routeLogicHover={routeLogicHover}
            />
          </div>
        </div>
      )}
    </div>
  );

  return (
    <>
      <div className="charts_container">
        <div className="margin-">{renderChartShell()}</div>
      </div>

      {isExpandedChart && (
        <div
          className="chart-overlay"
          style={{ top: `${overlayTopOffset}px` }}
          onClick={() => setExpandedChart(false)}
        >
          <div className="chart-overlay-card" onClick={(event) => event.stopPropagation()}>
            <div className="chart-overlay-layout">
              <div className="chart-overlay-chart">{renderChartShell(true)}</div>
              <div className="chart-overlay-table">
                {overlayTableProps ? (
                  <Section>
                    <PatternTable {...overlayTableProps} />
                  </Section>
                ) : null}
              </div>
            </div>
          </div>
        </div>
      )}
    </>
  );
};

export default CandleChartPanel;
