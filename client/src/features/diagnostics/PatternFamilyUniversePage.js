import React, { useEffect, useMemo, useState } from 'react';
import CandleChartPanel from '../candle-chart/CandleChartPanel';
import {
  fetchPatternDetail,
  fetchPatternFamilies,
  fetchPhase1FamilyPatterns,
  fetchPhase1Leaderboard,
  fetchPhase1PatternRouteReplay,
  fetchPhase1RouteReplay,
  fetchPhase1Results,
  fetchPhase1Supply,
  getCandles,
  getSupportResistanceLines,
} from '../../services/patternApi';
import { formatPattern } from '../../utils/patternFormatting';

const formatNumber = (value) =>
  Number.isFinite(Number(value)) ? Number(value).toLocaleString() : '0';

const formatDate = (value) => {
  if (!value) return 'N/A';
  const text = String(value);
  return text.length > 10 ? text.slice(0, 10) : text;
};

const formatDecimal = (value, digits = 2) =>
  Number.isFinite(Number(value)) ? Number(value).toFixed(digits) : '0.00';

const formatMoney = (value) =>
  Number.isFinite(Number(value))
    ? Number(value).toLocaleString('en-US', {
        style: 'currency',
        currency: 'USD',
        maximumFractionDigits: 0,
      })
    : 'N/A';

const formatRouteMode = (value = '') =>
  String(value || 'N/A')
    .split('_')
    .filter(Boolean)
    .map((part) => (part.length <= 2 ? part.toUpperCase() : `${part[0].toUpperCase()}${part.slice(1)}`))
    .join(' ');

const ROUTE_ENTRY_COPY = {
  next_open: 'enters on the next open after the pattern completes',
  d_break: 'scans forward after D; the first candle whose wick trades through the D high/low becomes the entry candle, filled at the D level',
  d_close_confirm: 'waits for a candle close beyond D; entry fills at that confirming close',
  c_break: 'scans forward after D; the first candle whose wick trades through the C high/low becomes the entry candle',
  b_break: 'scans forward after D; the first candle whose wick trades through the B high/low becomes the entry candle',
  confirm_p1_body_signal_p2_open:
    'uses Confirmation +1 as the body-break signal, then enters at Confirmation +2 open if that open also clears the body rule',
};

const ROUTE_STOP_COPY = {
  d_extreme: 'uses the D extreme as the stop-loss level',
  c_extreme: 'uses the C extreme as the stop-loss level',
  x_extreme: 'uses the X extreme as the stop-loss level',
  cd_025: 'sets the stop at 0.25 of the CD leg',
  cd_050: 'sets the stop at 0.50 of the CD leg',
  cd_075: 'sets the stop at 0.75 of the CD leg',
  cd_100: 'sets the stop at 1.00 of the CD leg',
  cd_150: 'sets the stop at 1.50 of the CD leg',
};

const ROUTE_ENTRY_ACTION = {
  next_open: 'NEXT OPEN',
  d_break: 'D LEVEL FILL',
  d_close_confirm: 'D CLOSE FILL',
  c_break: 'C LEVEL FILL',
  b_break: 'B LEVEL FILL',
  confirm_p1_body_signal_p2_open: 'CON+2 OPEN BODY BREAK',
};

const ROUTE_STOP_ACTION = {
  d_extreme: 'D STOP',
  c_extreme: 'C STOP',
  x_extreme: 'X STOP',
  cd_025: '0.25 CD STOP',
  cd_050: '0.50 CD STOP',
  cd_075: '0.75 CD STOP',
  cd_100: '1.00 CD STOP',
  cd_150: '1.50 CD STOP',
};

const getRouteDirectionMeta = (market = '') => {
  const isBearish = String(market).toLowerCase() === 'bearish';
  return {
    isBearish,
    side: isBearish ? 'SHORT' : 'LONG',
    breakSide: isBearish ? 'LOW' : 'HIGH',
    stopSide: isBearish ? 'HIGH' : 'LOW',
    confirmSide: isBearish ? 'below' : 'above',
  };
};

const getTradeSide = (trade = null) => {
  if (!trade) return null;

  const storedDirection = String(trade.trade_direction || '').trim().toLowerCase();
  if (storedDirection === 'long') return 'LONG';
  if (storedDirection === 'short') return 'SHORT';

  const entryPrice = Number(trade.trade_enter_price);
  const targetPrice = Number(trade.trade_reward_exit_price);
  if (!Number.isFinite(entryPrice) || !Number.isFinite(targetPrice) || targetPrice === entryPrice) {
    return null;
  }

  return targetPrice > entryPrice ? 'LONG' : 'SHORT';
};

const formatTradeDirection = (trade = null) => {
  const side = getTradeSide(trade);
  if (side === 'LONG') return '↑ LONG';
  if (side === 'SHORT') return '↓ SHORT';
  return 'N/A';
};

const getDirectionalEntryAction = (entryMode = '', market = '', trade = null) => {
  const direction = getRouteDirectionMeta(market);
  const tradeSide = getTradeSide(trade);

  if (entryMode === 'post_confirm_decision') {
    if (tradeSide) {
      return `${tradeSide}: DECISION OPEN`;
    }

    return direction.isBearish
      ? 'BEARISH SETUP: SHORT OR REVERSAL LONG'
      : 'BULLISH SETUP: LONG OR FAILURE SHORT';
  }

  if (entryMode === 'confirm_p1_body_signal_p2_open') {
    return direction.isBearish
      ? 'SHORT: CON+2 OPEN BELOW BODY'
      : 'LONG: CON+2 OPEN ABOVE BODY';
  }

  if (entryMode === 'next_open') {
    return `${direction.side}: NEXT OPEN`;
  }

  if (entryMode === 'd_break') {
    return `${direction.side}: TOUCH D ${direction.breakSide}`;
  }

  if (entryMode === 'd_close_confirm') {
    return `${direction.side}: CLOSE ${direction.confirmSide.toUpperCase()} D`;
  }

  if (entryMode === 'c_break') {
    return `${direction.side}: TOUCH C ${direction.breakSide}`;
  }

  if (entryMode === 'b_break') {
    return `${direction.side}: TOUCH B ${direction.breakSide}`;
  }

  return ROUTE_ENTRY_ACTION[entryMode] ?? formatRouteMode(entryMode).toUpperCase();
};

const getDirectionalEntryCopy = (entryMode = '', market = '') => {
  const direction = getRouteDirectionMeta(market);

  if (entryMode === 'post_confirm_decision') {
    return direction.isBearish
      ? 'bearish setup: entry is the decision candle open; short if that candle closes below the confirmation open, long only if it closes back above the D low'
      : 'bullish setup: entry is the decision candle open; long if that candle closes above the confirmation open, short only if it closes back below the D high';
  }

  if (entryMode === 'confirm_p1_body_signal_p2_open') {
    return direction.isBearish
      ? 'bearish setup: Confirmation +1 must close below Confirmation body low; entry is Confirmation +2 open only if that open is below Confirmation body low and below Confirmation +1 body high'
      : 'bullish setup: Confirmation +1 must close above Confirmation body high; entry is Confirmation +2 open only if that open is above Confirmation body high and above Confirmation +1 body low';
  }

  if (entryMode === 'd_break') {
    return direction.isBearish
      ? 'bearish route: wick counts; entry triggers when any post-D candle trades at or below the D low, then fills at the D low'
      : 'bullish route: wick counts; entry triggers when any post-D candle trades at or above the D high, then fills at the D high';
  }

  if (entryMode === 'c_break') {
    return direction.isBearish
      ? 'bearish route: wick counts; entry triggers when any post-D candle trades at or below the C low'
      : 'bullish route: wick counts; entry triggers when any post-D candle trades at or above the C high';
  }

  if (entryMode === 'b_break') {
    return direction.isBearish
      ? 'bearish route: wick counts; entry triggers when any post-D candle trades at or below the B low'
      : 'bullish route: wick counts; entry triggers when any post-D candle trades at or above the B high';
  }

  if (entryMode === 'd_close_confirm') {
    return direction.isBearish
      ? 'bearish route: waits for a candle close below D, then fills at that close'
      : 'bullish route: waits for a candle close above D, then fills at that close';
  }

  return ROUTE_ENTRY_COPY[entryMode] ?? `uses ${formatRouteMode(entryMode)} for entry`;
};

const getDirectionalStopAction = (stopMode = '', market = '') => {
  const direction = getRouteDirectionMeta(market);

  if (stopMode === 'x_extreme') {
    return `X ${direction.stopSide} STOP`;
  }

  if (stopMode === 'd_extreme') {
    return `D ${direction.stopSide} STOP`;
  }

  if (stopMode === 'c_extreme') {
    return `C ${direction.stopSide} STOP`;
  }

  return ROUTE_STOP_ACTION[stopMode] ?? formatRouteMode(stopMode).toUpperCase();
};

const getFamilyText = (family) =>
  [
    family.family_key,
    family.harmonic_type,
    family.bin,
    family.size_bucket,
    family.time_bin,
    family.x_strictness,
  ]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();

const uniqueValues = (rows, key) =>
  [...new Set(rows.map((row) => row[key]).filter(Boolean))].sort((left, right) =>
    String(left).localeCompare(String(right))
  );

const getYear = (value) => {
  if (!value) return null;
  const year = Number.parseInt(String(value).slice(0, 4), 10);
  return Number.isFinite(year) ? year : null;
};

const getYearOptions = (rows) => {
  const years = new Set();

  rows.forEach((row) => {
    const firstYear = getYear(row.first_d_date);
    const lastYear = getYear(row.last_d_date);
    if (!firstYear || !lastYear) return;

    for (let year = firstYear; year <= lastYear; year += 1) {
      years.add(year);
    }
  });

  return [...years].sort((left, right) => right - left);
};

const SOURCE_OPTIONS = [
  { value: 'futures', label: 'Futures' },
  { value: 'daily', label: 'Daily' },
  { value: 'all', label: 'All' },
];

const SIM_ACCOUNT_RULES = {
  '25K': {
    startingBalance: 25000,
    profitTarget: 1500,
    maxDrawdown: 1000,
    eodDailyLossLimit: 500,
  },
  '50K': {
    startingBalance: 50000,
    profitTarget: 3000,
    maxDrawdown: 2000,
    eodDailyLossLimit: 1000,
  },
  '100K': {
    startingBalance: 100000,
    profitTarget: 6000,
    maxDrawdown: 3000,
    eodDailyLossLimit: 1500,
  },
  '150K': {
    startingBalance: 150000,
    profitTarget: 9000,
    maxDrawdown: 4000,
    eodDailyLossLimit: 2000,
  },
};

const getRouteKey = (route) => `${route.run_id}-${route.route_id}`;
const getFamilyPatternKey = (pattern = {}) =>
  [
    pattern.pattern_id ?? '',
    pattern.pattern_group_id ?? '',
    pattern.symbol ?? '',
    pattern.d_date ?? '',
  ].join('|');
const getRouteTradeKey = (trade = {}) =>
  [
    trade.test_index ?? 1,
    trade.trade_index ?? 0,
    trade.pattern_id ?? trade.pattern_group_id ?? '',
    trade.entry_date ?? '',
  ].join('|');

const patternMatchesTrade = (pattern = {}, trade = {}) => {
  const samePatternId =
    pattern.pattern_id &&
    trade.pattern_id &&
    pattern.pattern_id === trade.pattern_id;
  const sameGroupId =
    pattern.pattern_group_id &&
    trade.pattern_group_id &&
    pattern.pattern_group_id === trade.pattern_group_id;
  return Boolean(samePatternId || sameGroupId);
};

const DATE_TIME_TEXT_PATTERN = /^(\d{4}-\d{2}-\d{2})(?:[T\s](\d{2}:\d{2}(?::\d{2})?))?/;
const padDatePart = (value) => String(value).padStart(2, '0');

const formatDateTimeForServer = (value) => {
  if (!value) return null;

  if (typeof value === 'string') {
    const match = value.match(DATE_TIME_TEXT_PATTERN);
    if (match) {
      const time = match[2] ? (match[2].length === 5 ? `${match[2]}:00` : match[2]) : '00:00:00';
      return `${match[1]} ${time}`;
    }
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return null;
  }

  return [
    parsed.getFullYear(),
    padDatePart(parsed.getMonth() + 1),
    padDatePart(parsed.getDate()),
  ].join('-') + ` ${[
    padDatePart(parsed.getHours()),
    padDatePart(parsed.getMinutes()),
    padDatePart(parsed.getSeconds()),
  ].join(':')}`;
};

const formatCandleDateForChart = (value) => {
  if (!value) return null;

  if (typeof value === 'string') {
    const match = value.match(DATE_TIME_TEXT_PATTERN);
    if (match) {
      const time = match[2] ? (match[2].length === 5 ? `${match[2]}:00` : match[2]) : '00:00:00';
      return `${match[1]} ${time}`;
    }
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return formatDateTimeForServer(parsed);
};

const buildPatternCandleWindow = (pattern = {}) => {
  const dateValues = [
    pattern.x_date,
    pattern.a_date,
    pattern.b_date,
    pattern.c_date,
    pattern.d_date,
    pattern.d_confirm_date,
    pattern.entry_date,
    pattern.target_date,
    pattern.trade_date,
  ]
    .map((value) => {
      const parsed = new Date(value);
      return Number.isNaN(parsed.getTime()) ? null : parsed.getTime();
    })
    .filter((value) => Number.isFinite(value));

  if (!dateValues.length) {
    return {};
  }

  const paddingMs = 24 * 60 * 60 * 1000;
  const exitDate = pattern.target_date ?? pattern.trade_date ?? null;
  const parsedExitDate = exitDate ? new Date(exitDate) : null;
  const exitTime =
    parsedExitDate && !Number.isNaN(parsedExitDate.getTime())
      ? parsedExitDate.getTime()
      : null;

  return {
    startDate: formatDateTimeForServer(new Date(Math.min(...dateValues) - paddingMs)),
    endDate:
      exitTime !== null
        ? formatDateTimeForServer(new Date(exitTime))
        : formatDateTimeForServer(new Date(Math.max(...dateValues) + paddingMs)),
  };
};

const normalizeCandles = (candles = []) =>
  candles
    .slice()
    .sort((left, right) => new Date(right.candle_date) - new Date(left.candle_date))
    .map((item) => ({
      ...item,
      candle_date: formatCandleDateForChart(item.candle_date),
    }));

const getDateTimeForCompare = (value) => {
  const formattedValue = formatDateTimeForServer(value);
  if (!formattedValue) {
    return null;
  }

  const parsed = new Date(formattedValue.replace(' ', 'T'));
  return Number.isNaN(parsed.getTime()) ? null : parsed.getTime();
};

const clipCandlesAfterTradeExit = (candles = [], pattern = {}) => {
  const exitTime = getDateTimeForCompare(pattern.target_date ?? pattern.trade_date);
  if (exitTime === null) {
    return candles;
  }

  return candles.filter((candle) => {
    const candleTime = getDateTimeForCompare(candle.candle_date ?? candle.date);
    return candleTime === null || candleTime <= exitTime;
  });
};

const mergeRouteTradeIntoPattern = (pattern = {}, trade = null) => {
  if (!trade) {
    return pattern;
  }

  const exitPrice =
    trade.exit_price ??
    trade.trade_exit_price ??
    (Number(trade.trade_result) === 2
      ? trade.trade_risk_exit_price
      : trade.trade_reward_exit_price);

  return {
    ...pattern,
    prop_outcome_mode: pattern?.prop_outcome_mode ?? 'phase1-family',
    entry_date: trade.entry_date ?? pattern.entry_date,
    target_date: trade.target_date ?? pattern.target_date,
    trade_enter_price: trade.trade_enter_price ?? pattern.trade_enter_price,
    trade_risk_exit_price: trade.trade_risk_exit_price ?? pattern.trade_risk_exit_price,
    trade_reward_exit_price: trade.trade_reward_exit_price ?? pattern.trade_reward_exit_price,
    trade_current_price: exitPrice ?? pattern.trade_current_price,
    target_close: exitPrice ?? pattern.target_close,
    trade_result: trade.trade_result ?? pattern.trade_result,
    result_r: trade.result_r ?? pattern.result_r,
    risk_points: trade.risk_points ?? pattern.risk_points,
  };
};

const SelectedSummaryRow = ({
  actionDisabled = false,
  actionLabel,
  emptyText,
  index,
  isLoading = false,
  label,
  loadingText,
  metrics = [],
  onAction,
  status,
  subtitle,
  title,
  tone = 'neutral',
}) => (
  <section className={`pattern-family-selected-card pattern-family-selected-card--${tone}`}>
    <header className="pattern-family-selected-head">
      <span className="pattern-family-selected-index">{String(index).padStart(2, '0')}</span>
      <strong>{label}</strong>
    </header>
    <div className="pattern-family-selected-data">
      <div className="pattern-family-selected-data-head">
        <span>Data</span>
        {status ? <strong title={status}>{status}</strong> : null}
      </div>
      <div className="pattern-family-selected-metrics">
        {isLoading ? (
          <div className="pattern-family-selected-empty">{loadingText}</div>
        ) : metrics.length ? (
          metrics.map((item) => (
            <div
              className={[
                'pattern-family-selected-metric',
                item.wide ? 'pattern-family-selected-metric--wide' : '',
                item.tone ? `pattern-family-selected-metric--${item.tone}` : '',
              ].filter(Boolean).join(' ')}
              key={item.label}
            >
              <span>{item.label}</span>
              <strong title={item.value}>{item.value}</strong>
            </div>
          ))
        ) : (
          <div className="pattern-family-selected-empty">{emptyText}</div>
        )}
      </div>
    </div>
    <div className="pattern-family-selected-action-wrap">
      <button type="button" onClick={onAction} disabled={actionDisabled}>
        {actionLabel}
      </button>
    </div>
  </section>
);

const PatternFamilyUniversePage = ({ initialFamilyKey = null } = {}) => {
  const [families, setFamilies] = useState([]);
  const [yearOptions, setYearOptions] = useState([]);
  const [isLoading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [search, setSearch] = useState('');
  const [harmonicType, setHarmonicType] = useState('All');
  const [sourceScope, setSourceScope] = useState('futures');
  const [yearFilter, setYearFilter] = useState('All');
  const [minSetups, setMinSetups] = useState('1');
  const [selectedFamilyKey, setSelectedFamilyKey] = useState(null);
  const [phase1Results, setPhase1Results] = useState([]);
  const [isPhase1Loading, setPhase1Loading] = useState(false);
  const [phase1Error, setPhase1Error] = useState('');
  const [selectedRouteKey, setSelectedRouteKey] = useState(null);
  const [familyPatterns, setFamilyPatterns] = useState([]);
  const [isFamilyPatternsLoading, setFamilyPatternsLoading] = useState(false);
  const [familyPatternsError, setFamilyPatternsError] = useState('');
  const [selectedFamilyPatternKey, setSelectedFamilyPatternKey] = useState(null);
  const [routeTrades, setRouteTrades] = useState([]);
  const [isRouteTradesLoading, setRouteTradesLoading] = useState(false);
  const [routeTradesError, setRouteTradesError] = useState('');
  const [routeFamilyRows, setRouteFamilyRows] = useState([]);
  const [isRouteFamiliesLoading, setRouteFamiliesLoading] = useState(false);
  const [routeFamiliesError, setRouteFamiliesError] = useState('');
  const [supplyData, setSupplyData] = useState({ symbols: [], families: [] });
  const [isSupplyLoading, setSupplyLoading] = useState(false);
  const [supplyError, setSupplyError] = useState('');
  const [selectedRouteTradeKey, setSelectedRouteTradeKey] = useState(null);
  const [selectedPatternRouteTrade, setSelectedPatternRouteTrade] = useState(null);
  const [isPatternRouteTradeLoading, setPatternRouteTradeLoading] = useState(false);
  const [patternRouteTradeError, setPatternRouteTradeError] = useState('');
  const [canvasChartData, setCanvasChartData] = useState({ candles: [], rust_patterns: null });
  const [canvasPattern, setCanvasPattern] = useState(null);
  const [isCanvasLoading, setCanvasLoading] = useState(false);
  const [canvasError, setCanvasError] = useState('');
  const [isCanvasExpanded, setCanvasExpanded] = useState(false);
  const [showCanvasCandles, setShowCanvasCandles] = useState(false);
  const [inspectorDetailMode, setInspectorDetailMode] = useState('trade');
  const [routeLogicHover, setRouteLogicHover] = useState(null);
  const simAccountSize = '50K';
  const simContracts = '1';
  const simTestsToChain = '1';
  const simDrawdownModel = 'intraday';
  const simOneTradeAtATime = false;
  const [browsePanel, setBrowsePanel] = useState(null);
  const [testOverviewTab, setTestOverviewTab] = useState('overview');

  useEffect(() => {
    let isCancelled = false;

    const loadFamilies = async () => {
      try {
        setLoading(true);
        setError('');
        const rows = await fetchPatternFamilies({
          limit: 5000,
          minSetupCount: 1,
          year: yearFilter === 'All' ? null : yearFilter,
          sourceScope,
        });
        if (!isCancelled) {
          setFamilies(rows);
          if (yearFilter === 'All') {
            setYearOptions(getYearOptions(rows));
          }
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setError('Could not load pattern families.');
        }
      } finally {
        if (!isCancelled) {
          setLoading(false);
        }
      }
    };

    void loadFamilies();

    return () => {
      isCancelled = true;
    };
  }, [sourceScope, yearFilter]);

  const harmonicOptions = useMemo(
    () => ['All', ...uniqueValues(families, 'harmonic_type')],
    [families]
  );

  const visibleFamilies = useMemo(() => {
    const query = search.trim().toLowerCase();
    const minCount = Number.parseInt(minSetups, 10);

    return families.filter((family) => {
      if (harmonicType !== 'All' && family.harmonic_type !== harmonicType) return false;
      if (Number.isFinite(minCount) && family.setup_count < minCount) return false;
      if (query && !getFamilyText(family).includes(query)) return false;
      return true;
    });
  }, [families, harmonicType, minSetups, search]);

  const visibleSetupCount = useMemo(
    () => visibleFamilies.reduce((sum, family) => sum + Number(family.setup_count || 0), 0),
    [visibleFamilies]
  );
  const largestFamily = visibleFamilies[0]?.setup_count ?? 0;
  const selectedFamily = useMemo(
    () => visibleFamilies.find((family) => family.family_key === selectedFamilyKey) ?? null,
    [selectedFamilyKey, visibleFamilies]
  );
  const selectedRoute = useMemo(
    () => phase1Results.find((result) => getRouteKey(result) === selectedRouteKey) ?? null,
    [phase1Results, selectedRouteKey]
  );
  const selectedFamilyPattern = useMemo(
    () => familyPatterns.find((pattern) => getFamilyPatternKey(pattern) === selectedFamilyPatternKey) ?? null,
    [familyPatterns, selectedFamilyPatternKey]
  );
  const selectedRouteTrade = useMemo(() => {
    const routeTrade = routeTrades.find((trade) => getRouteTradeKey(trade) === selectedRouteTradeKey) ?? null;
    if (routeTrade) {
      return routeTrade;
    }

    return selectedPatternRouteTrade &&
      getRouteTradeKey(selectedPatternRouteTrade) === selectedRouteTradeKey
        ? selectedPatternRouteTrade
        : null;
  }, [routeTrades, selectedPatternRouteTrade, selectedRouteTradeKey]);
  const selectedRouteTradeFamilyPattern = useMemo(() => {
    if (!selectedRouteTrade) {
      return null;
    }

    return familyPatterns.find((pattern) => patternMatchesTrade(pattern, selectedRouteTrade)) ?? null;
  }, [familyPatterns, selectedRouteTrade]);
  const selectedPatternTrade = useMemo(() => {
    if (!selectedFamilyPattern) {
      return null;
    }

    const routeTrade = routeTrades.find((trade) => patternMatchesTrade(selectedFamilyPattern, trade)) ?? null;
    if (routeTrade) {
      return routeTrade;
    }

    if (!selectedPatternRouteTrade) {
      return null;
    }

    return patternMatchesTrade(selectedFamilyPattern, selectedPatternRouteTrade)
      ? selectedPatternRouteTrade
      : null;
  }, [routeTrades, selectedFamilyPattern, selectedPatternRouteTrade]);
  const selectedRouteTradeMatchesSelectedPattern =
    Boolean(selectedRouteTrade && selectedFamilyPattern && patternMatchesTrade(selectedFamilyPattern, selectedRouteTrade));
  const selectedSourceLabel =
    SOURCE_OPTIONS.find((option) => option.value === sourceScope)?.label ?? 'All';
  const selectedSimAccountRules = SIM_ACCOUNT_RULES[simAccountSize] ?? SIM_ACCOUNT_RULES['50K'];
  const simContractsCount = Math.max(1, Number.parseInt(String(simContracts), 10) || 1);
  const simTestsCount = Math.max(1, Math.min(250, Number.parseInt(String(simTestsToChain), 10) || 1));
  const simDailyLossLimit =
    simDrawdownModel === 'eod' ? selectedSimAccountRules.eodDailyLossLimit : null;
  const selectedTradeResultR = Number(selectedRouteTrade?.result_r);
  const selectedTradePnl = Number(selectedRouteTrade?.pnl);
  const selectedTradeIsSkipped = Boolean(selectedRouteTrade?.skipped_for_overlap);
  const selectedTradeIsLoss =
    Number(selectedRouteTrade?.trade_result) === 2 ||
    (Number.isFinite(selectedTradeResultR) && selectedTradeResultR < 0) ||
    (Number.isFinite(selectedTradePnl) && selectedTradePnl < 0);
  const selectedFamilyLabel = selectedFamily
    ? [
        selectedFamily.harmonic_type,
        selectedFamily.bin,
        selectedFamily.size_bucket,
        selectedFamily.time_bin,
        selectedFamily.x_strictness,
      ]
        .filter(Boolean)
        .join(' / ')
    : 'No family selected';
  const topStats = [
    { label: 'Families', value: formatNumber(visibleFamilies.length) },
    { label: 'Setups', value: formatNumber(visibleSetupCount) },
    { label: 'Largest', value: formatNumber(largestFamily) },
    { label: 'Routes', value: formatNumber(phase1Results.length) },
    { label: 'Patterns', value: formatNumber(familyPatterns.length) },
  ];
  const selectedFamilyStats = [
    { label: 'Family Key', value: selectedFamily?.family_key ?? 'N/A', wide: true },
    { label: 'Setups', value: formatNumber(selectedFamily?.setup_count) },
    { label: 'Symbols', value: formatNumber(selectedFamily?.symbol_count) },
    { label: 'First D', value: formatDate(selectedFamily?.first_d_date) },
    { label: 'Last D', value: formatDate(selectedFamily?.last_d_date) },
  ];
  const selectedRouteStats = [
    { label: 'Rank', value: selectedRoute ? `#${selectedRoute.result_rank}` : 'N/A' },
    { label: 'Avg R', value: selectedRoute ? formatDecimal(selectedRoute.avg_r, 3) : 'N/A' },
    { label: 'Win', value: selectedRoute ? `${formatDecimal(selectedRoute.win_rate, 1)}%` : 'N/A' },
    { label: 'PF', value: selectedRoute ? formatDecimal(selectedRoute.profit_factor, 2) : 'N/A' },
    { label: 'DD', value: selectedRoute ? formatDecimal(selectedRoute.max_drawdown_r, 2) : 'N/A' },
  ];
  const selectedTradeDetailStats = [
    {
      label: 'Trade ID',
      value: selectedRouteTrade?.trade_uid ?? (selectedRouteTrade?.trade_id ? `#${selectedRouteTrade.trade_id}` : 'Replay only'),
      wide: true,
      code: true,
    },
    {
      label: 'Run ID',
      value: selectedRoute?.run_id ?? selectedRouteTrade?.run_id ?? 'N/A',
      wide: true,
      code: true,
    },
    {
      label: 'Pattern ID',
      value:
        selectedRouteTrade?.pattern_id ??
        selectedRouteTrade?.pattern_group_id ??
        canvasPattern?.pattern_id ??
        'N/A',
      wide: true,
    },
    { label: 'Symbol', value: selectedRouteTrade?.symbol ?? canvasPattern?.symbol ?? 'N/A' },
    {
      label: 'Direction',
      value: formatTradeDirection(selectedRouteTrade),
      tone: getTradeSide(selectedRouteTrade) === 'SHORT' ? 'loss' : getTradeSide(selectedRouteTrade) === 'LONG' ? 'win' : '',
    },
    {
      label: 'Result',
      value: selectedRouteTrade ? (selectedTradeIsSkipped ? 'Skipped' : selectedTradeIsLoss ? 'Loss' : 'Win') : 'N/A',
      tone: selectedRouteTrade ? (selectedTradeIsSkipped ? 'skipped' : selectedTradeIsLoss ? 'loss' : 'win') : '',
    },
    {
      label: 'Result R',
      value: Number.isFinite(selectedTradeResultR) ? formatDecimal(selectedTradeResultR, 2) : 'N/A',
      tone: Number.isFinite(selectedTradeResultR)
        ? selectedTradeResultR < 0
          ? 'loss'
          : 'win'
        : '',
    },
    {
      label: 'P/L',
      value: Number.isFinite(selectedTradePnl) ? formatMoney(selectedTradePnl) : 'N/A',
      tone: Number.isFinite(selectedTradePnl) ? (selectedTradePnl < 0 ? 'loss' : 'win') : '',
    },
    {
      label: 'Entry Price',
      value: formatDecimal(selectedRouteTrade?.trade_enter_price ?? canvasPattern?.trade_enter_price, 2),
    },
    {
      label: 'Stop Price',
      value: formatDecimal(selectedRouteTrade?.trade_risk_exit_price ?? canvasPattern?.trade_risk_exit_price, 2),
      tone: 'loss',
    },
    {
      label: 'Target Price',
      value: formatDecimal(selectedRouteTrade?.trade_reward_exit_price ?? canvasPattern?.trade_reward_exit_price, 2),
      tone: 'win',
    },
    {
      label: 'Exit Price',
      value: formatDecimal(
        selectedRouteTrade?.exit_price ??
          selectedRouteTrade?.trade_exit_price ??
          canvasPattern?.target_close ??
          canvasPattern?.trade_current_price,
        2
      ),
    },
    { label: 'Entry Date', value: formatDate(selectedRouteTrade?.entry_date ?? canvasPattern?.entry_date) },
    { label: 'Exit Date', value: formatDate(selectedRouteTrade?.target_date ?? canvasPattern?.target_date) },
    {
      label: 'Exit Reason',
      value: selectedRouteTrade?.exit_reason ? formatRouteMode(selectedRouteTrade.exit_reason) : 'N/A',
    },
    {
      label: 'Risk Points',
      value: Number.isFinite(Number(selectedRouteTrade?.risk_points ?? canvasPattern?.risk_points))
        ? formatDecimal(selectedRouteTrade?.risk_points ?? canvasPattern?.risk_points, 2)
      : 'N/A',
    },
  ];
  const selectedFamilyDetailStats = [...selectedFamilyStats, ...selectedRouteStats];
  const selectedRouteMarket =
    selectedRouteTrade?.market ?? canvasPattern?.market ?? selectedFamily?.market ?? '';
  const routeEntryExplanation = getDirectionalEntryCopy(selectedRoute?.entry_mode, selectedRouteMarket);
  const routeStopExplanation =
    ROUTE_STOP_COPY[selectedRoute?.stop_mode] ?? `uses ${formatRouteMode(selectedRoute?.stop_mode)} for stop loss`;
  const selectedRouteEntryFill = selectedRouteTrade
    ? `${formatDate(selectedRouteTrade.entry_date)} @ ${formatDecimal(selectedRouteTrade.trade_enter_price, 2)}`
    : selectedRoute
      ? 'select a trade to see the actual fill'
      : 'N/A';
  const selectedRouteEntryAction = getDirectionalEntryAction(
    selectedRoute?.entry_mode,
    selectedRouteMarket,
    selectedRouteTrade
  );
  const selectedRouteDisplayedSide =
    selectedRoute?.entry_mode === 'post_confirm_decision'
      ? getTradeSide(selectedRouteTrade) ?? getRouteDirectionMeta(selectedRouteMarket).side
      : getRouteDirectionMeta(selectedRouteMarket).side;
  const selectedRouteOverlayDetails = selectedRoute
    ? [
        {
          label: 'Entry',
          action: selectedRouteEntryAction,
          value: routeEntryExplanation,
          meta: selectedRouteEntryFill,
        },
        {
          label: 'Stop',
          action: getDirectionalStopAction(selectedRoute.stop_mode, selectedRouteMarket),
          value: routeStopExplanation,
          tone: 'loss',
        },
        {
          label: 'Target',
          action: `${formatDecimal(selectedRoute.target_r, 2)}R TARGET`,
          value: `takes profit at ${formatDecimal(selectedRoute.target_r, 2)}R from entry risk`,
          tone: 'win',
        },
        {
          label: 'Time Exit',
          action: `${selectedRoute.max_hold_multiple || '-'}X HOLD`,
          value: `closes after ${selectedRoute.max_hold_multiple || '-'}x pattern hold if stop/target has not hit`,
        },
      ]
    : [];
  const selectedRouteLogicStats = [
    {
      label: 'Route Label',
      value: selectedRoute?.route_label ?? 'N/A',
      wide: true,
    },
    { label: 'Route ID', value: selectedRoute?.route_id ?? 'N/A', wide: true },
    { label: 'Direction', value: selectedRouteDisplayedSide },
    { label: 'Entry Trigger', value: selectedRouteEntryAction },
    {
      label: 'Stop Loss',
      value: getDirectionalStopAction(selectedRoute?.stop_mode, selectedRouteMarket),
      tone: 'loss',
    },
    {
      label: 'Profit Target',
      value: selectedRoute ? `${formatDecimal(selectedRoute.target_r, 2)}R` : 'N/A',
      tone: 'win',
    },
    {
      label: 'Time Exit',
      value: selectedRoute ? `${selectedRoute.max_hold_multiple || '-'}x pattern hold` : 'N/A',
    },
    { label: 'Rank', value: selectedRoute ? `#${selectedRoute.result_rank}` : 'N/A' },
    { label: 'Avg R', value: selectedRoute ? formatDecimal(selectedRoute.avg_r, 3) : 'N/A' },
    { label: 'Win Rate', value: selectedRoute ? `${formatDecimal(selectedRoute.win_rate, 1)}%` : 'N/A' },
    { label: 'Profit Factor', value: selectedRoute ? formatDecimal(selectedRoute.profit_factor, 2) : 'N/A' },
    { label: 'Max DD', value: selectedRoute ? `${formatDecimal(selectedRoute.max_drawdown_r, 2)}R` : 'N/A' },
    { label: 'Worst Year', value: selectedRoute ? `${formatDecimal(selectedRoute.worst_year_avg_r, 3)}R` : 'N/A' },
    { label: 'Trades', value: selectedRoute ? formatNumber(selectedRoute.trade_count) : 'N/A' },
    { label: 'No Entry', value: selectedRoute ? formatNumber(selectedRoute.no_entry_count) : 'N/A' },
    {
      label: 'Replay Chain',
      value: `${simTestsCount} test${simTestsCount === 1 ? '' : 's'}`,
    },
    { label: 'Sizing', value: `${simAccountSize} / ${simContractsCount}x` },
    {
      label: 'Drawdown',
      value: `${formatRouteMode(simDrawdownModel)}${simOneTradeAtATime ? ' / One trade' : ''}`,
      wide: true,
    },
  ];
  const selectedPatternMarketTone =
    String(selectedFamilyPattern?.market ?? selectedFamily?.market ?? '').toLowerCase() === 'bearish'
      ? 'loss'
      : String(selectedFamilyPattern?.market ?? selectedFamily?.market ?? '').toLowerCase() === 'bullish'
        ? 'win'
        : '';
  const selectedFamilyRowMetrics = [
    { label: 'Family', value: selectedFamily?.family_key ?? 'N/A', wide: true },
    { label: 'Type', value: selectedFamilyLabel, wide: true },
    { label: 'Setups', value: formatNumber(selectedFamily?.setup_count) },
    { label: 'Symbols', value: formatNumber(selectedFamily?.symbol_count) },
    { label: 'Bin', value: selectedFamily?.bin ?? 'N/A' },
    { label: 'Size', value: selectedFamily?.size_bucket ?? 'N/A' },
    { label: 'Time', value: selectedFamily?.time_bin ?? 'N/A' },
    { label: 'X', value: selectedFamily?.x_strictness ?? 'N/A' },
    { label: 'First D', value: formatDate(selectedFamily?.first_d_date) },
    { label: 'Last D', value: formatDate(selectedFamily?.last_d_date) },
  ];
  const selectedRouteRowMetrics = [
    { label: 'Test', value: selectedRoute?.route_label ?? 'N/A', wide: true },
    { label: 'Test ID', value: selectedRoute?.route_id ?? 'N/A', wide: true },
    { label: 'Rank', value: selectedRoute ? `#${selectedRoute.result_rank}` : 'N/A' },
    {
      label: 'Avg R',
      value: selectedRoute ? formatDecimal(selectedRoute.avg_r, 3) : 'N/A',
      tone: selectedRoute ? (Number(selectedRoute.avg_r) < 0 ? 'loss' : 'win') : '',
    },
    {
      label: 'Win',
      value: selectedRoute ? `${formatDecimal(selectedRoute.win_rate, 1)}%` : 'N/A',
      tone: selectedRoute ? (Number(selectedRoute.win_rate) >= 50 ? 'win' : 'loss') : '',
    },
    { label: 'Trades', value: selectedRoute ? formatNumber(selectedRoute.trade_count) : 'N/A' },
    {
      label: 'PF',
      value: selectedRoute ? formatDecimal(selectedRoute.profit_factor, 2) : 'N/A',
      tone: selectedRoute ? (Number(selectedRoute.profit_factor) >= 1 ? 'win' : 'loss') : '',
    },
    { label: 'DD', value: selectedRoute ? `${formatDecimal(selectedRoute.max_drawdown_r, 2)}R` : 'N/A', tone: selectedRoute ? 'loss' : '' },
    {
      label: 'Worst Yr',
      value: selectedRoute ? `${formatDecimal(selectedRoute.worst_year_avg_r, 3)}R` : 'N/A',
      tone: selectedRoute ? (Number(selectedRoute.worst_year_avg_r) < 0 ? 'loss' : 'win') : '',
    },
    { label: 'Hold', value: selectedRoute ? `${selectedRoute.max_hold_multiple}x` : 'N/A' },
  ];
  const selectedPatternRowMetrics = [
    { label: 'Pattern', value: selectedFamilyPattern?.pattern_id ?? selectedFamilyPattern?.pattern_group_id ?? 'N/A', wide: true },
    { label: 'Symbol', value: selectedFamilyPattern?.symbol ?? 'N/A' },
    { label: 'Market', value: selectedFamilyPattern?.market ?? selectedFamily?.market ?? 'N/A', tone: selectedPatternMarketTone },
    { label: 'Harmonic', value: selectedFamilyPattern?.harmonic_type ?? selectedFamily?.harmonic_type ?? 'N/A' },
    { label: 'D Date', value: formatDate(selectedFamilyPattern?.d_date) },
    { label: 'Confirm', value: formatDate(selectedFamilyPattern?.d_confirm_date) },
    { label: 'Trade', value: selectedPatternTrade ? `#${selectedPatternTrade.trade_index ?? '-'}` : 'No trade', tone: selectedPatternTrade ? 'win' : '' },
  ];
  const selectedTradeRowMetrics = [
    {
      label: 'Trade ID',
      value: selectedRouteTrade?.trade_uid ?? (selectedRouteTrade?.trade_id ? `#${selectedRouteTrade.trade_id}` : 'N/A'),
      wide: true,
    },
    { label: 'Side', value: formatTradeDirection(selectedRouteTrade), tone: getTradeSide(selectedRouteTrade) === 'SHORT' ? 'loss' : getTradeSide(selectedRouteTrade) === 'LONG' ? 'win' : '' },
    {
      label: 'Result',
      value: selectedRouteTrade ? (selectedTradeIsSkipped ? 'Skipped' : selectedTradeIsLoss ? 'Loss' : 'Win') : 'N/A',
      tone: selectedRouteTrade ? (selectedTradeIsSkipped ? 'skipped' : selectedTradeIsLoss ? 'loss' : 'win') : '',
    },
    {
      label: 'R',
      value: Number.isFinite(selectedTradeResultR) ? formatDecimal(selectedTradeResultR, 2) : 'N/A',
      tone: Number.isFinite(selectedTradeResultR) ? (selectedTradeResultR < 0 ? 'loss' : 'win') : '',
    },
    {
      label: 'P/L',
      value: Number.isFinite(selectedTradePnl) ? formatMoney(selectedTradePnl) : 'N/A',
      tone: Number.isFinite(selectedTradePnl) ? (selectedTradePnl < 0 ? 'loss' : 'win') : '',
    },
    { label: 'Entry', value: formatDate(selectedRouteTrade?.entry_date) },
    { label: 'Exit', value: formatDate(selectedRouteTrade?.target_date) },
    {
      label: 'Exit Why',
      value: selectedRouteTrade?.exit_reason ? formatRouteMode(selectedRouteTrade.exit_reason) : 'N/A',
      tone: selectedRouteTrade?.exit_reason === 'stop' ? 'loss' : selectedRouteTrade?.exit_reason === 'target' ? 'win' : '',
    },
    { label: 'Pattern', value: selectedRouteTrade?.pattern_id ?? selectedRouteTrade?.pattern_group_id ?? 'N/A', wide: true },
  ];
  const selectedTestOverviewSections = selectedRoute
    ? [
        {
          title: 'Performance',
          items: [
            { label: 'Score', value: formatDecimal(selectedRoute.score, 1), tone: 'win' },
            { label: 'Rank', value: `#${selectedRoute.result_rank}` },
            {
              label: 'Avg R',
              value: formatDecimal(selectedRoute.avg_r, 3),
              tone: Number(selectedRoute.avg_r) < 0 ? 'loss' : 'win',
            },
            {
              label: 'Win Rate',
              value: `${formatDecimal(selectedRoute.win_rate, 1)}%`,
              tone: Number(selectedRoute.win_rate) >= 50 ? 'win' : 'loss',
            },
            {
              label: 'Profit Factor',
              value: formatDecimal(selectedRoute.profit_factor, 2),
              tone: Number(selectedRoute.profit_factor) >= 1 ? 'win' : 'loss',
            },
            { label: 'Max DD', value: `${formatDecimal(selectedRoute.max_drawdown_r, 2)}R`, tone: 'loss' },
            {
              label: 'Worst Year',
              value: `${formatDecimal(selectedRoute.worst_year_avg_r, 3)}R`,
              tone: Number(selectedRoute.worst_year_avg_r) < 0 ? 'loss' : 'win',
            },
            { label: 'Trades', value: formatNumber(selectedRoute.trade_count) },
          ],
        },
        {
          title: 'Coverage',
          items: [
            { label: 'Setups', value: formatNumber(selectedRoute.setup_count) },
            { label: 'No Entry', value: formatNumber(selectedRoute.no_entry_count), tone: 'skipped' },
            { label: 'Wins', value: formatNumber(selectedRoute.win_count), tone: 'win' },
            { label: 'Losses', value: formatNumber(selectedRoute.loss_count), tone: 'loss' },
            { label: 'Source', value: selectedSourceLabel },
            { label: 'Year', value: yearFilter === 'All' ? 'All Years' : yearFilter },
          ],
        },
        {
          title: 'Route Logic',
          items: [
            { label: 'Direction', value: selectedRouteDisplayedSide, tone: selectedRouteDisplayedSide === 'SHORT' ? 'loss' : 'win' },
            { label: 'Entry', value: selectedRouteEntryAction, wide: true },
            { label: 'Stop', value: getDirectionalStopAction(selectedRoute.stop_mode, selectedRouteMarket), tone: 'loss' },
            { label: 'Target', value: `${formatDecimal(selectedRoute.target_r, 2)}R`, tone: 'win' },
            { label: 'Hold', value: `${selectedRoute.max_hold_multiple || '-'}x pattern` },
            { label: 'Test ID', value: selectedRoute.route_id, wide: true },
          ],
        },
      ]
    : [];
  const testTradeAnalytics = useMemo(() => {
    const createGroup = (key) => ({
      key,
      trades: 0,
      wins: 0,
      losses: 0,
      skipped: 0,
      totalR: 0,
      totalPnl: 0,
      pnlCount: 0,
    });
    const addTrade = (group, trade) => {
      const resultR = Number(trade.result_r);
      const pnl = Number(trade.pnl);
      const skipped = Boolean(trade.skipped_for_overlap);
      const won =
        !skipped &&
        (Number(trade.trade_result) === 1 ||
          (Number.isFinite(resultR) && resultR > 0) ||
          (Number.isFinite(pnl) && pnl > 0));
      const lost =
        !skipped &&
        (Number(trade.trade_result) === 2 ||
          (Number.isFinite(resultR) && resultR < 0) ||
          (Number.isFinite(pnl) && pnl < 0));

      group.trades += 1;
      if (skipped) group.skipped += 1;
      if (won) group.wins += 1;
      if (lost) group.losses += 1;
      if (Number.isFinite(resultR)) group.totalR += resultR;
      if (Number.isFinite(pnl)) {
        group.totalPnl += pnl;
        group.pnlCount += 1;
      }
    };
    const finishGroup = (group) => ({
      ...group,
      avgR: group.trades ? group.totalR / group.trades : 0,
      winRate: group.trades ? (group.wins / group.trades) * 100 : 0,
      pnl: group.pnlCount ? group.totalPnl : null,
    });
    const symbolMap = new Map();
    const exitMap = new Map();
    const directionMap = new Map();
    const resultMix = {
      trades: routeTrades.length,
      skipped: 0,
      wins: 0,
      losses: 0,
      totalR: 0,
      winR: 0,
      winCount: 0,
      lossR: 0,
      lossCount: 0,
    };

    routeTrades.forEach((trade) => {
      const symbol = trade.symbol || 'N/A';
      const exitReason = trade.exit_reason ? formatRouteMode(trade.exit_reason) : 'Unknown';
      const direction = getTradeSide(trade) ?? formatRouteMode(trade.trade_direction || 'Unknown');
      const resultR = Number(trade.result_r);
      const pnl = Number(trade.pnl);
      const skipped = Boolean(trade.skipped_for_overlap);
      const won =
        !skipped &&
        (Number(trade.trade_result) === 1 ||
          (Number.isFinite(resultR) && resultR > 0) ||
          (Number.isFinite(pnl) && pnl > 0));
      const lost =
        !skipped &&
        (Number(trade.trade_result) === 2 ||
          (Number.isFinite(resultR) && resultR < 0) ||
          (Number.isFinite(pnl) && pnl < 0));

      if (!symbolMap.has(symbol)) symbolMap.set(symbol, createGroup(symbol));
      if (!exitMap.has(exitReason)) exitMap.set(exitReason, createGroup(exitReason));
      if (!directionMap.has(direction)) directionMap.set(direction, createGroup(direction));
      addTrade(symbolMap.get(symbol), trade);
      addTrade(exitMap.get(exitReason), trade);
      addTrade(directionMap.get(direction), trade);

      if (skipped) resultMix.skipped += 1;
      if (won) resultMix.wins += 1;
      if (lost) resultMix.losses += 1;
      if (Number.isFinite(resultR)) {
        resultMix.totalR += resultR;
        if (resultR > 0) {
          resultMix.winR += resultR;
          resultMix.winCount += 1;
        }
        if (resultR < 0) {
          resultMix.lossR += resultR;
          resultMix.lossCount += 1;
        }
      }
    });

    const symbols = Array.from(symbolMap.values()).map(finishGroup);
    const exits = Array.from(exitMap.values()).map(finishGroup);
    const directions = Array.from(directionMap.values()).map(finishGroup);
    const byAvgR = [...symbols].sort((left, right) => right.avgR - left.avgR);
    const byTotalR = [...symbols].sort((left, right) => right.totalR - left.totalR);
    const byTrades = [...symbols].sort((left, right) => right.trades - left.trades);

    return {
      symbols,
      symbolsByAvgR: byAvgR,
      symbolsByTotalR: byTotalR,
      symbolsByTrades: byTrades,
      exits: exits.sort((left, right) => right.trades - left.trades),
      directions: directions.sort((left, right) => right.trades - left.trades),
      resultMix,
    };
  }, [routeTrades]);
  const worstAvgSymbol =
    testTradeAnalytics.symbolsByAvgR[testTradeAnalytics.symbolsByAvgR.length - 1] ?? null;
  const bestPnlSymbol =
    testTradeAnalytics.symbols
      .filter((item) => item.pnl !== null)
      .sort((left, right) => (right.pnl ?? 0) - (left.pnl ?? 0))[0] ?? null;
  const selectedTestSymbolSections = selectedRoute
    ? [
        {
          title: 'Symbol Leaders',
          items: [
            {
              label: 'Best Avg R',
              value: testTradeAnalytics.symbolsByAvgR[0]
                ? `${testTradeAnalytics.symbolsByAvgR[0].key} / ${formatDecimal(testTradeAnalytics.symbolsByAvgR[0].avgR, 2)}R`
                : 'N/A',
              tone: testTradeAnalytics.symbolsByAvgR[0]?.avgR >= 0 ? 'win' : 'loss',
              wide: true,
            },
            {
              label: 'Best Total R',
              value: testTradeAnalytics.symbolsByTotalR[0]
                ? `${testTradeAnalytics.symbolsByTotalR[0].key} / ${formatDecimal(testTradeAnalytics.symbolsByTotalR[0].totalR, 2)}R`
                : 'N/A',
              tone: testTradeAnalytics.symbolsByTotalR[0]?.totalR >= 0 ? 'win' : 'loss',
              wide: true,
            },
            {
              label: 'Most Trades',
              value: testTradeAnalytics.symbolsByTrades[0]
                ? `${testTradeAnalytics.symbolsByTrades[0].key} / ${formatNumber(testTradeAnalytics.symbolsByTrades[0].trades)}`
                : 'N/A',
            },
            {
              label: 'Worst Avg R',
              value: worstAvgSymbol
                ? `${worstAvgSymbol.key} / ${formatDecimal(worstAvgSymbol.avgR, 2)}R`
                : 'N/A',
              tone: 'loss',
            },
          ],
        },
        {
          title: 'Symbol Board',
          items: testTradeAnalytics.symbolsByTotalR.slice(0, 10).map((symbol) => ({
            label: symbol.key,
            value: `${formatDecimal(symbol.avgR, 2)}R avg / ${formatDecimal(symbol.winRate, 0)}% / ${formatNumber(symbol.trades)} trades`,
            tone: symbol.totalR < 0 ? 'loss' : 'win',
            wide: true,
          })),
        },
        {
          title: 'Symbol Notes',
          items: [
            { label: 'Symbols', value: formatNumber(testTradeAnalytics.symbols.length) },
            { label: 'Loaded Trades', value: formatNumber(routeTrades.length) },
            { label: 'Best P/L', value: bestPnlSymbol ? `${bestPnlSymbol.key} / ${formatMoney(bestPnlSymbol.pnl)}` : 'N/A', tone: bestPnlSymbol && bestPnlSymbol.pnl < 0 ? 'loss' : 'win' },
            { label: 'Use Next', value: 'Add session/day filters', wide: true },
          ],
        },
      ]
    : [];
  const selectedTestOutcomeSections = selectedRoute
    ? [
        {
          title: 'Result Mix',
          items: [
            { label: 'Wins', value: formatNumber(testTradeAnalytics.resultMix.wins), tone: 'win' },
            { label: 'Losses', value: formatNumber(testTradeAnalytics.resultMix.losses), tone: 'loss' },
            { label: 'Skipped', value: formatNumber(testTradeAnalytics.resultMix.skipped), tone: 'skipped' },
            {
              label: 'Total R',
              value: `${formatDecimal(testTradeAnalytics.resultMix.totalR, 2)}R`,
              tone: testTradeAnalytics.resultMix.totalR < 0 ? 'loss' : 'win',
            },
            {
              label: 'Avg Win',
              value: testTradeAnalytics.resultMix.winCount
                ? `${formatDecimal(testTradeAnalytics.resultMix.winR / testTradeAnalytics.resultMix.winCount, 2)}R`
                : 'N/A',
              tone: 'win',
            },
            {
              label: 'Avg Loss',
              value: testTradeAnalytics.resultMix.lossCount
                ? `${formatDecimal(testTradeAnalytics.resultMix.lossR / testTradeAnalytics.resultMix.lossCount, 2)}R`
                : 'N/A',
              tone: 'loss',
            },
          ],
        },
        {
          title: 'Exit Reasons',
          items: testTradeAnalytics.exits.map((item) => ({
            label: item.key,
            value: `${formatNumber(item.trades)} trades / ${formatDecimal(item.avgR, 2)}R avg`,
            tone: item.key.toLowerCase().includes('stop') || item.avgR < 0 ? 'loss' : item.key.toLowerCase().includes('target') ? 'win' : '',
            wide: true,
          })),
        },
        {
          title: 'Direction Split',
          items: testTradeAnalytics.directions.map((item) => ({
            label: item.key,
            value: `${formatNumber(item.trades)} trades / ${formatDecimal(item.winRate, 0)}% / ${formatDecimal(item.totalR, 2)}R`,
            tone: item.key === 'SHORT' ? 'loss' : item.key === 'LONG' ? 'win' : '',
            wide: true,
          })),
        },
      ]
    : [];
  const familyRouteAnalytics = useMemo(() => {
    const routes = phase1Results.filter((route) => route && route.route_id);
    const byScore = [...routes].sort((left, right) => Number(right.score) - Number(left.score));
    const byAvgR = [...routes].sort((left, right) => Number(right.avg_r) - Number(left.avg_r));
    const byTrades = [...routes].sort((left, right) => Number(right.trade_count) - Number(left.trade_count));
    const byWin = [...routes].sort((left, right) => Number(right.win_rate) - Number(left.win_rate));
    const byDrawdown = [...routes].sort((left, right) => Number(left.max_drawdown_r) - Number(right.max_drawdown_r));
    const activeRoutes = routes.filter((route) => Number(route.trade_count) > 0);
    const avgScore = routes.length
      ? routes.reduce((sum, route) => sum + (Number(route.score) || 0), 0) / routes.length
      : 0;
    const totalTrades = routes.reduce((sum, route) => sum + (Number(route.trade_count) || 0), 0);

    return {
      routes,
      activeRoutes,
      byScore,
      byAvgR,
      byTrades,
      byWin,
      byDrawdown,
      avgScore,
      totalTrades,
    };
  }, [phase1Results]);
  const selectedFamilyOverviewSections = selectedFamily
    ? [
        {
          title: 'Family Identity',
          items: [
            { label: 'Family Key', value: selectedFamily.family_key, wide: true },
            { label: 'Type', value: selectedFamilyLabel, wide: true },
            { label: 'Setups', value: formatNumber(selectedFamily.setup_count) },
            { label: 'Patterns Loaded', value: formatNumber(familyPatterns.length) },
            { label: 'Symbols', value: formatNumber(selectedFamily.symbol_count) },
            { label: 'Date Range', value: `${formatDate(selectedFamily.first_d_date)} to ${formatDate(selectedFamily.last_d_date)}`, wide: true },
          ],
        },
        {
          title: 'Family Test Leaders',
          items: [
            {
              label: 'Best Score',
              value: familyRouteAnalytics.byScore[0]
                ? `#${familyRouteAnalytics.byScore[0].result_rank} / ${formatDecimal(familyRouteAnalytics.byScore[0].score, 1)}`
                : 'N/A',
              tone: 'win',
            },
            {
              label: 'Best Avg R',
              value: familyRouteAnalytics.byAvgR[0]
                ? `${formatDecimal(familyRouteAnalytics.byAvgR[0].avg_r, 2)}R`
                : 'N/A',
              tone: familyRouteAnalytics.byAvgR[0] && Number(familyRouteAnalytics.byAvgR[0].avg_r) < 0 ? 'loss' : 'win',
            },
            {
              label: 'Best Win',
              value: familyRouteAnalytics.byWin[0]
                ? `${formatDecimal(familyRouteAnalytics.byWin[0].win_rate, 1)}%`
                : 'N/A',
              tone: 'win',
            },
            {
              label: 'Lowest DD',
              value: familyRouteAnalytics.byDrawdown[0]
                ? `${formatDecimal(familyRouteAnalytics.byDrawdown[0].max_drawdown_r, 2)}R`
                : 'N/A',
            },
            { label: 'Tests Loaded', value: formatNumber(familyRouteAnalytics.routes.length) },
            { label: 'Active Tests', value: formatNumber(familyRouteAnalytics.activeRoutes.length), tone: 'win' },
            { label: 'Total Test Trades', value: formatNumber(familyRouteAnalytics.totalTrades), wide: true },
            { label: 'Avg Score', value: formatDecimal(familyRouteAnalytics.avgScore, 1) },
          ],
        },
        {
          title: 'Top Tests In Family',
          items: familyRouteAnalytics.byScore.slice(0, 8).map((route) => ({
            label: `#${route.result_rank}`,
            value: `${formatDecimal(route.score, 1)} score / ${formatDecimal(route.avg_r, 2)}R / ${formatDecimal(route.win_rate, 0)}%`,
            tone: Number(route.avg_r) < 0 ? 'loss' : 'win',
            wide: true,
          })),
        },
      ]
    : [];
  const routeFamilyAnalytics = useMemo(() => {
    const rows = routeFamilyRows.filter((row) => row && row.family_key);
    const byScore = [...rows].sort((left, right) => Number(right.score) - Number(left.score));
    const byAvgR = [...rows].sort((left, right) => Number(right.avg_r) - Number(left.avg_r));
    const byTrades = [...rows].sort((left, right) => Number(right.trade_count) - Number(left.trade_count));
    const byWin = [...rows].sort((left, right) => Number(right.win_rate) - Number(left.win_rate));
    const byWorstYear = [...rows].sort((left, right) => Number(right.worst_year_avg_r) - Number(left.worst_year_avg_r));
    const activeFamilies = rows.filter((row) => Number(row.trade_count) > 0);
    const totalTrades = rows.reduce((sum, row) => sum + (Number(row.trade_count) || 0), 0);
    const avgScore = rows.length
      ? rows.reduce((sum, row) => sum + (Number(row.score) || 0), 0) / rows.length
      : 0;

    return {
      rows,
      activeFamilies,
      byScore,
      byAvgR,
      byTrades,
      byWin,
      byWorstYear,
      totalTrades,
      avgScore,
    };
  }, [routeFamilyRows]);
  const selectedRouteFamiliesSections = selectedRoute
    ? [
        {
          title: 'Route Family Coverage',
          items: [
            { label: 'Families Ran', value: formatNumber(routeFamilyAnalytics.rows.length), tone: routeFamilyAnalytics.rows.length ? 'win' : '' },
            { label: 'Active Families', value: formatNumber(routeFamilyAnalytics.activeFamilies.length), tone: routeFamilyAnalytics.activeFamilies.length ? 'win' : '' },
            { label: 'Total Trades', value: formatNumber(routeFamilyAnalytics.totalTrades) },
            { label: 'Avg Family Score', value: formatDecimal(routeFamilyAnalytics.avgScore, 1) },
            {
              label: 'Best Family',
              value: routeFamilyAnalytics.byScore[0]
                ? `${routeFamilyAnalytics.byScore[0].harmonic_type || 'Family'} / ${formatDecimal(routeFamilyAnalytics.byScore[0].score, 1)}`
                : 'N/A',
              tone: 'win',
              wide: true,
            },
            {
              label: 'Current Family',
              value: selectedFamily?.family_key ?? 'N/A',
              wide: true,
            },
          ],
        },
        {
          title: 'Family Leaders',
          items: [
            {
              label: 'Best Avg R',
              value: routeFamilyAnalytics.byAvgR[0]
                ? `${routeFamilyAnalytics.byAvgR[0].family_key} / ${formatDecimal(routeFamilyAnalytics.byAvgR[0].avg_r, 2)}R`
                : 'N/A',
              tone: routeFamilyAnalytics.byAvgR[0] && Number(routeFamilyAnalytics.byAvgR[0].avg_r) < 0 ? 'loss' : 'win',
              wide: true,
            },
            {
              label: 'Best Win Rate',
              value: routeFamilyAnalytics.byWin[0]
                ? `${routeFamilyAnalytics.byWin[0].family_key} / ${formatDecimal(routeFamilyAnalytics.byWin[0].win_rate, 1)}%`
                : 'N/A',
              tone: 'win',
              wide: true,
            },
            {
              label: 'Most Trades',
              value: routeFamilyAnalytics.byTrades[0]
                ? `${routeFamilyAnalytics.byTrades[0].family_key} / ${formatNumber(routeFamilyAnalytics.byTrades[0].trade_count)}`
                : 'N/A',
              wide: true,
            },
            {
              label: 'Best Worst Year',
              value: routeFamilyAnalytics.byWorstYear[0]
                ? `${routeFamilyAnalytics.byWorstYear[0].family_key} / ${formatDecimal(routeFamilyAnalytics.byWorstYear[0].worst_year_avg_r, 2)}R`
                : 'N/A',
              tone: routeFamilyAnalytics.byWorstYear[0] && Number(routeFamilyAnalytics.byWorstYear[0].worst_year_avg_r) < 0 ? 'loss' : 'win',
              wide: true,
            },
          ],
        },
        {
          title: `Family Board (${formatNumber(routeFamilyAnalytics.byScore.length)})`,
          items: routeFamilyAnalytics.byScore.map((row) => ({
            label: row.harmonic_type || row.family_key,
            value: `${formatDecimal(row.score, 1)} score / ${formatDecimal(row.avg_r, 2)}R / ${formatDecimal(row.win_rate, 0)}% / ${formatNumber(row.trade_count)} trades`,
            tone: Number(row.avg_r) < 0 ? 'loss' : 'win',
            wide: true,
          })),
        },
      ]
    : [];
  const supplyAnalytics = useMemo(() => {
    const apiSymbolRows = supplyData.symbols ?? [];
    const apiFamilyRows = supplyData.families ?? [];
    const selectedFamilySymbolMap = new Map();

    familyPatterns.forEach((pattern) => {
      const symbol = pattern.symbol || 'N/A';
      const current = selectedFamilySymbolMap.get(symbol) ?? {
        symbol,
        setup_count: 0,
        pattern_count: 0,
        family_count: selectedFamily ? 1 : 0,
        contract_count: 1,
        first_d_date: pattern.d_date,
        last_d_date: pattern.d_date,
      };
      current.setup_count += 1;
      current.pattern_count += 1;
      if (pattern.d_date && (!current.first_d_date || new Date(pattern.d_date) < new Date(current.first_d_date))) {
        current.first_d_date = pattern.d_date;
      }
      if (pattern.d_date && (!current.last_d_date || new Date(pattern.d_date) > new Date(current.last_d_date))) {
        current.last_d_date = pattern.d_date;
      }
      selectedFamilySymbolMap.set(symbol, current);
    });

    const fallbackFamilyRows = visibleFamilies.map((family) => ({
      family_key: family.family_key,
      harmonic_type: family.harmonic_type,
      bin: family.bin,
      size_bucket: family.size_bucket,
      time_bin: family.time_bin,
      x_strictness: family.x_strictness,
      setup_count: family.setup_count,
      pattern_count: family.setup_count,
      symbol_count: family.symbol_count,
      first_d_date: family.first_d_date,
      last_d_date: family.last_d_date,
    }));
    const symbolRows = apiSymbolRows.length ? apiSymbolRows : Array.from(selectedFamilySymbolMap.values());
    const familyRows = apiFamilyRows.length ? apiFamilyRows : fallbackFamilyRows;
    const totalSetups = symbolRows.reduce((sum, row) => sum + (Number(row.setup_count) || 0), 0);
    const totalPatterns = symbolRows.reduce((sum, row) => sum + (Number(row.pattern_count) || 0), 0);
    const byFamilies = [...symbolRows].sort((left, right) => Number(right.family_count) - Number(left.family_count));
    const byPatterns = [...symbolRows].sort((left, right) => Number(right.pattern_count) - Number(left.pattern_count));
    const bySymbols = [...familyRows].sort((left, right) => Number(right.symbol_count) - Number(left.symbol_count));

    return {
      isFallback: !apiSymbolRows.length || !apiFamilyRows.length,
      symbolScope: apiSymbolRows.length ? 'Universe' : 'Selected family',
      familyScope: apiFamilyRows.length ? 'Universe' : 'Loaded families',
      symbolRows,
      familyRows,
      totalSetups,
      totalPatterns,
      byFamilies,
      byPatterns,
      bySymbols,
    };
  }, [familyPatterns, selectedFamily, supplyData, visibleFamilies]);
  const supplyOverviewSections = [
    {
      title: 'Supply Summary',
      items: [
        { label: 'Symbols', value: formatNumber(supplyAnalytics.symbolRows.length), tone: supplyAnalytics.symbolRows.length ? 'win' : '' },
        { label: 'Families', value: formatNumber(supplyAnalytics.familyRows.length), tone: supplyAnalytics.familyRows.length ? 'win' : '' },
        { label: 'Setups', value: formatNumber(supplyAnalytics.totalSetups), wide: true },
        { label: 'Patterns', value: formatNumber(supplyAnalytics.totalPatterns), wide: true },
        { label: 'Symbol Scope', value: supplyAnalytics.symbolScope, tone: supplyAnalytics.isFallback ? 'skipped' : 'win' },
        { label: 'Family Scope', value: supplyAnalytics.familyScope, tone: supplyAnalytics.isFallback ? 'skipped' : 'win' },
        {
          label: 'Top Symbol',
          value: supplyAnalytics.symbolRows[0]
            ? `${supplyAnalytics.symbolRows[0].symbol} / ${formatNumber(supplyAnalytics.symbolRows[0].setup_count)} setups`
            : 'N/A',
          wide: true,
        },
        {
          label: 'Most Families',
          value: supplyAnalytics.byFamilies[0]
            ? `${supplyAnalytics.byFamilies[0].symbol} / ${formatNumber(supplyAnalytics.byFamilies[0].family_count)} families`
            : 'N/A',
          wide: true,
        },
      ],
    },
    {
      title: 'Symbol Supply',
      items: supplyAnalytics.symbolRows.map((row) => ({
        label: row.symbol,
        value: `${formatNumber(row.setup_count)} setups / ${formatNumber(row.pattern_count)} patterns / ${formatNumber(row.family_count)} families`,
        wide: true,
      })),
    },
    {
      title: 'Family Supply',
      items: supplyAnalytics.familyRows.map((row) => ({
        label: row.harmonic_type || row.family_key,
        value: `${formatNumber(row.setup_count)} setups / ${formatNumber(row.pattern_count)} patterns / ${formatNumber(row.symbol_count)} symbols`,
        wide: true,
      })),
    },
  ];
  const activeTestOverviewSections =
    testOverviewTab === 'supply'
      ? supplyOverviewSections
      : testOverviewTab === 'families'
      ? selectedRouteFamiliesSections
      : testOverviewTab === 'family'
      ? selectedFamilyOverviewSections
      : testOverviewTab === 'symbols'
      ? selectedTestSymbolSections
      : testOverviewTab === 'outcomes'
        ? selectedTestOutcomeSections
        : selectedTestOverviewSections;

  useEffect(() => {
    if (initialFamilyKey) {
      setSelectedFamilyKey(initialFamilyKey);
    }
  }, [initialFamilyKey]);

  useEffect(() => {
    if (isLoading) return;

    const selectedStillVisible = visibleFamilies.some(
      (family) => family.family_key === selectedFamilyKey
    );
    if (!selectedStillVisible) {
      setSelectedFamilyKey(visibleFamilies[0]?.family_key ?? null);
    }
  }, [isLoading, selectedFamilyKey, visibleFamilies]);

  useEffect(() => {
    let isCancelled = false;

    const loadFamilyPatterns = async () => {
      if (!selectedFamilyKey) {
        setFamilyPatterns([]);
        setFamilyPatternsError('');
        setFamilyPatternsLoading(false);
        return;
      }

      try {
        setFamilyPatternsLoading(true);
        setFamilyPatternsError('');
        const result = await fetchPhase1FamilyPatterns(
          {
            familyKey: selectedFamilyKey,
            sourceScope,
            year: yearFilter === 'All' ? null : yearFilter,
          },
          { limit: 500, offset: 0, includeCount: true }
        );

        if (!isCancelled) {
          setFamilyPatterns(result.patterns ?? []);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setFamilyPatternsError('Could not load family patterns.');
          setFamilyPatterns([]);
        }
      } finally {
        if (!isCancelled) {
          setFamilyPatternsLoading(false);
        }
      }
    };

    void loadFamilyPatterns();

    return () => {
      isCancelled = true;
    };
  }, [selectedFamilyKey, sourceScope, yearFilter]);

  useEffect(() => {
    const selectedPatternStillVisible = familyPatterns.some(
      (pattern) => getFamilyPatternKey(pattern) === selectedFamilyPatternKey
    );

    if (!selectedPatternStillVisible) {
      setSelectedFamilyPatternKey(familyPatterns[0] ? getFamilyPatternKey(familyPatterns[0]) : null);
    }
  }, [familyPatterns, selectedFamilyPatternKey]);

  useEffect(() => {
    if (!selectedRouteTradeFamilyPattern) {
      return;
    }

    const tradePatternKey = getFamilyPatternKey(selectedRouteTradeFamilyPattern);
    if (tradePatternKey !== selectedFamilyPatternKey) {
      setSelectedFamilyPatternKey(tradePatternKey);
    }
  }, [selectedFamilyPatternKey, selectedRouteTradeFamilyPattern]);

  useEffect(() => {
    let isCancelled = false;

    const loadPhase1Results = async () => {
      if (!selectedFamilyKey) {
        setPhase1Results([]);
        setPhase1Error('');
        return;
      }

      try {
        setPhase1Loading(true);
        setPhase1Error('');
        const rows = await fetchPhase1Results({
          familyKey: selectedFamilyKey,
          sourceScope,
          year: yearFilter === 'All' ? null : yearFilter,
          limit: 250,
        });
        if (!isCancelled) {
          setPhase1Results(rows);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setPhase1Error('Could not load Phase 1 results.');
        }
      } finally {
        if (!isCancelled) {
          setPhase1Loading(false);
        }
      }
    };

    void loadPhase1Results();

    return () => {
      isCancelled = true;
    };
  }, [selectedFamilyKey, sourceScope, yearFilter]);

  useEffect(() => {
    const selectedRouteStillVisible = phase1Results.some(
      (result) => getRouteKey(result) === selectedRouteKey
    );

    if (!selectedRouteStillVisible) {
      setSelectedRouteKey(phase1Results[0] ? getRouteKey(phase1Results[0]) : null);
    }
  }, [phase1Results, selectedRouteKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadSupplyData = async () => {
      try {
        setSupplyLoading(true);
        setSupplyError('');
        const data = await fetchPhase1Supply({
          sourceScope,
          year: yearFilter === 'All' ? null : yearFilter,
          limit: 500,
        });

        if (!isCancelled) {
          setSupplyData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setSupplyError('Could not load supply data.');
          setSupplyData({ symbols: [], families: [] });
        }
      } finally {
        if (!isCancelled) {
          setSupplyLoading(false);
        }
      }
    };

    void loadSupplyData();

    return () => {
      isCancelled = true;
    };
  }, [sourceScope, yearFilter]);

  useEffect(() => {
    let isCancelled = false;

    const loadRouteFamilies = async () => {
      if (!selectedRoute?.route_id) {
        setRouteFamilyRows([]);
        setRouteFamiliesError('');
        setRouteFamiliesLoading(false);
        return;
      }

      try {
        setRouteFamiliesLoading(true);
        setRouteFamiliesError('');
        const rows = await fetchPhase1Leaderboard({
          sourceScope,
          year: yearFilter === 'All' ? null : yearFilter,
          limit: 2000,
          minTradeCount: 0,
          minSetupCount: 0,
          bestPerFamily: false,
          routeId: selectedRoute.route_id,
        });

        if (!isCancelled) {
          setRouteFamilyRows(rows);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setRouteFamiliesError('Could not load families for this test.');
          setRouteFamilyRows([]);
        }
      } finally {
        if (!isCancelled) {
          setRouteFamiliesLoading(false);
        }
      }
    };

    void loadRouteFamilies();

    return () => {
      isCancelled = true;
    };
  }, [selectedRoute?.route_id, sourceScope, yearFilter]);

  useEffect(() => {
    let isCancelled = false;

    const loadRouteTrades = async () => {
      if (!selectedFamilyKey || !selectedRoute) {
        setRouteTrades([]);
        setRouteTradesError('');
        setRouteTradesLoading(false);
        return;
      }

      try {
        setRouteTradesLoading(true);
        setRouteTradesError('');
        setRouteTrades([]);
        const replay = await fetchPhase1RouteReplay({
          familyKey: selectedFamilyKey,
          runId: selectedRoute.run_id,
          routeId: selectedRoute.route_id,
          firstStartDate: selectedFamily?.first_d_date ?? '2021-01-01',
          testsToChain: simTestsCount,
          contracts: simContractsCount,
          accountRules: {
            ...selectedSimAccountRules,
            dailyLossLimit: simDailyLossLimit,
          },
          drawdownModel: simDrawdownModel,
          oneTradeAtATime: simOneTradeAtATime,
        });

        if (!isCancelled) {
          setRouteTrades(replay?.trades ?? []);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setRouteTradesError('Could not load trades.');
        }
      } finally {
        if (!isCancelled) {
          setRouteTradesLoading(false);
        }
      }
    };

    void loadRouteTrades();

    return () => {
      isCancelled = true;
    };
  }, [
    selectedFamily?.first_d_date,
    selectedFamilyKey,
    selectedRoute,
    selectedSimAccountRules,
    simContractsCount,
    simDailyLossLimit,
    simDrawdownModel,
    simOneTradeAtATime,
    simTestsCount,
  ]);

  useEffect(() => {
    let isCancelled = false;

    const loadSelectedPatternRouteTrade = async () => {
      if (!selectedFamilyKey || !selectedRoute || !selectedFamilyPattern) {
        setSelectedPatternRouteTrade(null);
        setPatternRouteTradeError('');
        setPatternRouteTradeLoading(false);
        return;
      }

      try {
        setPatternRouteTradeLoading(true);
        setPatternRouteTradeError('');
        const replay = await fetchPhase1PatternRouteReplay({
          familyKey: selectedFamilyKey,
          runId: selectedRoute.run_id,
          routeId: selectedRoute.route_id,
          patternId: selectedFamilyPattern.pattern_id,
          patternGroupId: selectedFamilyPattern.pattern_group_id,
          contracts: simContractsCount,
        });
        const trade = replay?.trades?.[0] ?? null;

        if (!isCancelled) {
          if (trade) {
            const routedTrade = {
              ...trade,
              route_id: selectedRoute.route_id,
              route_label: selectedRoute.route_label,
              prop_outcome_mode: 'phase1-family',
            };
            setSelectedPatternRouteTrade(routedTrade);
            if (!selectedRouteTradeMatchesSelectedPattern) {
              setSelectedRouteTradeKey(getRouteTradeKey(routedTrade));
            }
          } else {
            setSelectedPatternRouteTrade(null);
            setPatternRouteTradeError('Selected test produced no trade for this pattern.');
          }
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setSelectedPatternRouteTrade(null);
          setPatternRouteTradeError('Could not replay the selected test on this pattern.');
        }
      } finally {
        if (!isCancelled) {
          setPatternRouteTradeLoading(false);
        }
      }
    };

    void loadSelectedPatternRouteTrade();

    return () => {
      isCancelled = true;
    };
  }, [selectedFamilyKey, selectedFamilyPattern, selectedRoute, selectedRouteTradeMatchesSelectedPattern, simContractsCount]);

  useEffect(() => {
    const selectedTradeStillVisible = routeTrades.some(
      (trade) => getRouteTradeKey(trade) === selectedRouteTradeKey
    );

    if (!selectedTradeStillVisible) {
      setSelectedRouteTradeKey(
        selectedPatternTrade
          ? getRouteTradeKey(selectedPatternTrade)
            : selectedFamilyPattern
              ? null
            : routeTrades[0]
              ? getRouteTradeKey(routeTrades[0])
              : null
      );
    }
  }, [routeTrades, selectedFamilyPattern, selectedPatternTrade, selectedRouteTradeKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadCanvasPreview = async () => {
      if (!selectedFamilyKey) {
        setCanvasChartData({ candles: [], rust_patterns: null });
        setCanvasPattern(null);
        setCanvasError('');
        setCanvasLoading(false);
        return;
      }

      try {
        setCanvasLoading(true);
        setCanvasError('');
        setCanvasChartData({ candles: [], rust_patterns: null });
        setCanvasPattern(null);

        const routeTradeForPattern =
          selectedPatternTrade &&
          (!selectedRouteTrade || getRouteTradeKey(selectedRouteTrade) !== getRouteTradeKey(selectedPatternTrade))
            ? selectedPatternTrade
            : selectedRouteTrade;

        let summary = routeTradeForPattern
          ? {
              ...routeTradeForPattern,
              prop_outcome_mode: 'phase1-family',
            }
          : selectedFamilyPattern
            ? {
                ...selectedFamilyPattern,
                prop_outcome_mode: 'phase1-family',
              }
          : null;

        if (!summary) {
          const result = await fetchPhase1FamilyPatterns(
            {
              familyKey: selectedFamilyKey,
              sourceScope,
              year: yearFilter === 'All' ? null : yearFilter,
            },
            { limit: 1, offset: 0, includeCount: false }
          );
          summary = result.patterns?.[0] ?? null;
        }

        if (!summary) {
          if (!isCancelled) {
            setCanvasError('No setup was found for this family.');
          }
          return;
        }

        const detail = await fetchPatternDetail(summary);
        if (!detail) {
          if (!isCancelled) {
            setCanvasError('Could not load the selected family setup.');
          }
          return;
        }
        const chartPattern = mergeRouteTradeIntoPattern(detail, routeTradeForPattern);
        if (!isCancelled) {
          setCanvasPattern(chartPattern);
        }

        const [candles, snrLines] = await Promise.all([
          chartPattern.symbol
            ? getCandles(chartPattern.symbol, buildPatternCandleWindow(chartPattern)).then(normalizeCandles)
            : Promise.resolve([]),
          getSupportResistanceLines(chartPattern.symbol),
        ]);

        if (isCancelled) {
          return;
        }

        formatPattern(
          clipCandlesAfterTradeExit(candles, chartPattern),
          chartPattern,
          snrLines,
          setCanvasChartData
        );
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setCanvasError('Could not load the family canvas.');
          setCanvasChartData({ candles: [], rust_patterns: null });
          setCanvasPattern(null);
        }
      } finally {
        if (!isCancelled) {
          setCanvasLoading(false);
        }
      }
    };

    void loadCanvasPreview();

    return () => {
      isCancelled = true;
    };
  }, [selectedFamilyKey, selectedFamilyPattern, selectedPatternTrade, selectedRouteTrade, sourceScope, yearFilter]);

  return (
    <div className="pattern-family-page pattern-family-terminal">
      <header className="pattern-family-terminal-bar">
        <div className="pattern-family-connection-strip">
          <span className="pattern-family-led pattern-family-led--online" />
          <strong>Connected</strong>
          <span>Realtime data</span>
        </div>
        <div className="pattern-family-terminal-title">
          <span>Monitor</span>
          <strong>Family Universe</strong>
        </div>
        <div className="pattern-family-terminal-clock">
          <span>{selectedSourceLabel}</span>
          <strong>{yearFilter === 'All' ? 'ALL YEARS' : yearFilter}</strong>
        </div>
      </header>

      <section className="pattern-family-command-deck">
        <div className="pattern-family-command-copy">
          <span>Pattern Families</span>
          <strong>{selectedFamilyLabel}</strong>
          <small>
            {formatNumber(families.length)} loaded groups / {formatNumber(visibleFamilies.length)} visible
          </small>
        </div>
        <div className="pattern-family-top-stats">
          {topStats.map((item) => (
            <div className="pattern-family-top-stat" key={item.label}>
              <span>{item.label}</span>
              <strong>{item.value}</strong>
            </div>
          ))}
        </div>
      </section>

      {error ? <div className="pattern-family-error">{error}</div> : null}

      <div className="pattern-family-workspace pattern-family-one-page">
        <main className="pattern-family-table-stack">
          <section className="pattern-family-controls">
            <label className="pattern-family-search-field">
              <span>Search</span>
              <input
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="family key, harmonic, bin, size..."
              />
            </label>
            <label>
              <span>Harmonic</span>
              <select value={harmonicType} onChange={(event) => setHarmonicType(event.target.value)}>
                {harmonicOptions.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>Source</span>
              <select
                value={sourceScope}
                onChange={(event) => {
                  setSourceScope(event.target.value);
                  setYearFilter('All');
                }}
              >
                {SOURCE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>Year</span>
              <select value={yearFilter} onChange={(event) => setYearFilter(event.target.value)}>
                <option value="All">All</option>
                {yearOptions.map((year) => (
                  <option key={year} value={year}>
                    {year}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>Min Setups</span>
              <input value={minSetups} onChange={(event) => setMinSetups(event.target.value)} />
            </label>
          </section>

          <section className="pattern-family-selected-columns">
            <SelectedSummaryRow
              actionLabel="Browse Families"
              emptyText="No pattern family selected."
              index={1}
              isLoading={isLoading}
              label="Family"
              loadingText="Loading pattern families..."
              metrics={selectedFamilyRowMetrics}
              onAction={() => setBrowsePanel('families')}
              status={`${formatNumber(visibleFamilies.length)} visible`}
              subtitle={selectedFamilyLabel}
              title={selectedFamily?.family_key ?? null}
              tone="family"
            />

            <SelectedSummaryRow
              actionDisabled={isFamilyPatternsLoading}
              actionLabel="Browse Patterns"
              emptyText={familyPatternsError || (selectedFamily ? 'No family patterns loaded.' : 'Select a family before choosing a pattern.')}
              index={2}
              isLoading={isFamilyPatternsLoading}
              label="Pattern"
              loadingText="Loading family patterns..."
              metrics={selectedPatternRowMetrics}
              onAction={() => setBrowsePanel('patterns')}
              status={familyPatternsError || `${formatNumber(familyPatterns.length)} patterns`}
              subtitle={
                selectedFamilyPattern
                  ? `${selectedFamilyPattern.market ?? 'Market N/A'} / ${selectedFamilyPattern.harmonic_type ?? selectedFamily?.harmonic_type ?? 'Harmonic N/A'}`
                  : 'Choose a setup to inspect'
              }
              title={selectedFamilyPattern?.pattern_id ?? selectedFamilyPattern?.pattern_group_id ?? null}
              tone="pattern"
            />

            <SelectedSummaryRow
              actionDisabled={!phase1Results.length}
              actionLabel="Browse Tests"
              emptyText={phase1Error || 'No stored Entry / Exit results for this family.'}
              index={3}
              isLoading={isPhase1Loading}
              label="Entry / Exit Test"
              loadingText="Loading Phase 1 results..."
              metrics={selectedRouteRowMetrics}
              onAction={() => setBrowsePanel('routes')}
              status={phase1Error || `${formatNumber(phase1Results.length)} tests`}
              subtitle={selectedRoute?.route_id ?? 'Select a family to load tests'}
              title={selectedRoute?.route_label ?? null}
              tone="route"
            />

            <SelectedSummaryRow
              actionDisabled={isRouteTradesLoading}
              actionLabel="Browse Trades"
              emptyText={patternRouteTradeError || (selectedRoute ? 'No trades loaded.' : 'Select a test to open its trades.')}
              index={4}
              isLoading={isPatternRouteTradeLoading && !selectedRouteTrade}
              label="Trade"
              loadingText="Replaying selected test on this pattern..."
              metrics={selectedTradeRowMetrics}
              onAction={() => setBrowsePanel('trades')}
              status={routeTradesError || patternRouteTradeError || `${formatNumber(routeTrades.length)} trades`}
              subtitle={
                selectedRouteTrade
                  ? `${formatTradeDirection(selectedRouteTrade)} / ${formatDate(selectedRouteTrade.entry_date)} to ${formatDate(selectedRouteTrade.target_date)}`
                  : 'Choose a stored trade'
              }
              title={selectedRouteTrade?.trade_uid ?? (selectedRouteTrade?.trade_id ? `#${selectedRouteTrade.trade_id}` : null)}
              tone={selectedTradeIsLoss ? 'loss' : selectedRouteTrade ? 'win' : 'trade'}
            />
          </section>

          <section className="pattern-family-sim-panel">
            <div className="pattern-family-sim-header">
              <div>
                <span>Data Center</span>
                <strong>
                  {selectedRoute ? 'Test Overview' : 'No test selected'}
                </strong>
              </div>
              <small>
                {selectedRoute ? selectedRoute.route_label : 'Select an Entry / Exit Test'}
              </small>
            </div>
            <div className="pattern-family-sim-body">
              {selectedRoute ? (
                <div className="pattern-family-test-overview">
                  <div className="pattern-family-test-overview-tabs" aria-label="Test overview sections">
                    {[
                      { id: 'overview', label: 'Overview' },
                      { id: 'supply', label: 'Supply' },
                      { id: 'family', label: 'Family' },
                      { id: 'families', label: 'Families' },
                      { id: 'symbols', label: 'Symbols' },
                      { id: 'outcomes', label: 'Outcomes' },
                      { id: 'simulator', label: 'Simulator', disabled: true },
                    ].map((tab) => (
                      <button
                        className={
                          testOverviewTab === tab.id
                            ? 'pattern-family-test-overview-tab pattern-family-test-overview-tab--active'
                            : 'pattern-family-test-overview-tab'
                        }
                        disabled={tab.disabled}
                        key={tab.id}
                        onClick={() => setTestOverviewTab(tab.id)}
                        type="button"
                      >
                        {tab.label}
                      </button>
                    ))}
                  </div>
                  <div
                    className={[
                      'pattern-family-test-overview-loading',
                      testOverviewTab === 'families' && routeFamiliesError ? 'pattern-family-test-overview-loading--error' : '',
                      testOverviewTab === 'supply' && supplyError ? 'pattern-family-test-overview-loading--error' : '',
                      (testOverviewTab === 'families' && (isRouteFamiliesLoading || routeFamiliesError)) ||
                      (testOverviewTab === 'supply' && (isSupplyLoading || supplyError))
                        ? ''
                        : 'pattern-family-test-overview-loading--empty',
                    ].filter(Boolean).join(' ')}
                  >
                    {testOverviewTab === 'supply' && supplyError
                      ? supplyError
                      : testOverviewTab === 'supply' && isSupplyLoading
                        ? 'Loading supply data...'
                        : testOverviewTab === 'families' && routeFamiliesError
                      ? routeFamiliesError
                      : testOverviewTab === 'families' && isRouteFamiliesLoading
                        ? 'Loading families for this test...'
                        : null}
                  </div>
                  <div className="pattern-family-test-overview-grid">
                    {activeTestOverviewSections.map((section) => (
                      <section className="pattern-family-test-overview-section" key={section.title}>
                        <header>
                          <span>{section.title}</span>
                        </header>
                        <div className="pattern-family-test-overview-cells">
                          {section.items.length ? (
                            section.items.map((item) => (
                              <div
                                className={[
                                  'pattern-family-test-overview-cell',
                                  item.wide ? 'pattern-family-test-overview-cell--wide' : '',
                                  item.tone ? `pattern-family-test-overview-cell--${item.tone}` : '',
                                ].filter(Boolean).join(' ')}
                                key={`${section.title}-${item.label}`}
                              >
                                <span>{item.label}</span>
                                <strong title={item.value}>{item.value}</strong>
                              </div>
                            ))
                          ) : (
                            <div className="pattern-family-test-overview-empty">
                              No trade breakdown data loaded yet.
                            </div>
                          )}
                        </div>
                      </section>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="pattern-family-sim-blank">
                  <span>Test Overview</span>
                  <strong>No Entry / Exit Test Selected</strong>
                  <small>Choose a test to load performance, coverage, and route logic stats here.</small>
                </div>
              )}
            </div>
          </section>
        </main>

        <aside className="pattern-family-inspector">
          <header className="pattern-family-inspector-head">
            <div className="pattern-family-inspector-title">
              <span>Trade Inspector</span>
              <strong>
                {canvasChartData.rust_patterns?.symbol ??
                  selectedFamily?.harmonic_type ??
                  'Family Preview'}
              </strong>
              <small>
                {selectedFamily
                  ? `${selectedFamily.harmonic_type} / ${selectedFamily.bin} / ${selectedFamily.size_bucket}`
                  : 'Select a family'}
              </small>
            </div>

            <div className="pattern-family-inspector-status">
              <span className={selectedTradeIsSkipped ? 'phase1-route-trade-skipped' : selectedTradeIsLoss ? 'pattern-family-inspector-loss' : 'pattern-family-inspector-win'}>
                {selectedRouteTrade ? (selectedTradeIsSkipped ? 'Skipped' : selectedTradeIsLoss ? 'Loss' : 'Win') : 'No Trade'}
              </span>
              <strong>{Number.isFinite(selectedTradeResultR) ? `${formatDecimal(selectedTradeResultR, 2)}R` : 'R N/A'}</strong>
            </div>

            <button
              className={showCanvasCandles ? 'pattern-family-inspector-toggle pattern-family-inspector-toggle--active' : 'pattern-family-inspector-toggle'}
              onClick={() => setShowCanvasCandles((current) => !current)}
              type="button"
            >
              {showCanvasCandles ? 'Candles On' : 'Candles Off'}
            </button>
          </header>

          <div className="pattern-family-inspector-body">
            <section className="pattern-family-trade-ticket">
              <div className="pattern-family-trade-ticket-head">
                <div className="pattern-family-trade-ticket-title">
                  <span>
                    {inspectorDetailMode === 'trade'
                      ? 'Current Trade'
                      : inspectorDetailMode === 'family'
                        ? 'Family Details'
                        : 'Entry / Exit Logic'}
                  </span>
                  <strong>
                    {inspectorDetailMode === 'trade'
                      ? selectedRouteTrade?.symbol ?? 'No trade'
                      : inspectorDetailMode === 'family'
                        ? selectedFamily?.harmonic_type ?? 'No family'
                        : selectedRoute ? `Route #${selectedRoute.result_rank}` : 'No route'}
                  </strong>
                  <small>
                    {inspectorDetailMode === 'trade'
                      ? selectedRouteTrade
                        ? `${formatDate(selectedRouteTrade.entry_date)} to ${formatDate(selectedRouteTrade.target_date)}`
                        : 'Select a trade'
                      : inspectorDetailMode === 'family'
                        ? selectedFamily?.family_key ?? 'Select a family'
                        : selectedRoute?.route_label ?? 'Select a route'}
                  </small>
                </div>

                <div className="pattern-family-ticket-toggle" role="tablist" aria-label="Inspector details">
                  <button
                    className={inspectorDetailMode === 'trade' ? 'pattern-family-ticket-toggle-button pattern-family-ticket-toggle-button--active' : 'pattern-family-ticket-toggle-button'}
                    onClick={() => setInspectorDetailMode('trade')}
                    type="button"
                  >
                    Trade
                  </button>
                  <button
                    className={inspectorDetailMode === 'family' ? 'pattern-family-ticket-toggle-button pattern-family-ticket-toggle-button--active' : 'pattern-family-ticket-toggle-button'}
                    onClick={() => setInspectorDetailMode('family')}
                    type="button"
                  >
                    Family
                  </button>
                  <button
                    className={inspectorDetailMode === 'logic' ? 'pattern-family-ticket-toggle-button pattern-family-ticket-toggle-button--active' : 'pattern-family-ticket-toggle-button'}
                    onClick={() => setInspectorDetailMode('logic')}
                    type="button"
                  >
                    Logic
                  </button>
                </div>
              </div>

              <div className="pattern-family-trade-ticket-grid">
                {(inspectorDetailMode === 'trade'
                  ? selectedTradeDetailStats
                  : inspectorDetailMode === 'family'
                    ? selectedFamilyDetailStats
                    : selectedRouteLogicStats
                ).map((item) => (
                  <div
                    className={[
                      'pattern-family-trade-ticket-cell',
                      item.wide ? 'pattern-family-trade-ticket-cell--wide' : '',
                      item.code ? 'pattern-family-trade-ticket-cell--code' : '',
                      item.tone ? `pattern-family-trade-ticket-cell--${item.tone}` : '',
                    ].filter(Boolean).join(' ')}
                    key={item.label}
                  >
                    <span>{item.label}</span>
                    <strong title={item.value}>{item.value}</strong>
                  </div>
                ))}
              </div>

              {selectedRoute ? (
                <div className="pattern-family-route-logic-panel">
                  <div className="pattern-family-route-logic-title">
                    <span>Route Logic</span>
                    <strong title={selectedRoute.route_label}>{selectedRoute.route_label}</strong>
                    <small title={selectedRoute.route_id}>{selectedRoute.route_id}</small>
                  </div>
                  <div className="pattern-family-route-logic-steps">
                    {selectedRouteOverlayDetails.map((item, index) => (
                      <div
                        className={[
                          'pattern-family-route-logic-step',
                          item.label === 'Entry' ? 'pattern-family-route-logic-step--entry' : '',
                          item.tone ? `pattern-family-route-logic-step--${item.tone}` : '',
                        ].filter(Boolean).join(' ')}
                        key={item.label}
                        onMouseEnter={() => setRouteLogicHover(item.label === 'Entry' ? 'entry' : null)}
                        onMouseLeave={() => setRouteLogicHover(null)}
                      >
                        <div className="pattern-family-route-logic-step-head">
                          <span>{String(index + 1).padStart(2, '0')}</span>
                          <b>{item.label}</b>
                        </div>
                        <strong>{item.action}</strong>
                        {item.meta ? <em>{item.meta}</em> : null}
                        <small>{item.value}</small>
                      </div>
                    ))}
                  </div>
                </div>
              ) : null}
            </section>

            <section className="pattern-family-chart-bay">
              {isCanvasLoading ? (
                <div className="pattern-family-inspector-empty">Loading family canvas...</div>
              ) : (
                <>
                  {canvasError ? <div className="pattern-family-inspector-error">{canvasError}</div> : null}
                  {canvasChartData.candles.length && canvasChartData.rust_patterns ? (
                    <div className="pattern-family-full-chart pattern-family-inspector-chart">
                      <CandleChartPanel
                        chartData={canvasChartData}
                        isSectionsExpanded={isCanvasExpanded}
                        setSectionsExpanded={setCanvasExpanded}
                        focusMode="prop"
                        market={canvasChartData.rust_patterns.market ?? selectedFamily?.market ?? 'Bullish'}
                        overlayTopOffset={0}
                        showCandles={showCanvasCandles}
                        presentationMode="graph"
                        routeLogicHover={routeLogicHover}
                      />
                    </div>
                  ) : canvasPattern ? (
                    <div className="pattern-family-inspector-empty">
                      Canvas is waiting for candle data so the XABCD lines can use the chart scale.
                    </div>
                  ) : (
                    <div className="pattern-family-inspector-empty">Select a family to preview its canvas.</div>
                  )}
                </>
              )}
            </section>
          </div>
        </aside>
      </div>

      {browsePanel ? (
        <div className="pattern-family-browse-overlay" role="presentation">
          <section className="pattern-family-browse-dialog" role="dialog" aria-modal="true">
            <header className="pattern-family-browse-head">
              <div>
                <span>Browse</span>
                <strong>
                  {browsePanel === 'families'
                    ? 'Pattern Families'
                    : browsePanel === 'routes'
                      ? 'Entry / Exit Tests'
                      : browsePanel === 'patterns'
                        ? 'Patterns'
                        : 'Trades'}
                </strong>
              </div>
              <button type="button" onClick={() => setBrowsePanel(null)}>
                Close
              </button>
            </header>

            <div className="pattern-family-browse-table">
              {browsePanel === 'families' ? (
                <>
                  <div className="pattern-family-row pattern-family-row--head">
                    <span>Family</span>
                    <span>Setups</span>
                    <span>Symbols</span>
                    <span>Harmonic</span>
                    <span>Bin</span>
                    <span>Size</span>
                    <span>Time</span>
                    <span>X</span>
                    <span>First D</span>
                    <span>Last D</span>
                  </div>
                  {visibleFamilies.length ? (
                    visibleFamilies.map((family) => (
                      <button
                        className={`pattern-family-row pattern-family-row--button${
                          family.family_key === selectedFamilyKey ? ' pattern-family-row--selected' : ''
                        }`}
                        key={family.family_key}
                        onClick={() => {
                          setSelectedFamilyKey(family.family_key);
                          setBrowsePanel(null);
                        }}
                        type="button"
                      >
                        <span className="pattern-family-id" title={family.family_key}>
                          {family.family_key}
                        </span>
                        <strong className="pattern-family-number">{formatNumber(family.setup_count)}</strong>
                        <span className="pattern-family-number">{formatNumber(family.symbol_count)}</span>
                        <span className="pattern-family-badge pattern-family-badge--harmonic">
                          {family.harmonic_type}
                        </span>
                        <span className="pattern-family-badge">{family.bin}</span>
                        <span className="pattern-family-badge">{family.size_bucket}</span>
                        <span className="pattern-family-badge">{family.time_bin}</span>
                        <span className="pattern-family-badge">{family.x_strictness}</span>
                        <span>{formatDate(family.first_d_date)}</span>
                        <span>{formatDate(family.last_d_date)}</span>
                      </button>
                    ))
                  ) : (
                    <div className="pattern-family-empty">No pattern families matched.</div>
                  )}
                </>
              ) : null}

              {browsePanel === 'routes' ? (
                <>
                  <div className="phase1-results-row phase1-results-row--head">
                    <span>Rank</span>
                    <span>Route</span>
                    <span>Avg R</span>
                    <span>Win</span>
                    <span>Trades</span>
                    <span>PF</span>
                    <span>DD</span>
                    <span>Worst Yr</span>
                    <span>Hold</span>
                    <span>Score</span>
                  </div>
                  {phase1Results.length ? (
                    phase1Results.map((result) => {
                      const routeKey = getRouteKey(result);

                      return (
                        <button
                          className={`phase1-results-row phase1-results-row--button${
                            routeKey === selectedRouteKey ? ' phase1-results-row--selected' : ''
                          }`}
                          key={routeKey}
                          onClick={() => {
                            setSelectedRouteKey(routeKey);
                            setBrowsePanel(null);
                          }}
                          type="button"
                        >
                          <strong>#{result.result_rank}</strong>
                          <span title={result.route_label}>{result.route_label}</span>
                          <strong>{formatDecimal(result.avg_r, 3)}</strong>
                          <span>{formatDecimal(result.win_rate, 1)}%</span>
                          <span>{formatNumber(result.trade_count)}</span>
                          <span>{formatDecimal(result.profit_factor, 2)}</span>
                          <span>{formatDecimal(result.max_drawdown_r, 2)}</span>
                          <span>{formatDecimal(result.worst_year_avg_r, 3)}</span>
                          <span>{result.max_hold_multiple}x</span>
                          <span>{formatDecimal(result.score, 1)}</span>
                        </button>
                      );
                    })
                  ) : (
                    <div className="pattern-family-empty">No stored Entry / Exit results for this family.</div>
                  )}
                </>
              ) : null}

              {browsePanel === 'patterns' ? (
                <>
                  <div className="pattern-family-pattern-row pattern-family-pattern-row--head">
                    <span>Pattern</span>
                    <span>Symbol</span>
                    <span>Market</span>
                    <span>Harmonic</span>
                    <span>D Date</span>
                    <span>Trade</span>
                  </div>
                  {familyPatterns.length ? (
                    familyPatterns.map((pattern) => {
                      const patternKey = getFamilyPatternKey(pattern);
                      const matchingTrade = routeTrades.find((trade) => patternMatchesTrade(pattern, trade));

                      return (
                        <button
                          className={`pattern-family-pattern-row pattern-family-pattern-row--button${
                            patternKey === selectedFamilyPatternKey ? ' pattern-family-pattern-row--selected' : ''
                          }`}
                          key={patternKey}
                          onClick={() => {
                            setSelectedFamilyPatternKey(patternKey);
                            if (matchingTrade) {
                              setSelectedRouteTradeKey(getRouteTradeKey(matchingTrade));
                            }
                            setBrowsePanel(null);
                          }}
                          type="button"
                        >
                          <span className="pattern-family-id" title={pattern.pattern_id ?? pattern.pattern_group_id ?? ''}>
                            {pattern.pattern_id ?? pattern.pattern_group_id ?? 'N/A'}
                          </span>
                          <strong>{pattern.symbol ?? 'N/A'}</strong>
                          <span>{pattern.market ?? 'N/A'}</span>
                          <span>{pattern.harmonic_type ?? selectedFamily?.harmonic_type ?? 'N/A'}</span>
                          <span>{formatDate(pattern.d_date)}</span>
                          <span>{matchingTrade ? `#${matchingTrade.trade_index ?? '-'}` : 'N/A'}</span>
                        </button>
                      );
                    })
                  ) : (
                    <div className="pattern-family-empty">No family patterns loaded.</div>
                  )}
                </>
              ) : null}

              {browsePanel === 'trades' ? (
                <>
                  <div className="phase1-route-trade-row phase1-route-trade-row--head">
                    <span>#</span>
                    <span>Symbol</span>
                    <span>Result</span>
                    <span>R</span>
                    <span>P/L</span>
                    <span>Entry</span>
                    <span>Exit</span>
                    <span>Pattern</span>
                  </div>
                  {routeTrades.length ? (
                    routeTrades.map((trade) => {
                      const tradeKey = getRouteTradeKey(trade);
                      const resultR = Number(trade.result_r);
                      const pnl = Number(trade.pnl);
                      const isSkipped = Boolean(trade.skipped_for_overlap);
                      const isLoss =
                        Number(trade.trade_result) === 2 ||
                        (Number.isFinite(resultR) && resultR < 0) ||
                        (Number.isFinite(pnl) && pnl < 0);

                      return (
                        <button
                          className={`phase1-route-trade-row phase1-route-trade-row--button${
                            tradeKey === selectedRouteTradeKey ? ' phase1-route-trade-row--selected' : ''
                          }`}
                          key={tradeKey}
                          onClick={() => {
                            setSelectedRouteTradeKey(tradeKey);
                            const matchingPattern = familyPatterns.find((pattern) => patternMatchesTrade(pattern, trade));
                            if (matchingPattern) {
                              setSelectedFamilyPatternKey(getFamilyPatternKey(matchingPattern));
                            }
                            setBrowsePanel(null);
                          }}
                          type="button"
                        >
                          <strong>{trade.trade_index ?? '-'}</strong>
                          <span>{trade.symbol ?? 'N/A'}</span>
                          <span className={isSkipped ? 'phase1-route-trade-skipped' : isLoss ? 'phase1-route-trade-loss' : 'phase1-route-trade-win'}>
                            {isSkipped ? 'Skipped' : isLoss ? 'Loss' : 'Win'}
                          </span>
                          <strong>{Number.isFinite(resultR) ? formatDecimal(resultR, 2) : 'N/A'}</strong>
                          <span>{Number.isFinite(pnl) ? formatMoney(pnl) : 'N/A'}</span>
                          <span>{formatDate(trade.entry_date)}</span>
                          <span>{formatDate(trade.target_date)}</span>
                          <span className="pattern-family-id" title={trade.pattern_id ?? trade.pattern_group_id ?? ''}>
                            {trade.pattern_id ?? trade.pattern_group_id ?? 'N/A'}
                          </span>
                        </button>
                      );
                    })
                  ) : (
                    <div className="pattern-family-empty">No trades loaded.</div>
                  )}
                </>
              ) : null}
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
};

export default PatternFamilyUniversePage;
