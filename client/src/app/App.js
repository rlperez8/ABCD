import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import PatternTable from '../components/PatternTable';
import Section from '../components/Section';
import CandleChartPanel from '../features/candle-chart/CandleChartPanel';
import StrategyLeaderboardCard, { rankStrategies } from '../features/strategies/StrategyLeaderboardCard';
import StrategyInsightCharts from '../features/strategies/StrategyInsightCharts';
import StrategyFrequencyPanel from '../features/strategies/StrategyFrequencyPanel';
import StrategyContractBreakdownPanel from '../features/strategies/StrategyContractBreakdownPanel';
import StrategyVariationPoolCard from '../features/strategies/StrategyVariationPoolCard';
import StrategyWorkbenchCard from '../features/strategies/StrategyWorkbenchCard';
import TradeSimulatorPanel from '../features/simulator/TradeSimulatorPanel';
import {
  fetchCurrentSetupStrategies,
  fetchCurrentSetups,
  fetchPatternDetail,
  fetchSetupComparison,
  fetchStrategyCandidates,
  fetchStrategyTrades,
  getCandles,
  getSupportResistanceLines,
} from '../services/patternApi';
import { formatPattern } from '../utils/patternFormatting';

const STRATEGY_TRADE_PAGE_SIZE = 50;

const ALL_PATTERNS_OPTION = 'All Patterns';
const ALL_BINS_OPTION = 'All Bins';
const STRATEGY_WORKSPACE_VIEW_CANVAS = 'canvas';
const STRATEGY_WORKSPACE_VIEW_GRAPHS = 'graphs';
const STRATEGY_WORKSPACE_VIEW_FREQUENCY = 'frequency';
const STRATEGY_WORKSPACE_VIEW_SIMULATOR = 'simulator';
const STRATEGY_LIBRARY_VIEW_MATCHED = 'matched-patterns';
const STRATEGY_LIBRARY_VIEW_CURRENT = 'current-setups';
const STRATEGY_MODE_PROP = 'prop';
const PROP_OUTCOME_MODE_REVERSAL = 'reversal';
const STRATEGY_ALL_MARKETS = 'All Markets';
const STRATEGY_ALL_SIZES = 'All Sizes';
const STRATEGY_ALL_TIME = 'All Time Bins';
const STRATEGY_DAYS_OPEN_OPTIONS = ['1', '3', '5', '7', '14', '30', 'Any'];
const BEST_PICK_DEFAULT_FILTERS = {
  minClosedTrades: '0',
  minExpectancy: '-999',
  maxDownYears: '99',
  minWorstYearExpectancy: '-999',
  minScore: '-999',
};
const STRATEGY_MARKET_OPTIONS = [STRATEGY_ALL_MARKETS, 'Bullish', 'Bearish'];
const STRATEGY_SIZE_OPTIONS = [STRATEGY_ALL_SIZES, 'Micro', 'Small', 'Normal', 'Large', 'Massive'];
const STRATEGY_MARKET_FEATURE_OPTIONS = STRATEGY_MARKET_OPTIONS.filter(
  (option) => option !== STRATEGY_ALL_MARKETS
);
const BIN_OPTIONS = [
  ALL_BINS_OPTION,
  '0-10',
  '10-20',
  '20-30',
  '30-40',
  '40-50',
  '50-60',
  '60-70',
  '70-80',
  '80-90',
  '90-100',
];
const HARMONIC_TYPE_OPTIONS = [
  ALL_PATTERNS_OPTION,
  'Bat',
  'AlternateBat',
  'Butterfly',
  'Gartley',
  'Crab',
  'DeepCrab',
  'Shark',
];
const STRATEGY_PATTERN_FEATURE_OPTIONS = HARMONIC_TYPE_OPTIONS.filter((option) => option !== ALL_PATTERNS_OPTION);
const STRATEGY_BIN_FEATURE_OPTIONS = BIN_OPTIONS.filter((option) => option !== ALL_BINS_OPTION);
const STRATEGY_SIZE_FEATURE_OPTIONS = STRATEGY_SIZE_OPTIONS.filter((option) => option !== STRATEGY_ALL_SIZES);
const STRATEGY_TIME_OPTIONS = [STRATEGY_ALL_TIME, ...STRATEGY_BIN_FEATURE_OPTIONS];
const STRATEGY_TIME_FEATURE_OPTIONS = STRATEGY_TIME_OPTIONS.filter((option) => option !== STRATEGY_ALL_TIME);
const STRATEGY_STRICTNESS_FEATURE_OPTIONS = ['Loose', 'Normal', 'Strict'];
const STRATEGY_FIT_BIN_OPTIONS = ['50-60', '60-70', '70-80', '80-90', '90-100'];
const STRATEGY_FIT_SIZE_OPTIONS = ['Micro', 'Small', 'Normal'];
const STRATEGY_FIT_STRICTNESS_OPTIONS = ['Strict'];
const STRATEGY_REVERSAL_FEATURE_OPTIONS = [
  'None',
  'BullishKeyReversal',
  'BearishKeyReversal',
  'BullishEngulfing',
  'BearishEngulfing',
  'BullishOutsideReversal',
  'BearishOutsideReversal',
  'MorningStar',
  'EveningStar',
  'ThreeWhiteSoldiers',
  'ThreeBlackCrows',
  'Hammer',
  'ShootingStar',
];
const STRATEGY_TREND_FEATURE_OPTIONS = ['Bullish', 'Bearish', 'Unknown'];
const DEFAULT_STRATEGY_SORT = {
  key: 'score',
  direction: 'desc',
};
const INITIAL_STRATEGY_FILTERS = {
  maxDaysOpen: '7',
  bestPickMinClosedTrades: BEST_PICK_DEFAULT_FILTERS.minClosedTrades,
  bestPickMinExpectancy: BEST_PICK_DEFAULT_FILTERS.minExpectancy,
  bestPickMaxDownYears: BEST_PICK_DEFAULT_FILTERS.maxDownYears,
  bestPickMinWorstYearExpectancy: BEST_PICK_DEFAULT_FILTERS.minWorstYearExpectancy,
  bestPickMinScore: BEST_PICK_DEFAULT_FILTERS.minScore,
};

const INITIAL_ACTIVE_FILTERS = {
  market: {
    active: false,
    filter: 'Both',
  },
  result: {
    active: false,
    filter: null,
  },
  retracement: {
    active: false,
    filter: null,
  },
};
const NEUTRAL_QUERY_FILTERS = INITIAL_ACTIVE_FILTERS;

const selectedServerOptions = (selectedOptions = [], allOptions = []) => {
  if (!Array.isArray(selectedOptions) || selectedOptions.length === 0) {
    return null;
  }

  if (selectedOptions.length >= allOptions.length) {
    return null;
  }

  return selectedOptions;
};

const HARMONIC_ACCURACY_FIELDS = [
  { label: 'Bat', key: 'bat_accuracy' },
  { label: 'Butterfly', key: 'butterfly_accuracy' },
  { label: 'Gartley', key: 'gartley_accuracy' },
  { label: 'Crab', key: 'crab_accuracy' },
  { label: 'Shark', key: 'shark_accuracy' },
];

const formatDateTimeForServer = (value) => {
  if (!value) return null;

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return null;
  }

  return parsed.toISOString().slice(0, 19).replace('T', ' ');
};

const formatCandleDateForChart = (value) => {
  if (!value) return null;

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return parsed.toISOString().slice(0, 19).replace('T', ' ');
};

const buildPatternCandleWindow = (pattern = {}) => {
  const dateValues = [
    pattern.x_date,
    pattern.a_date,
    pattern.b_date,
    pattern.c_date,
    pattern.d_date,
    pattern.d_confirm_date,
    pattern.reversal_detect_date,
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

  return {
    startDate: formatDateTimeForServer(new Date(Math.min(...dateValues) - paddingMs)),
    endDate: formatDateTimeForServer(new Date(Math.max(...dateValues) + paddingMs)),
  };
};

const normalizeCandles = (candles = []) =>
  candles
    .slice()
    .sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date))
    .map((item) => ({
      ...item,
      candle_date: formatCandleDateForChart(item.candle_date),
    }));

const handleCandles = async (symbol, options = {}) => {
  const candles = await getCandles(symbol, options);
  return normalizeCandles(candles);
};

const sortStrategyTrades = (patterns = []) =>
  [...patterns].sort((left, right) => {
    const leftDate = left?.reversal_detect_date ?? left?.d_date;
    const rightDate = right?.reversal_detect_date ?? right?.d_date;
    const leftTime = leftDate ? new Date(leftDate).getTime() : 0;
    const rightTime = rightDate ? new Date(rightDate).getTime() : 0;

    if (leftTime !== rightTime) {
      return rightTime - leftTime;
    }

    return String(left?.symbol ?? '').localeCompare(String(right?.symbol ?? ''));
  });

const strategyFeatureMatches = (selectedOptions = [], allOptions = [], value, fallback = null) => {
  const normalizedValue = value ?? fallback;
  if (normalizedValue === null || normalizedValue === undefined) {
    return selectedOptions.length === allOptions.length;
  }

  return selectedOptions.includes(normalizedValue);
};

const sameOptionSet = (left = [], right = []) =>
  left.length === right.length && right.every((option) => left.includes(option));

const parseNumericFilter = (value, fallback = 0) => {
  const parsed = Number.parseFloat(String(value ?? '').trim());
  return Number.isFinite(parsed) ? parsed : fallback;
};

const strategyPassesBestPickFilters = (strategy = {}, filters = INITIAL_STRATEGY_FILTERS) => {
  const summary = strategy?.comparison?.summary ?? {};
  const closedCount = Number(summary.closed_count ?? 0);
  const expectancy = Number(summary.expectancy ?? 0);
  const score = Number(strategy.score ?? 0);
  const worstYearExpectancy = Number(strategy.worstYearExpectancy ?? 0);
  const downYears = Number(strategy.downYears ?? 0);
  const minClosedTrades = parseNumericFilter(
    filters.bestPickMinClosedTrades,
    Number(BEST_PICK_DEFAULT_FILTERS.minClosedTrades)
  );
  const minExpectancy = parseNumericFilter(
    filters.bestPickMinExpectancy,
    Number(BEST_PICK_DEFAULT_FILTERS.minExpectancy)
  );
  const maxDownYears = parseNumericFilter(
    filters.bestPickMaxDownYears,
    Number(BEST_PICK_DEFAULT_FILTERS.maxDownYears)
  );
  const minWorstYearExpectancy = parseNumericFilter(
    filters.bestPickMinWorstYearExpectancy,
    Number(BEST_PICK_DEFAULT_FILTERS.minWorstYearExpectancy)
  );
  const minScore = parseNumericFilter(
    filters.bestPickMinScore,
    Number(BEST_PICK_DEFAULT_FILTERS.minScore)
  );

  return (
    closedCount >= minClosedTrades &&
    expectancy > minExpectancy &&
    downYears <= maxDownYears &&
    worstYearExpectancy > minWorstYearExpectancy &&
    score > minScore
  );
};

const updateSelectedPattern = async (
  selectedPattern,
  setChartData,
  { getPatternDetail, getCandlesForSymbol }
) => {
  const hydratedPatternPromise =
    selectedPattern?.x_date || (!selectedPattern?.pattern_id && !selectedPattern?.pattern_group_id)
      ? Promise.resolve(selectedPattern)
      : getPatternDetail(selectedPattern);
  const hydratedPattern = await hydratedPatternPromise;

  if (!hydratedPattern) {
    throw new Error('Pattern detail not found');
  }

  const mergedPattern = { ...hydratedPattern };
  [
    'prop_outcome_mode',
    'd_confirm_date',
    'reversal_detect_date',
    'target_date',
    'target_open',
    'target_high',
    'target_low',
    'target_close',
    'trade_enter_price',
    'trade_risk_exit_price',
    'trade_reward_exit_price',
  ].forEach((key) => {
    if (selectedPattern?.[key] !== null && selectedPattern?.[key] !== undefined) {
      mergedPattern[key] = selectedPattern[key];
    }
  });

  const [candles, snrLines] = await Promise.all([
    mergedPattern?.symbol
      ? getCandlesForSymbol(mergedPattern.symbol, buildPatternCandleWindow(mergedPattern))
      : Promise.resolve([]),
    getSupportResistanceLines(mergedPattern?.symbol),
  ]);
  formatPattern(candles, mergedPattern, snrLines, setChartData);
  return mergedPattern;
};

const getClosestPatternMatch = (pattern = {}) => {
  const bestMatch = HARMONIC_ACCURACY_FIELDS.reduce((currentBest, entry) => {
    const value = pattern?.[entry.key];

    if (!Number.isFinite(value)) {
      return currentBest;
    }

    if (!currentBest || value > currentBest.accuracy) {
      return { label: entry.label, accuracy: value };
    }

    return currentBest;
  }, null);

  const routeLabel =
    typeof pattern?.harmonic_type === 'string' && pattern.harmonic_type.trim()
      ? pattern.harmonic_type.trim()
      : null;

  return routeLabel && bestMatch ? { label: routeLabel, accuracy: bestMatch.accuracy } : bestMatch;
};

const accuracyToBin = (value) => {
  if (!Number.isFinite(value)) return null;
  if (value <= 10) return '0-10';
  if (value <= 20) return '10-20';
  if (value <= 30) return '20-30';
  if (value <= 40) return '30-40';
  if (value <= 50) return '40-50';
  if (value <= 60) return '50-60';
  if (value <= 70) return '60-70';
  if (value <= 80) return '70-80';
  if (value <= 90) return '80-90';
  return '90-100';
};

const buildStrategyId = ({
  market,
  harmonicType,
  bin,
  reversalType,
  sizeBucket,
  timeBin,
  xStrictness,
  threeMonthTrend,
  sixMonthTrend,
  twelveMonthTrend,
}) =>
  `${market}-${harmonicType}-${bin}-${reversalType}-${sizeBucket}-${timeBin}-${xStrictness}-${threeMonthTrend}-${sixMonthTrend}-${twelveMonthTrend}`
    .toLowerCase()
    .replace(/\s+/g, '-');

const buildEmptyStrategyComparison = () => ({
  summary: {
    total_count: 0,
    closed_count: 0,
    open_count: 0,
    win_count: 0,
    loss_count: 0,
    expectancy: 0,
    avg_return: 0,
    win_rate: 0,
    avg_win: 0,
    avg_loss: 0,
  },
  yearly_performance: [],
  recent_examples: [],
});

const getStrategyPerformanceSummary = (comparison = null) => {
  const yearlyPerformance = comparison?.yearly_performance ?? [];
  const validExpectancies = yearlyPerformance
    .map((entry) => Number(entry?.expectancy))
    .filter((value) => Number.isFinite(value));

  return {
    worstYearExpectancy: validExpectancies.length ? Math.min(...validExpectancies) : 0,
    downYears: validExpectancies.filter((value) => value <= 0).length,
  };
};

const buildStrategySnapshot = ({
  propStrategyId = null,
  familyKey = null,
  familyName = null,
  familyLevel = null,
  includedDimensions = null,
  outcomeModel = null,
  score = null,
  avgTargetRange = null,
  weeklyCadence = null,
  market,
  harmonicType,
  bin,
  reversalType = 'None',
  sizeBucket,
  timeBin,
  xStrictness = null,
  threeMonthTrend,
  sixMonthTrend,
  twelveMonthTrend,
  comparison = null,
  worstYearExpectancy = null,
  downYears = null,
  rank = 0,
}) => {
  const normalizedComparison = comparison ?? buildEmptyStrategyComparison();
  const performanceSummary = getStrategyPerformanceSummary(normalizedComparison);

  return {
    id:
      familyKey ??
      propStrategyId ??
      buildStrategyId({
        market,
        harmonicType,
        bin,
        reversalType,
        sizeBucket,
        timeBin,
        xStrictness,
        threeMonthTrend,
        sixMonthTrend,
        twelveMonthTrend,
      }),
    propStrategyId:
      familyKey ??
      propStrategyId ??
      buildStrategyId({
        market,
        harmonicType,
        bin,
        reversalType,
        sizeBucket,
        timeBin,
        xStrictness,
        threeMonthTrend,
        sixMonthTrend,
        twelveMonthTrend,
      }),
    familyKey,
    familyName,
    familyLevel,
    includedDimensions,
    outcomeModel,
    score,
    avgTargetRange,
    weeklyCadence: weeklyCadence ?? {
      totalCalendarWeeks: 0,
      activeWeeks: 0,
      zeroSetupWeeks: 0,
      zeroSetupWeekRate: 0,
      totalSetups: 0,
      avgSetupsPerWeek: 0,
      maxSetupsPerWeek: 0,
    },
    name: `${market} ${harmonicType} ${bin} ${reversalType} ${sizeBucket}`,
    description: `${reversalType} ${sizeBucket.toLowerCase()} ${String(xStrictness ?? 'Loose').toLowerCase()} setups in ${timeBin} time fit with ${threeMonthTrend}/${sixMonthTrend}/${twelveMonthTrend} trend.`,
    thesis: `A ${harmonicType} cohort in the ${bin} price bin with ${reversalType} reversal context, ${sizeBucket.toLowerCase()} structure size, ${String(xStrictness ?? 'Loose').toLowerCase()} X strictness, ${timeBin} dominant time fit, and ${threeMonthTrend}/${sixMonthTrend}/${twelveMonthTrend} 3M/6M/12M trend context for ${market.toLowerCase()} setups.`,
    harmonicType,
    market,
    bin,
    reversalType,
    sizeBucket,
    timeBin,
    xStrictness,
    threeMonthTrend,
    sixMonthTrend,
    twelveMonthTrend,
    worstYearExpectancy: worstYearExpectancy ?? performanceSummary.worstYearExpectancy,
    downYears: downYears ?? performanceSummary.downYears,
    comparison: normalizedComparison,
    rank,
  };
};

const getPatternStrategyDefinition = (pattern = {}) => {
  const match = getClosestPatternMatch(pattern);
  const bin = accuracyToBin(match?.accuracy);

  if (
    !pattern?.market ||
    !match?.label ||
    !bin ||
    !pattern?.size_bucket ||
    !pattern?.time_bin
  ) {
    return null;
  }

  return {
    market: pattern.market,
    harmonicType: match.label,
    bin,
    reversalType: pattern.reversal_type ?? 'None',
    sizeBucket: pattern.size_bucket,
    timeBin: pattern.time_bin,
    xStrictness: pattern.x_strictness ?? null,
    threeMonthTrend: pattern.three_month_trend ?? 'Unknown',
    sixMonthTrend: pattern.six_month_trend ?? 'Unknown',
    twelveMonthTrend: pattern.twelve_month_trend ?? 'Unknown',
  };
};

const getPatternStrategyId = (pattern = {}) => {
  if (pattern?.prop_strategy_id) {
    return pattern.prop_strategy_id;
  }

  const strategyDefinition = getPatternStrategyDefinition(pattern);

  return strategyDefinition ? buildStrategyId(strategyDefinition) : null;
};

const getPatternSelectionKey = (pattern = {}) => {
  if (!pattern) {
    return '';
  }

  if (pattern.pattern_id) {
    return `pattern:${pattern.pattern_id}`;
  }

  return [
    pattern.pattern_group_id ?? '',
    pattern.symbol ?? '',
    pattern.d_date ?? '',
    pattern.trade_enter_price ?? '',
    pattern.market ?? '',
  ].join('|');
};

const parseStrategyMaxDaysOpen = (value) => {
  if (!value || value === 'Any') {
    return null;
  }

  const parsed = Number.parseInt(String(value), 10);
  return Number.isFinite(parsed) ? parsed : null;
};

const App = () => {
  const [currentSetups, setCurrentSetups] = useState([]);
  const [isLoadingCurrentSetups, setLoadingCurrentSetups] = useState(false);
  const strategyMode = STRATEGY_MODE_PROP;
  const [propOutcomeMode] = useState(PROP_OUTCOME_MODE_REVERSAL);
  const [strategySnapshots, setStrategySnapshots] = useState([]);
  const [currentSetupStrategySnapshots, setCurrentSetupStrategySnapshots] = useState([]);
  const [isLoadingCurrentSetupStrategies, setLoadingCurrentSetupStrategies] = useState(false);
  const [isHydratingStrategy, setHydratingStrategy] = useState(false);
  const [selectedStrategyId, setSelectedStrategyId] = useState('');
  const [selectedStrategyComparison, setSelectedStrategyComparison] = useState(null);
  const [hoveredStrategyId, setHoveredStrategyId] = useState('');
  const [strategySortState, setStrategySortState] = useState(DEFAULT_STRATEGY_SORT);
  const [leaderChartStartIndex, setLeaderChartStartIndex] = useState(0);
  const [strategyTrades, setStrategyTrades] = useState([]);
  const [strategyTradeTotalCount, setStrategyTradeTotalCount] = useState(0);
  const [hasMoreStrategyTrades, setHasMoreStrategyTrades] = useState(false);
  const [isLoadingStrategyTrades, setLoadingStrategyTrades] = useState(false);
  const [isFetchingMoreStrategyTrades, setFetchingMoreStrategyTrades] = useState(false);
  const [selectedStrategyTradeIndex, setSelectedStrategyTradeIndex] = useState(0);
  const [selectedStrategyTradeKey, setSelectedStrategyTradeKey] = useState('');
  const [strategyChartData, setStrategyChartData] = useState({ candles: [], rust_patterns: null });
  const [isStrategyChartExpanded, setStrategyChartExpanded] = useState(true);
  const [, setLoadingStrategyChart] = useState(false);
  const [strategyFilters, setStrategyFilters] = useState(INITIAL_STRATEGY_FILTERS);
  const [selectedStrategyMarketFeatures, setSelectedStrategyMarketFeatures] = useState(
    STRATEGY_MARKET_FEATURE_OPTIONS
  );
  const [selectedStrategyPatternFeatures, setSelectedStrategyPatternFeatures] = useState(
    STRATEGY_PATTERN_FEATURE_OPTIONS
  );
  const [selectedStrategyBinFeatures, setSelectedStrategyBinFeatures] = useState(
    STRATEGY_BIN_FEATURE_OPTIONS
  );
  const [selectedStrategySizeFeatures, setSelectedStrategySizeFeatures] = useState(
    STRATEGY_SIZE_FEATURE_OPTIONS
  );
  const [selectedStrategyTimeFeatures, setSelectedStrategyTimeFeatures] = useState(
    STRATEGY_TIME_FEATURE_OPTIONS
  );
  const [selectedStrategyStrictnessFeatures, setSelectedStrategyStrictnessFeatures] = useState(
    STRATEGY_STRICTNESS_FEATURE_OPTIONS
  );
  const [selectedStrategyReversalFeatures, setSelectedStrategyReversalFeatures] = useState(
    STRATEGY_REVERSAL_FEATURE_OPTIONS
  );
  const [selectedStrategyThreeMonthTrendFeatures, setSelectedStrategyThreeMonthTrendFeatures] =
    useState(STRATEGY_TREND_FEATURE_OPTIONS);
  const [selectedStrategySixMonthTrendFeatures, setSelectedStrategySixMonthTrendFeatures] =
    useState(STRATEGY_TREND_FEATURE_OPTIONS);
  const [selectedStrategyTwelveMonthTrendFeatures, setSelectedStrategyTwelveMonthTrendFeatures] =
    useState(STRATEGY_TREND_FEATURE_OPTIONS);
  const [activeStrategyWorkspaceView, setActiveStrategyWorkspaceView] = useState(
    STRATEGY_WORKSPACE_VIEW_CANVAS
  );
  const [strategyLibraryPatternView, setStrategyLibraryPatternView] = useState(
    STRATEGY_LIBRARY_VIEW_MATCHED
  );
  const [selectedStrategyCurrentSetupIndex, setSelectedStrategyCurrentSetupIndex] = useState(-1);
  const [selectedCurrentSetupKey, setSelectedCurrentSetupKey] = useState('');
  const [chartOverlayTop, setChartOverlayTop] = useState(0);
  const latestStrategyTradesQueryKeyRef = useRef('');
  const requestedStrategyTradeOffsetsRef = useRef(new Set());
  const strategyChartRequestIdRef = useRef(0);
  const candleCacheRef = useRef(new Map());
  const candleRequestCacheRef = useRef(new Map());
  const patternDetailCacheRef = useRef(new Map());
  const patternDetailRequestCacheRef = useRef(new Map());
  const strategyComparisonCacheRef = useRef(new Map());
  const hydratedStrategyIdsRef = useRef(new Set());
  const headerRef = useRef(null);

  const stationActiveFilters = NEUTRAL_QUERY_FILTERS;
  const currentSetupFilters = useMemo(
    () => ({
      bin: null,
      harmonicType: null,
    }),
    []
  );
  const strategyServerFilters = useMemo(
    () => ({
      markets: selectedServerOptions(selectedStrategyMarketFeatures, STRATEGY_MARKET_FEATURE_OPTIONS),
      harmonicTypes: selectedServerOptions(
        selectedStrategyPatternFeatures,
        STRATEGY_PATTERN_FEATURE_OPTIONS
      ),
      bins: selectedServerOptions(selectedStrategyBinFeatures, STRATEGY_BIN_FEATURE_OPTIONS),
      sizeBuckets: selectedServerOptions(selectedStrategySizeFeatures, STRATEGY_SIZE_FEATURE_OPTIONS),
      timeBins: selectedServerOptions(selectedStrategyTimeFeatures, STRATEGY_TIME_FEATURE_OPTIONS),
      xStrictness: selectedServerOptions(
        selectedStrategyStrictnessFeatures,
        STRATEGY_STRICTNESS_FEATURE_OPTIONS
      ),
      reversalTypes: selectedServerOptions(
        selectedStrategyReversalFeatures,
        STRATEGY_REVERSAL_FEATURE_OPTIONS
      ),
      threeMonthTrends: selectedServerOptions(
        selectedStrategyThreeMonthTrendFeatures,
        STRATEGY_TREND_FEATURE_OPTIONS
      ),
      sixMonthTrends: selectedServerOptions(
        selectedStrategySixMonthTrendFeatures,
        STRATEGY_TREND_FEATURE_OPTIONS
      ),
      twelveMonthTrends: selectedServerOptions(
        selectedStrategyTwelveMonthTrendFeatures,
        STRATEGY_TREND_FEATURE_OPTIONS
      ),
    }),
    [
      selectedStrategyBinFeatures,
      selectedStrategyMarketFeatures,
      selectedStrategyPatternFeatures,
      selectedStrategyReversalFeatures,
      selectedStrategySizeFeatures,
      selectedStrategyStrictnessFeatures,
      selectedStrategySixMonthTrendFeatures,
      selectedStrategyThreeMonthTrendFeatures,
      selectedStrategyTimeFeatures,
      selectedStrategyTwelveMonthTrendFeatures,
    ]
  );
  const currentSetupsMaxDaysOpen = parseStrategyMaxDaysOpen(strategyFilters.maxDaysOpen);
  const isPropStrategyMode = strategyMode === STRATEGY_MODE_PROP;
  const isPropReversalMode = isPropStrategyMode && propOutcomeMode === PROP_OUTCOME_MODE_REVERSAL;
  const sortedStrategyCurrentSetups = useMemo(
    () => sortStrategyTrades(currentSetups),
    [currentSetups]
  );
  const filteredStrategyCurrentSetups = useMemo(() => {
    return sortedStrategyCurrentSetups.filter((setup) => {
      const strategyDefinition = getPatternStrategyDefinition(setup);

      if (!strategyDefinition) {
        return false;
      }

      const strategyId = getPatternStrategyId(setup);

      if (selectedStrategyId && strategyId !== selectedStrategyId) {
        return false;
      }

      if (!strategyFeatureMatches(selectedStrategyMarketFeatures, STRATEGY_MARKET_FEATURE_OPTIONS, strategyDefinition.market)) {
        return false;
      }

      if (!strategyFeatureMatches(selectedStrategyPatternFeatures, STRATEGY_PATTERN_FEATURE_OPTIONS, strategyDefinition.harmonicType)) {
        return false;
      }

      if (!strategyFeatureMatches(selectedStrategyBinFeatures, STRATEGY_BIN_FEATURE_OPTIONS, strategyDefinition.bin)) {
        return false;
      }

      if (!strategyFeatureMatches(selectedStrategySizeFeatures, STRATEGY_SIZE_FEATURE_OPTIONS, strategyDefinition.sizeBucket)) {
        return false;
      }

      if (!strategyFeatureMatches(selectedStrategyTimeFeatures, STRATEGY_TIME_FEATURE_OPTIONS, strategyDefinition.timeBin)) {
        return false;
      }

      if (!strategyFeatureMatches(selectedStrategyStrictnessFeatures, STRATEGY_STRICTNESS_FEATURE_OPTIONS, strategyDefinition.xStrictness)) {
        return false;
      }

      if (!strategyFeatureMatches(selectedStrategyReversalFeatures, STRATEGY_REVERSAL_FEATURE_OPTIONS, strategyDefinition.reversalType, 'None')) {
        return false;
      }

      if (!strategyFeatureMatches(selectedStrategyThreeMonthTrendFeatures, STRATEGY_TREND_FEATURE_OPTIONS, strategyDefinition.threeMonthTrend)) {
        return false;
      }

      if (!strategyFeatureMatches(selectedStrategySixMonthTrendFeatures, STRATEGY_TREND_FEATURE_OPTIONS, strategyDefinition.sixMonthTrend)) {
        return false;
      }

      if (
        !strategyFeatureMatches(selectedStrategyTwelveMonthTrendFeatures, STRATEGY_TREND_FEATURE_OPTIONS, strategyDefinition.twelveMonthTrend)
      ) {
        return false;
      }

      return true;
    });
  }, [
    selectedStrategyBinFeatures,
    selectedStrategyMarketFeatures,
    selectedStrategyPatternFeatures,
    selectedStrategyReversalFeatures,
    selectedStrategyId,
    selectedStrategyStrictnessFeatures,
    selectedStrategySizeFeatures,
    selectedStrategySixMonthTrendFeatures,
    selectedStrategyThreeMonthTrendFeatures,
    selectedStrategyTimeFeatures,
    selectedStrategyTwelveMonthTrendFeatures,
    sortedStrategyCurrentSetups,
  ]);
  const filterStrategyUniverse = useCallback(
    (strategies = []) => {
      return strategies.filter((strategy) => {
        if (!strategyFeatureMatches(selectedStrategyMarketFeatures, STRATEGY_MARKET_FEATURE_OPTIONS, strategy.market)) {
          return false;
        }

        if (!strategyFeatureMatches(selectedStrategyPatternFeatures, STRATEGY_PATTERN_FEATURE_OPTIONS, strategy.harmonicType)) {
          return false;
        }

        if (!strategyFeatureMatches(selectedStrategyBinFeatures, STRATEGY_BIN_FEATURE_OPTIONS, strategy.bin)) {
          return false;
        }

        if (!strategyFeatureMatches(selectedStrategySizeFeatures, STRATEGY_SIZE_FEATURE_OPTIONS, strategy.sizeBucket)) {
          return false;
        }

        if (!strategyFeatureMatches(selectedStrategyTimeFeatures, STRATEGY_TIME_FEATURE_OPTIONS, strategy.timeBin)) {
          return false;
        }

        if (!strategyFeatureMatches(selectedStrategyStrictnessFeatures, STRATEGY_STRICTNESS_FEATURE_OPTIONS, strategy.xStrictness)) {
          return false;
        }

        if (!strategyFeatureMatches(selectedStrategyReversalFeatures, STRATEGY_REVERSAL_FEATURE_OPTIONS, strategy.reversalType, 'None')) {
          return false;
        }

        if (!strategyFeatureMatches(selectedStrategyThreeMonthTrendFeatures, STRATEGY_TREND_FEATURE_OPTIONS, strategy.threeMonthTrend)) {
          return false;
        }

        if (!strategyFeatureMatches(selectedStrategySixMonthTrendFeatures, STRATEGY_TREND_FEATURE_OPTIONS, strategy.sixMonthTrend)) {
          return false;
        }

        if (!strategyFeatureMatches(selectedStrategyTwelveMonthTrendFeatures, STRATEGY_TREND_FEATURE_OPTIONS, strategy.twelveMonthTrend)) {
          return false;
        }

        if (!strategyPassesBestPickFilters(strategy, strategyFilters)) {
          return false;
        }

        return true;
      });
    },
    [
      selectedStrategyBinFeatures,
      selectedStrategyMarketFeatures,
      selectedStrategyPatternFeatures,
      selectedStrategyReversalFeatures,
      selectedStrategyStrictnessFeatures,
      selectedStrategySizeFeatures,
      selectedStrategySixMonthTrendFeatures,
      selectedStrategyThreeMonthTrendFeatures,
      selectedStrategyTimeFeatures,
      selectedStrategyTwelveMonthTrendFeatures,
      strategyFilters,
    ]
  );
  const filteredCurrentSetupStrategySnapshots = useMemo(
    () => filterStrategyUniverse(currentSetupStrategySnapshots),
    [currentSetupStrategySnapshots, filterStrategyUniverse]
  );
  const filteredStrategySnapshots = useMemo(
    () => filterStrategyUniverse(strategySnapshots),
    [strategySnapshots, filterStrategyUniverse]
  );
  const showingPropCurrentStrategies =
    strategyMode === STRATEGY_MODE_PROP &&
    strategyLibraryPatternView === STRATEGY_LIBRARY_VIEW_CURRENT;
  const strategyTableSnapshots = showingPropCurrentStrategies
    ? filteredCurrentSetupStrategySnapshots
    : filteredStrategySnapshots;
  const rankedStrategyTableSnapshots = useMemo(
    () => rankStrategies(strategyTableSnapshots, strategySortState),
    [strategySortState, strategyTableSnapshots]
  );

  useEffect(() => {
    setLeaderChartStartIndex(0);
  }, [showingPropCurrentStrategies, strategySortState]);
  const totalStrategyUniverseCount = showingPropCurrentStrategies
    ? currentSetupStrategySnapshots.length
    : strategySnapshots.length;
  const isLoadingStrategyUniverse =
    showingPropCurrentStrategies
      ? isLoadingCurrentSetupStrategies ||
        (strategyLibraryPatternView === STRATEGY_LIBRARY_VIEW_CURRENT && isLoadingCurrentSetups)
      : isLoadingCurrentSetupStrategies;
  const selectedStrategy = useMemo(
    () =>
      strategyTableSnapshots.find((strategy) => strategy.id === selectedStrategyId) ??
      strategyTableSnapshots[0] ??
      null,
    [selectedStrategyId, strategyTableSnapshots]
  );
  const selectedStrategyForInsights = useMemo(() => {
    if (!selectedStrategy) {
      return null;
    }

    if (selectedStrategyComparison?.strategyId !== selectedStrategy.id) {
      return selectedStrategy;
    }

    return {
      ...selectedStrategy,
      comparison: selectedStrategyComparison.comparison ?? selectedStrategy.comparison,
    };
  }, [selectedStrategy, selectedStrategyComparison]);
  useEffect(() => {
    if (!strategyTableSnapshots.length) {
      if (selectedStrategyId) {
        setSelectedStrategyId('');
      }
      return;
    }

    if (!selectedStrategyId) {
      setSelectedStrategyId(strategyTableSnapshots[0]?.id ?? '');
      return;
    }

    if (!strategyTableSnapshots.some((strategy) => strategy.id === selectedStrategyId)) {
      setSelectedStrategyId(strategyTableSnapshots[0]?.id ?? '');
    }
  }, [selectedStrategyId, strategyTableSnapshots]);
  const strategyTradesQueryKey = useMemo(
    () =>
      selectedStrategy
        ? JSON.stringify({
            strategyMode,
            propOutcomeMode,
            propStrategyId: selectedStrategy.propStrategyId ?? selectedStrategy.id,
            strategyId: selectedStrategy.id,
          })
        : '',
    [propOutcomeMode, selectedStrategy, strategyMode]
  );

  const handleSelectStrategyCurrentSetup = useCallback(
    (setup, rowIndex) => {
      setSelectedStrategyCurrentSetupIndex(rowIndex);
      setSelectedCurrentSetupKey(getPatternSelectionKey(setup));

      const setupStrategyId = getPatternStrategyId(setup);

      if (!setupStrategyId) {
        return;
      }

      setSelectedStrategyId(setupStrategyId);
      setActiveStrategyWorkspaceView(STRATEGY_WORKSPACE_VIEW_CANVAS);
    },
    []
  );

  const handleSelectStrategyTrade = useCallback((trade, rowIndex) => {
    setSelectedStrategyTradeIndex(rowIndex);
    setSelectedStrategyTradeKey(getPatternSelectionKey(trade));
    setActiveStrategyWorkspaceView(STRATEGY_WORKSPACE_VIEW_CANVAS);
  }, []);

  const handleSelectStrategy = useCallback(
    (strategyId) => {
      if (!strategyId) {
        return;
      }

      if (strategyId === selectedStrategyId) {
        return;
      }

      setSelectedStrategyId(strategyId);
      setSelectedStrategyComparison(null);
      setStrategyTrades([]);
      setStrategyTradeTotalCount(0);
      setHasMoreStrategyTrades(false);
      setSelectedStrategyTradeIndex(0);
      setSelectedStrategyTradeKey('');
      setStrategyChartData({ candles: [], rust_patterns: null });
    },
    [selectedStrategyId]
  );

  const handleSelectStrategyFromChart = useCallback(
    (strategyId) => {
      if (!strategyId || strategyId === selectedStrategyId) {
        return;
      }

      setSelectedStrategyId(strategyId);
      setSelectedStrategyComparison(null);
      setStrategyTrades([]);
      setStrategyTradeTotalCount(0);
      setHasMoreStrategyTrades(false);
      setSelectedStrategyTradeIndex(0);
      setSelectedStrategyTradeKey('');
      setStrategyChartData({ candles: [], rust_patterns: null });
    },
    [selectedStrategyId]
  );

  useEffect(() => {
    strategyComparisonCacheRef.current = new Map();
    hydratedStrategyIdsRef.current = new Set();
    setSelectedStrategyId('');
    setSelectedStrategyComparison(null);
    setStrategyTrades([]);
    setStrategyTradeTotalCount(0);
    setHasMoreStrategyTrades(false);
    setSelectedStrategyTradeIndex(0);
    setSelectedStrategyTradeKey('');
    setCurrentSetups([]);
    setSelectedStrategyCurrentSetupIndex(-1);
    setSelectedCurrentSetupKey('');
    setStrategyChartData({ candles: [], rust_patterns: null });

  }, [propOutcomeMode, strategyMode]);

  const getCandlesForSymbol = useCallback(async (symbol, options = {}) => {
    if (!symbol) {
      return [];
    }

    const cacheKey = [
      symbol,
      options?.startDate ?? 'start',
      options?.endDate ?? 'end',
    ].join('|');

    const cachedCandles = candleCacheRef.current.get(cacheKey);
    if (cachedCandles) {
      return cachedCandles;
    }

    const pendingRequest = candleRequestCacheRef.current.get(cacheKey);
    if (pendingRequest) {
      return pendingRequest;
    }

    const request = handleCandles(symbol, options)
      .then((candles) => {
        candleCacheRef.current.set(cacheKey, candles);
        return candles;
      })
      .finally(() => {
        candleRequestCacheRef.current.delete(cacheKey);
      });

    candleRequestCacheRef.current.set(cacheKey, request);
    return request;
  }, []);

  const getPatternDetail = useCallback(async (pattern) => {
    if (!pattern) {
      return null;
    }

    if (pattern?.x_date || (!pattern?.pattern_id && !pattern?.pattern_group_id)) {
      return pattern;
    }

    const cacheKey = [
      pattern.pattern_id ?? 'unknown-pattern-id',
      pattern.pattern_group_id,
      pattern.d_date ?? 'unknown-date',
      pattern.market ?? 'unknown-market',
      pattern.harmonic_type ?? 'unknown-harmonic',
      pattern.size_bucket ?? 'unknown-size',
      pattern.balance_bucket ?? 'unknown-balance',
      pattern.trade_enter_price ?? 'unknown-enter',
      pattern.trade_risk_exit_price ?? 'unknown-risk',
      pattern.trade_reward_exit_price ?? 'unknown-reward',
      pattern.x_length ?? 'unknown-x-length',
      pattern.a_length ?? 'unknown-a-length',
      pattern.b_length ?? 'unknown-b-length',
      pattern.c_length ?? 'unknown-c-length',
      pattern.d_length ?? 'unknown-d-length',
    ].join('|');

    const cachedPattern = patternDetailCacheRef.current.get(cacheKey);
    if (cachedPattern) {
      return cachedPattern;
    }

    const pendingRequest = patternDetailRequestCacheRef.current.get(cacheKey);
    if (pendingRequest) {
      return pendingRequest;
    }

    const request = fetchPatternDetail(pattern)
      .then((hydratedPattern) => {
        if (hydratedPattern) {
          patternDetailCacheRef.current.set(cacheKey, hydratedPattern);
        }

        return hydratedPattern;
      })
      .finally(() => {
        patternDetailRequestCacheRef.current.delete(cacheKey);
      });

    patternDetailRequestCacheRef.current.set(cacheKey, request);
    return request;
  }, []);

  const updateStrategyPatternForChart = useCallback(
    async (selectedPattern, setChartDataValue = setStrategyChartData) => {
      const requestId = strategyChartRequestIdRef.current + 1;
      strategyChartRequestIdRef.current = requestId;

      return updateSelectedPattern(
        selectedPattern,
        (nextChartData) => {
          if (strategyChartRequestIdRef.current === requestId) {
            setChartDataValue(nextChartData);
          }
        },
        {
          getPatternDetail,
          getCandlesForSymbol,
        }
      );
    },
    [getCandlesForSymbol, getPatternDetail]
  );

  useEffect(() => {
    const updateChartOverlayTop = () => {
      if (!headerRef.current) {
        setChartOverlayTop(0);
        return;
      }

      setChartOverlayTop(Math.max(0, Math.round(headerRef.current.getBoundingClientRect().bottom + 8)));
    };

    updateChartOverlayTop();
    window.addEventListener('resize', updateChartOverlayTop);

    return () => {
      window.removeEventListener('resize', updateChartOverlayTop);
    };
  }, []);

  useEffect(() => {
    let isCancelled = false;

    const loadCurrentSetupStrategies = async () => {
      try {
        setLoadingCurrentSetupStrategies(true);
        const currentStrategies =
          showingPropCurrentStrategies
            ? await fetchCurrentSetupStrategies(currentSetupFilters, stationActiveFilters, {
                maxDaysOpen: currentSetupsMaxDaysOpen,
                propOutcomeMode,
                sort: strategySortState,
                familyFilters: strategyServerFilters,
                bestPickFilters: strategyFilters,
              })
            : await fetchStrategyCandidates({
                minClosedTrades:
                  Number.parseInt(String(strategyFilters.bestPickMinClosedTrades), 10) || 0,
                limit: 500,
                propMode: strategyMode === STRATEGY_MODE_PROP,
                propOutcomeMode,
                sort: strategySortState,
                familyFilters: strategyServerFilters,
                bestPickFilters: strategyFilters,
              });

        if (isCancelled) {
          return;
        }

        const nextComparisonCache = new Map(strategyComparisonCacheRef.current);
        const nextSnapshots = (currentStrategies ?? []).map((strategy, index) => {
          const strategyDefinition = {
            propStrategyId: strategy.family_key ?? strategy.prop_strategy_id ?? null,
            familyKey: strategy.family_key ?? strategy.prop_strategy_id ?? null,
            familyName: strategy.family_name ?? null,
            familyLevel: strategy.family_level ?? null,
            includedDimensions: strategy.included_dimensions ?? null,
            outcomeModel: strategy.outcome_model ?? null,
            score: strategy.score ?? null,
            avgTargetRange: strategy.avg_target_range ?? null,
            weeklyCadence: strategy.weeklyCadence ?? null,
            market: strategy.market,
            harmonicType: strategy.harmonic_type,
            bin: strategy.bin,
            reversalType: strategy.reversal_type,
            sizeBucket: strategy.size_bucket,
            timeBin: strategy.time_bin,
            xStrictness: strategy.x_strictness ?? null,
            threeMonthTrend: strategy.three_month_trend,
            sixMonthTrend: strategy.six_month_trend,
            twelveMonthTrend: strategy.twelve_month_trend,
          };
          const strategyId =
            strategyDefinition.familyKey ??
            strategyDefinition.propStrategyId ??
            buildStrategyId(strategyDefinition);
          const cachedComparison = strategyComparisonCacheRef.current.get(strategyId);
          const cachedYearlyPerformance = Array.isArray(cachedComparison?.yearly_performance)
            ? cachedComparison.yearly_performance
            : [];
          const comparison = {
            summary: {
              total_count: strategy.total_count,
              closed_count: strategy.closed_count,
              open_count: strategy.open_count,
              win_count: strategy.win_count,
              loss_count: strategy.loss_count,
              expectancy: strategy.expectancy,
              avg_return: strategy.avg_return,
              win_rate: strategy.win_rate,
              avg_win: strategy.avg_win,
              avg_loss: strategy.avg_loss,
            },
            yearly_performance: cachedYearlyPerformance,
            recent_examples: cachedComparison?.recent_examples ?? [],
          };

          if (!cachedYearlyPerformance.length) {
            hydratedStrategyIdsRef.current.delete(strategyId);
          }
          nextComparisonCache.set(strategyId, comparison);

          return buildStrategySnapshot({
            ...strategyDefinition,
            comparison,
            worstYearExpectancy: strategy.worst_year_expectancy,
            downYears: strategy.down_years,
            rank: index + 1,
          });
        });

        strategyComparisonCacheRef.current = nextComparisonCache;

        if (!isCancelled) {
          if (showingPropCurrentStrategies) {
            setCurrentSetupStrategySnapshots(nextSnapshots);
          } else {
            setStrategySnapshots(nextSnapshots);
          }
          setSelectedStrategyId((currentSelected) => {
            if (nextSnapshots.some((strategy) => strategy.id === currentSelected)) {
              return currentSelected;
            }

            return nextSnapshots[0]?.id ?? '';
          });
        }
      } finally {
        if (!isCancelled) {
          setLoadingCurrentSetupStrategies(false);
        }
      }
    };

    void loadCurrentSetupStrategies();

    return () => {
      isCancelled = true;
    };
  }, [
    currentSetupsMaxDaysOpen,
    currentSetupFilters,
    propOutcomeMode,
    showingPropCurrentStrategies,
    stationActiveFilters,
    strategyServerFilters,
    strategyFilters,
    strategyFilters.bestPickMinClosedTrades,
    strategyMode,
    strategyLibraryPatternView,
    strategySortState,
  ]);

  useEffect(() => {
    setSelectedStrategyId((currentSelected) => {
      if (strategyTableSnapshots.some((strategy) => strategy.id === currentSelected)) {
        return currentSelected;
      }

      return strategyTableSnapshots[0]?.id ?? '';
    });
  }, [strategyTableSnapshots]);

  useEffect(() => {
    if (!selectedStrategy) {
      setHydratingStrategy(false);
      setSelectedStrategyComparison(null);
      return undefined;
    }

    const cachedComparison = strategyComparisonCacheRef.current.get(selectedStrategy.id);
    const isHydrated = hydratedStrategyIdsRef.current.has(selectedStrategy.id);
    const updateSnapshots = showingPropCurrentStrategies
      ? setCurrentSetupStrategySnapshots
      : setStrategySnapshots;

    if (isHydrated && cachedComparison) {
      setSelectedStrategyComparison({
        strategyId: selectedStrategy.id,
        comparison: cachedComparison,
      });

      if (cachedComparison !== selectedStrategy.comparison) {
        updateSnapshots((currentStrategies) =>
          currentStrategies.map((strategy) =>
            strategy.id === selectedStrategy.id
              ? {
                  ...strategy,
                  comparison: cachedComparison,
                }
              : strategy
          )
        );
      }

      return undefined;
    }

    let isCancelled = false;

    const hydrateSelectedStrategy = async () => {
      try {
        setHydratingStrategy(true);
        const comparison = await fetchSetupComparison({
          propStrategyId: selectedStrategy.propStrategyId ?? selectedStrategy.id,
          harmonicType: selectedStrategy.harmonicType,
          market: selectedStrategy.market,
          bin: selectedStrategy.bin,
          reversalType: selectedStrategy.reversalType,
          sizeBucket: selectedStrategy.sizeBucket,
          timeBin: selectedStrategy.timeBin,
          threeMonthTrend: selectedStrategy.threeMonthTrend,
          sixMonthTrend: selectedStrategy.sixMonthTrend,
          twelveMonthTrend: selectedStrategy.twelveMonthTrend,
          includeExamples: false,
          propMode: strategyMode === STRATEGY_MODE_PROP,
          propOutcomeMode,
        });

        if (isCancelled) {
          return;
        }

        if (comparison) {
          strategyComparisonCacheRef.current.set(selectedStrategy.id, comparison);
          hydratedStrategyIdsRef.current.add(selectedStrategy.id);
          setSelectedStrategyComparison({
            strategyId: selectedStrategy.id,
            comparison,
          });
        }

        if (comparison) {
          updateSnapshots((currentStrategies) =>
            currentStrategies.map((strategy) =>
              strategy.id === selectedStrategy.id
                ? {
                    ...strategy,
                    comparison,
                  }
                : strategy
            )
          );
        }
      } finally {
        if (!isCancelled) {
          setHydratingStrategy(false);
        }
      }
    };

    hydrateSelectedStrategy();

    return () => {
      isCancelled = true;
    };
  }, [propOutcomeMode, selectedStrategy, showingPropCurrentStrategies, strategyMode]);

  useEffect(() => {
    if (!selectedStrategy) {
      strategyChartRequestIdRef.current += 1;
      latestStrategyTradesQueryKeyRef.current = '';
      requestedStrategyTradeOffsetsRef.current = new Set();
      setStrategyTrades([]);
      setStrategyTradeTotalCount(0);
      setHasMoreStrategyTrades(false);
      setSelectedStrategyTradeIndex(0);
      setSelectedStrategyTradeKey('');
      setStrategyChartData({ candles: [], rust_patterns: null });
      setLoadingStrategyTrades(false);
      setFetchingMoreStrategyTrades(false);
      return undefined;
    }

    let isCancelled = false;
    latestStrategyTradesQueryKeyRef.current = strategyTradesQueryKey;
    requestedStrategyTradeOffsetsRef.current = new Set([0]);

    const loadTrades = async () => {
      try {
        setLoadingStrategyTrades(true);
        setFetchingMoreStrategyTrades(false);
        const data = await fetchStrategyTrades(selectedStrategy, {
          limit: STRATEGY_TRADE_PAGE_SIZE,
          offset: 0,
          includeCount: false,
        }, {
          propMode: strategyMode === STRATEGY_MODE_PROP,
          propOutcomeMode,
        });

        if (isCancelled || latestStrategyTradesQueryKeyRef.current !== strategyTradesQueryKey) {
          return;
        }

        const nextTrades = sortStrategyTrades(data?.patterns ?? []);
        setStrategyTrades(nextTrades);
        setStrategyTradeTotalCount(
          Number.isFinite(data?.total_count) && data.total_count >= 0 ? data.total_count : -1
        );
        setHasMoreStrategyTrades(Boolean(data?.has_more));

        if (
          strategyMode === STRATEGY_MODE_PROP &&
          strategyLibraryPatternView === STRATEGY_LIBRARY_VIEW_CURRENT
        ) {
          return;
        }

        if (!nextTrades.length) {
          strategyChartRequestIdRef.current += 1;
          setSelectedStrategyTradeKey('');
          setSelectedStrategyTradeIndex(0);
          setStrategyChartData({ candles: [], rust_patterns: null });
          return;
        }
      } catch (error) {
        console.error('Strategy trades load failed:', error);
        if (!isCancelled) {
          strategyChartRequestIdRef.current += 1;
          setStrategyTrades([]);
          setStrategyTradeTotalCount(0);
          setHasMoreStrategyTrades(false);
          setSelectedStrategyTradeKey('');
          setSelectedStrategyTradeIndex(0);
          setStrategyChartData({ candles: [], rust_patterns: null });
        }
      } finally {
        if (!isCancelled) {
          setLoadingStrategyTrades(false);
        }
      }
    };

    void loadTrades();

    return () => {
      isCancelled = true;
    };
  }, [
    propOutcomeMode,
    selectedStrategy,
    strategyMode,
    strategyLibraryPatternView,
    strategyTradesQueryKey,
    updateStrategyPatternForChart,
  ]);

  useEffect(() => {
    if (
      !selectedStrategy ||
      (strategyMode === STRATEGY_MODE_PROP &&
        strategyLibraryPatternView === STRATEGY_LIBRARY_VIEW_CURRENT)
    ) {
      return undefined;
    }

    if (!strategyTrades.length) {
      strategyChartRequestIdRef.current += 1;
      setSelectedStrategyTradeIndex(0);
      setSelectedStrategyTradeKey('');
      setStrategyChartData({ candles: [], rust_patterns: null });
      return undefined;
    }

    let isCancelled = false;

    const loadFocusedStrategyTrade = async () => {
      const matchingIndex = selectedStrategyTradeKey
        ? strategyTrades.findIndex(
            (trade) => getPatternSelectionKey(trade) === selectedStrategyTradeKey
          )
        : -1;
      const nextIndex = matchingIndex >= 0 ? matchingIndex : 0;
      const nextTrade = strategyTrades[nextIndex];

      if (!nextTrade) {
        return;
      }

      try {
        setLoadingStrategyChart(true);
        setSelectedStrategyTradeIndex(nextIndex);
        setSelectedStrategyTradeKey(getPatternSelectionKey(nextTrade));
        await updateStrategyPatternForChart(nextTrade, (nextChartData) => {
          if (!isCancelled) {
            setStrategyChartData(nextChartData);
          }
        });
      } catch (error) {
        console.error('Strategy trade chart load failed:', error);
        if (!isCancelled) {
          setStrategyChartData({ candles: [], rust_patterns: null });
        }
      } finally {
        if (!isCancelled) {
          setLoadingStrategyChart(false);
        }
      }
    };

    void loadFocusedStrategyTrade();

    return () => {
      isCancelled = true;
    };
  }, [
    selectedStrategy,
    selectedStrategyTradeKey,
    strategyLibraryPatternView,
    strategyMode,
    strategyTrades,
    updateStrategyPatternForChart,
  ]);

  useEffect(() => {
    if (
      strategyMode !== STRATEGY_MODE_PROP ||
      strategyLibraryPatternView !== STRATEGY_LIBRARY_VIEW_CURRENT
    ) {
      setLoadingCurrentSetups(false);
      return undefined;
    }

    let isCancelled = false;

    const loadCurrentSetups = async () => {
      try {
        setLoadingCurrentSetups(true);
        const data = await fetchCurrentSetups(currentSetupFilters, stationActiveFilters, {
          limit: 1000,
          maxDaysOpen: currentSetupsMaxDaysOpen,
          propOutcomeMode,
        });

        if (isCancelled) {
          return;
        }

        setCurrentSetups(data?.patterns ?? []);
        setSelectedStrategyCurrentSetupIndex(-1);
      } catch (error) {
        console.error('Current setups load failed:', error);
        if (!isCancelled) {
          setCurrentSetups([]);
        }
      } finally {
        if (!isCancelled) {
          setLoadingCurrentSetups(false);
        }
      }
    };

    loadCurrentSetups();

    return () => {
      isCancelled = true;
    };
  }, [
    currentSetupsMaxDaysOpen,
    stationActiveFilters,
    currentSetupFilters,
    propOutcomeMode,
    strategyMode,
    strategyLibraryPatternView,
  ]);

  useEffect(() => {
    if (
      strategyMode !== STRATEGY_MODE_PROP ||
      strategyLibraryPatternView !== STRATEGY_LIBRARY_VIEW_CURRENT ||
      !selectedStrategy
    ) {
      return undefined;
    }

    if (!filteredStrategyCurrentSetups.length) {
      strategyChartRequestIdRef.current += 1;
      setSelectedStrategyCurrentSetupIndex(-1);
      setSelectedCurrentSetupKey('');
      setStrategyChartData({ candles: [], rust_patterns: null });
      return undefined;
    }

    let isCancelled = false;

    const loadFocusedCurrentSetup = async () => {
      const matchingIndex = selectedCurrentSetupKey
        ? filteredStrategyCurrentSetups.findIndex(
            (setup) => getPatternSelectionKey(setup) === selectedCurrentSetupKey
          )
        : -1;
      const nextIndex = matchingIndex >= 0 ? matchingIndex : 0;
      const nextSetup = filteredStrategyCurrentSetups[nextIndex];

      if (!nextSetup) {
        return;
      }

      try {
        setLoadingStrategyChart(true);
        setSelectedStrategyCurrentSetupIndex(nextIndex);
        setSelectedCurrentSetupKey(getPatternSelectionKey(nextSetup));
        await updateStrategyPatternForChart(nextSetup, (nextChartData) => {
          if (!isCancelled) {
            setStrategyChartData(nextChartData);
          }
        });
      } finally {
        if (!isCancelled) {
          setLoadingStrategyChart(false);
        }
      }
    };

    void loadFocusedCurrentSetup();

    return () => {
      isCancelled = true;
    };
  }, [
    filteredStrategyCurrentSetups,
    propOutcomeMode,
    selectedCurrentSetupKey,
    selectedStrategy,
    strategyMode,
    strategyLibraryPatternView,
    updateStrategyPatternForChart,
  ]);

  const loadMoreStrategyTrades = async () => {
    if (
      !selectedStrategy ||
      isLoadingStrategyTrades ||
      isFetchingMoreStrategyTrades ||
      !hasMoreStrategyTrades ||
      !strategyTrades.length
    ) {
      return;
    }

    const requestKey = strategyTradesQueryKey;
    const nextOffset = strategyTrades.length;

    if (requestedStrategyTradeOffsetsRef.current.has(nextOffset)) {
      return;
    }

    latestStrategyTradesQueryKeyRef.current = requestKey;
    requestedStrategyTradeOffsetsRef.current.add(nextOffset);
    setFetchingMoreStrategyTrades(true);

    try {
      const data = await fetchStrategyTrades(selectedStrategy, {
        limit: STRATEGY_TRADE_PAGE_SIZE,
        offset: nextOffset,
        includeCount: false,
      }, {
        propMode: true,
        propOutcomeMode,
      });

      if (latestStrategyTradesQueryKeyRef.current !== requestKey) {
        return;
      }

      setStrategyTrades((prev) => sortStrategyTrades([...prev, ...(data?.patterns ?? [])]));
      setStrategyTradeTotalCount((prev) =>
        Number.isFinite(data?.total_count) && data.total_count >= 0 ? data.total_count : prev
      );
      setHasMoreStrategyTrades(Boolean(data?.has_more));
    } catch (error) {
      requestedStrategyTradeOffsetsRef.current.delete(nextOffset);
      console.error('Strategy trade pagination failed:', error);
    } finally {
      if (latestStrategyTradesQueryKeyRef.current === requestKey) {
        setFetchingMoreStrategyTrades(false);
      }
    }
  };

  const updateStrategyFilter = (key, value) => {
    setStrategyFilters((prev) => ({
      ...prev,
      [key]: value,
    }));
  };

  const toggleStrategyFeature = useCallback((setState, options, value) => {
    setState((current) => {
      if (current.includes(value)) {
        if (current.length === 1) {
          return current;
        }

        return current.filter((entry) => entry !== value);
      }

      return options.filter((entry) => current.includes(entry) || entry === value);
    });
  }, []);

  const toggleStrategyTimeFeature = useCallback((value) => {
    toggleStrategyFeature(setSelectedStrategyTimeFeatures, STRATEGY_TIME_FEATURE_OPTIONS, value);
  }, [toggleStrategyFeature]);

  const toggleStrategyStrictnessFeature = useCallback((value) => {
    toggleStrategyFeature(
      setSelectedStrategyStrictnessFeatures,
      STRATEGY_STRICTNESS_FEATURE_OPTIONS,
      value
    );
  }, [toggleStrategyFeature]);

  const toggleStrategyReversalFeature = useCallback((value) => {
    toggleStrategyFeature(
      setSelectedStrategyReversalFeatures,
      STRATEGY_REVERSAL_FEATURE_OPTIONS,
      value
    );
  }, [toggleStrategyFeature]);

  const toggleStrategyMarketFeature = useCallback((value) => {
    toggleStrategyFeature(setSelectedStrategyMarketFeatures, STRATEGY_MARKET_FEATURE_OPTIONS, value);
  }, [toggleStrategyFeature]);

  const toggleStrategyPatternFeature = useCallback((value) => {
    toggleStrategyFeature(setSelectedStrategyPatternFeatures, STRATEGY_PATTERN_FEATURE_OPTIONS, value);
  }, [toggleStrategyFeature]);

  const toggleStrategyBinFeature = useCallback((value) => {
    toggleStrategyFeature(setSelectedStrategyBinFeatures, STRATEGY_BIN_FEATURE_OPTIONS, value);
  }, [toggleStrategyFeature]);

  const toggleStrategySizeFeature = useCallback((value) => {
    toggleStrategyFeature(setSelectedStrategySizeFeatures, STRATEGY_SIZE_FEATURE_OPTIONS, value);
  }, [toggleStrategyFeature]);

  const toggleStrategyThreeMonthTrendFeature = useCallback((value) => {
    toggleStrategyFeature(
      setSelectedStrategyThreeMonthTrendFeatures,
      STRATEGY_TREND_FEATURE_OPTIONS,
      value
    );
  }, [toggleStrategyFeature]);

  const toggleStrategySixMonthTrendFeature = useCallback((value) => {
    toggleStrategyFeature(
      setSelectedStrategySixMonthTrendFeatures,
      STRATEGY_TREND_FEATURE_OPTIONS,
      value
    );
  }, [toggleStrategyFeature]);

  const toggleStrategyTwelveMonthTrendFeature = useCallback((value) => {
    toggleStrategyFeature(
      setSelectedStrategyTwelveMonthTrendFeatures,
      STRATEGY_TREND_FEATURE_OPTIONS,
      value
    );
  }, [toggleStrategyFeature]);

  const isDefaultFitActive = useMemo(
    () =>
      sameOptionSet(selectedStrategyBinFeatures, STRATEGY_FIT_BIN_OPTIONS) &&
      sameOptionSet(selectedStrategyTimeFeatures, STRATEGY_FIT_BIN_OPTIONS) &&
      sameOptionSet(selectedStrategySizeFeatures, STRATEGY_FIT_SIZE_OPTIONS) &&
      sameOptionSet(selectedStrategyStrictnessFeatures, STRATEGY_FIT_STRICTNESS_OPTIONS),
    [
      selectedStrategyBinFeatures,
      selectedStrategySizeFeatures,
      selectedStrategyStrictnessFeatures,
      selectedStrategyTimeFeatures,
    ]
  );

  const toggleDefaultFitFilters = useCallback(() => {
    if (isDefaultFitActive) {
      setSelectedStrategyBinFeatures(STRATEGY_BIN_FEATURE_OPTIONS);
      setSelectedStrategyTimeFeatures(STRATEGY_TIME_FEATURE_OPTIONS);
      setSelectedStrategySizeFeatures(STRATEGY_SIZE_FEATURE_OPTIONS);
      setSelectedStrategyStrictnessFeatures(STRATEGY_STRICTNESS_FEATURE_OPTIONS);
      return;
    }

    setSelectedStrategyBinFeatures(STRATEGY_FIT_BIN_OPTIONS);
    setSelectedStrategyTimeFeatures(STRATEGY_FIT_BIN_OPTIONS);
    setSelectedStrategySizeFeatures(STRATEGY_FIT_SIZE_OPTIONS);
    setSelectedStrategyStrictnessFeatures(STRATEGY_FIT_STRICTNESS_OPTIONS);
  }, [isDefaultFitActive]);

  const strategyFeatureGroups = useMemo(
    () => [
      {
        title: 'Market',
        options: STRATEGY_MARKET_FEATURE_OPTIONS,
        selectedOptions: selectedStrategyMarketFeatures,
        onToggleOption: toggleStrategyMarketFeature,
      },
      {
        title: 'Dominant Harmonic',
        options: STRATEGY_PATTERN_FEATURE_OPTIONS,
        selectedOptions: selectedStrategyPatternFeatures,
        onToggleOption: toggleStrategyPatternFeature,
      },
      {
        title: 'Price Ratio Accuracy',
        options: STRATEGY_BIN_FEATURE_OPTIONS,
        selectedOptions: selectedStrategyBinFeatures,
        onToggleOption: toggleStrategyBinFeature,
      },
      {
        title: 'Size',
        options: STRATEGY_SIZE_FEATURE_OPTIONS,
        selectedOptions: selectedStrategySizeFeatures,
        onToggleOption: toggleStrategySizeFeature,
      },
      {
        title: 'Time Ratio Accuracy',
        options: STRATEGY_TIME_FEATURE_OPTIONS,
        selectedOptions: selectedStrategyTimeFeatures,
        onToggleOption: toggleStrategyTimeFeature,
      },
      {
        title: 'X Mode',
        options: STRATEGY_STRICTNESS_FEATURE_OPTIONS,
        selectedOptions: selectedStrategyStrictnessFeatures,
        onToggleOption: toggleStrategyStrictnessFeature,
      },
      {
        title: 'Reversal',
        options: STRATEGY_REVERSAL_FEATURE_OPTIONS,
        selectedOptions: selectedStrategyReversalFeatures,
        onToggleOption: toggleStrategyReversalFeature,
      },
      {
        title: '3M Trend',
        options: STRATEGY_TREND_FEATURE_OPTIONS,
        selectedOptions: selectedStrategyThreeMonthTrendFeatures,
        onToggleOption: toggleStrategyThreeMonthTrendFeature,
      },
      {
        title: '6M Trend',
        options: STRATEGY_TREND_FEATURE_OPTIONS,
        selectedOptions: selectedStrategySixMonthTrendFeatures,
        onToggleOption: toggleStrategySixMonthTrendFeature,
      },
      {
        title: '12M Trend',
        options: STRATEGY_TREND_FEATURE_OPTIONS,
        selectedOptions: selectedStrategyTwelveMonthTrendFeatures,
        onToggleOption: toggleStrategyTwelveMonthTrendFeature,
      },
    ],
    [
      selectedStrategyBinFeatures,
      selectedStrategyMarketFeatures,
      selectedStrategyPatternFeatures,
      selectedStrategyReversalFeatures,
      selectedStrategyStrictnessFeatures,
      selectedStrategySizeFeatures,
      selectedStrategySixMonthTrendFeatures,
      selectedStrategyThreeMonthTrendFeatures,
      selectedStrategyTimeFeatures,
      selectedStrategyTwelveMonthTrendFeatures,
      toggleStrategyBinFeature,
      toggleStrategyMarketFeature,
      toggleStrategyPatternFeature,
      toggleStrategyReversalFeature,
      toggleStrategyStrictnessFeature,
      toggleStrategySixMonthTrendFeature,
      toggleStrategySizeFeature,
      toggleStrategyThreeMonthTrendFeature,
      toggleStrategyTimeFeature,
      toggleStrategyTwelveMonthTrendFeature,
    ]
  );

  return (
    <div className="App">
      <div className="app-inner">
        <div className="main">
          <div className="app-header" ref={headerRef} />

              <div className="strategies-station-shell">
                <div className="strategies-scoreboard-zone">
                  <StrategyWorkbenchCard
                    strategy={selectedStrategy}
                    isLoading={isHydratingStrategy}
                  />
                </div>
                <div className="strategies-station-layout">
                  <div className="strategies-left-column">
                    <div className="strategies-feature-column">
                      <div className="q strategies-pool-zone">
                        <StrategyVariationPoolCard
                          embedded
                          featureGroups={strategyFeatureGroups}
                          strategyCount={strategyTableSnapshots.length}
                          totalStrategyCount={totalStrategyUniverseCount}
                          defaultFitActive={isDefaultFitActive}
                          onToggleDefaultFit={toggleDefaultFitFilters}
                        />
                      </div>
                    </div>

                    <div className="strategies-library-column">
                      <div className="q strategies-library-zone">
                        <StrategyLeaderboardCard
                          strategies={strategyTableSnapshots}
                          selectedStrategyId={selectedStrategy?.id ?? ''}
                          hoveredStrategyId={hoveredStrategyId}
                          isLoading={isLoadingStrategyUniverse}
                          sortState={strategySortState}
                          onSortChange={setStrategySortState}
                          onSelectStrategy={handleSelectStrategy}
                          onVisibleRangeChange={({ startIndex }) => {
                            setLeaderChartStartIndex(startIndex);
                          }}
                          filtersContent={
                            <>
                              <div className="strategies-filter-shell strategy-library-filter-shell">
                                <div className="strategies-filter-copy">
                                  <div className="strategies-filter-title">Strategy Filters</div>
                                  <div className="strategies-filter-subtitle">
                                    Narrow the prop family universe, then inspect the strongest cohorts.
                                  </div>
                                </div>

                                <div className="strategies-filter-grid">
                                  <label className="strategies-filter-field">
                                    <span className="strategies-filter-label">Closed &gt;=</span>
                                    <input
                                      type="number"
                                      min="0"
                                      step="1"
                                      className="strategies-filter-select"
                                      value={strategyFilters.bestPickMinClosedTrades}
                                      onChange={(event) =>
                                        updateStrategyFilter(
                                          'bestPickMinClosedTrades',
                                          event.target.value
                                        )
                                      }
                                    />
                                  </label>

                                  <label className="strategies-filter-field">
                                    <span className="strategies-filter-label">Exp &gt;</span>
                                    <input
                                      type="number"
                                      step="0.1"
                                      className="strategies-filter-select"
                                      value={strategyFilters.bestPickMinExpectancy}
                                      onChange={(event) =>
                                        updateStrategyFilter(
                                          'bestPickMinExpectancy',
                                          event.target.value
                                        )
                                      }
                                    />
                                  </label>

                                  <label className="strategies-filter-field">
                                    <span className="strategies-filter-label">Max Down</span>
                                    <input
                                      type="number"
                                      min="0"
                                      step="1"
                                      className="strategies-filter-select"
                                      value={strategyFilters.bestPickMaxDownYears}
                                      onChange={(event) =>
                                        updateStrategyFilter(
                                          'bestPickMaxDownYears',
                                          event.target.value
                                        )
                                      }
                                    />
                                  </label>

                                  <label className="strategies-filter-field">
                                    <span className="strategies-filter-label">Worst &gt;</span>
                                    <input
                                      type="number"
                                      step="0.1"
                                      className="strategies-filter-select"
                                      value={strategyFilters.bestPickMinWorstYearExpectancy}
                                      onChange={(event) =>
                                        updateStrategyFilter(
                                          'bestPickMinWorstYearExpectancy',
                                          event.target.value
                                        )
                                      }
                                    />
                                  </label>

                                  <label className="strategies-filter-field">
                                    <span className="strategies-filter-label">Score &gt;</span>
                                    <input
                                      type="number"
                                      step="0.1"
                                      className="strategies-filter-select"
                                      value={strategyFilters.bestPickMinScore}
                                      onChange={(event) =>
                                        updateStrategyFilter(
                                          'bestPickMinScore',
                                          event.target.value
                                        )
                                      }
                                    />
                                  </label>

                                  {isPropStrategyMode &&
                                  strategyLibraryPatternView === STRATEGY_LIBRARY_VIEW_CURRENT ? (
                                    <label className="strategies-filter-field">
                                      <span className="strategies-filter-label">Days Open</span>
                                      <select
                                        className="strategies-filter-select"
                                        value={strategyFilters.maxDaysOpen}
                                        onChange={(event) =>
                                          updateStrategyFilter('maxDaysOpen', event.target.value)
                                        }
                                      >
                                        {STRATEGY_DAYS_OPEN_OPTIONS.map((option) => (
                                          <option key={option} value={option}>
                                            {option === 'Any' ? 'Any' : `${option} days`}
                                          </option>
                                        ))}
                                      </select>
                                    </label>
                                  ) : null}
                                </div>

                                <div className="strategies-filter-summary">
                                  <span className="strategies-summary-pill">
                                    {strategyTableSnapshots.length} / {totalStrategyUniverseCount} shown
                                  </span>
                                  <span className="strategies-summary-pill">
                                    Closed &gt;= {strategyFilters.bestPickMinClosedTrades || 0},
                                    Exp &gt; {strategyFilters.bestPickMinExpectancy || 0},
                                    Down &lt;= {strategyFilters.bestPickMaxDownYears || 0},
                                    Worst &gt; {strategyFilters.bestPickMinWorstYearExpectancy || 0},
                                    Score &gt; {strategyFilters.bestPickMinScore || 0}
                                  </span>
                                </div>
                              </div>
                            </>
                          }
                          bottomContent={
                            <div className="strategy-library-matches">
                              <StrategyContractBreakdownPanel
                                selectedStrategy={selectedStrategy}
                                loadedTrades={strategyTrades}
                                totalTradeCount={strategyTradeTotalCount}
                              />

                              <div className="strategy-library-bottom-head">
                                <div className="strategy-library-mode-tabs">
                                  <button
                                    type="button"
                                    className={
                                      strategyLibraryPatternView === STRATEGY_LIBRARY_VIEW_MATCHED
                                        ? 'strategy-library-mode-tab strategy-library-mode-tab--active'
                                        : 'strategy-library-mode-tab'
                                    }
                                    onClick={() => {
                                      setStrategyLibraryPatternView(STRATEGY_LIBRARY_VIEW_MATCHED);
                                    }}
                                  >
                                    Matched Patterns
                                  </button>
                                  {isPropStrategyMode ? (
                                    <button
                                      type="button"
                                      className={
                                        strategyLibraryPatternView === STRATEGY_LIBRARY_VIEW_CURRENT
                                          ? 'strategy-library-mode-tab strategy-library-mode-tab--active'
                                          : 'strategy-library-mode-tab'
                                      }
                                      onClick={() => {
                                        setStrategyLibraryPatternView(STRATEGY_LIBRARY_VIEW_CURRENT);
                                      }}
                                    >
                                      Current Setups
                                    </button>
                                  ) : null}
                                </div>

                              </div>

                              <div className="strategy-library-subtitle">
                                {isPropStrategyMode &&
                                strategyLibraryPatternView === STRATEGY_LIBRARY_VIEW_CURRENT
                                  ? isPropReversalMode
                                    ? 'Current Reversal Setups'
                                    : 'Current D Setups'
                                  : isPropReversalMode
                                  ? 'Matched Reversal Patterns'
                                  : 'Matched Patterns'}
                              </div>

                              <div className="strategy-library-matches-table">
                                <Section>
                                  {isPropStrategyMode &&
                                  strategyLibraryPatternView === STRATEGY_LIBRARY_VIEW_CURRENT ? (
                                    <PatternTable
                                      key={`current-${selectedStrategy?.id ?? 'none'}`}
                                      density="compact"
                                      fixedHeight="420px"
                                      includeSizeColumn
                                      statusLabel="Current Setups"
                                      emptyMessage="No current setups are available right now."
                                      patterns={filteredStrategyCurrentSetups}
                                      totalPatternCount={filteredStrategyCurrentSetups.length}
                                      hasMorePatterns={false}
                                      isLoadingMorePatterns={isLoadingCurrentSetups}
                                      onLoadMorePatterns={null}
                                      onSelectPattern={handleSelectStrategyCurrentSetup}
                                      setLoadingPatterns={setLoadingStrategyChart}
                                      setChartData={setStrategyChartData}
                                      selectedRowIndex={selectedStrategyCurrentSetupIndex}
                                      setSelectedRowIndex={setSelectedStrategyCurrentSetupIndex}
                                      updateSelectedPattern={updateStrategyPatternForChart}
                                    />
                                  ) : (
                                    <PatternTable
                                      key={`matched-${selectedStrategy?.id ?? 'none'}`}
                                      density="compact"
                                      fixedHeight="420px"
                                      includeSizeColumn
                                      patterns={strategyTrades}
                                      totalPatternCount={strategyTradeTotalCount}
                                      hasMorePatterns={hasMoreStrategyTrades}
                                      isLoadingMorePatterns={isFetchingMoreStrategyTrades}
                                      onLoadMorePatterns={loadMoreStrategyTrades}
                                      onSelectPattern={handleSelectStrategyTrade}
                                      setLoadingPatterns={setLoadingStrategyChart}
                                      setChartData={setStrategyChartData}
                                      selectedRowIndex={selectedStrategyTradeIndex}
                                      setSelectedRowIndex={setSelectedStrategyTradeIndex}
                                      updateSelectedPattern={updateStrategyPatternForChart}
                                    />
                                  )}
                                </Section>
                              </div>
                            </div>
                          }
                        />
                      </div>
                    </div>
                  </div>

                  <div className="strategies-right-column">
                    <div className="strategies-workspace-frame">
                      <div className="strategies-workspace-tabs">
                        <button
                          type="button"
                          className={
                            activeStrategyWorkspaceView === STRATEGY_WORKSPACE_VIEW_CANVAS
                              ? 'strategies-workspace-tab strategies-workspace-tab--active'
                              : 'strategies-workspace-tab'
                          }
                          onClick={() =>
                            setActiveStrategyWorkspaceView(STRATEGY_WORKSPACE_VIEW_CANVAS)
                          }
                        >
                          Canvas
                        </button>
                        <button
                          type="button"
                          className={
                            activeStrategyWorkspaceView === STRATEGY_WORKSPACE_VIEW_GRAPHS
                              ? 'strategies-workspace-tab strategies-workspace-tab--active'
                              : 'strategies-workspace-tab'
                          }
                          onClick={() =>
                            setActiveStrategyWorkspaceView(STRATEGY_WORKSPACE_VIEW_GRAPHS)
                          }
                        >
                          Graphs
                        </button>
                        <button
                          type="button"
                          className={
                            activeStrategyWorkspaceView === STRATEGY_WORKSPACE_VIEW_FREQUENCY
                              ? 'strategies-workspace-tab strategies-workspace-tab--active'
                              : 'strategies-workspace-tab'
                          }
                          onClick={() =>
                            setActiveStrategyWorkspaceView(STRATEGY_WORKSPACE_VIEW_FREQUENCY)
                          }
                        >
                          Frequency
                        </button>
                        <button
                          type="button"
                          className={
                            activeStrategyWorkspaceView === STRATEGY_WORKSPACE_VIEW_SIMULATOR
                              ? 'strategies-workspace-tab strategies-workspace-tab--active'
                              : 'strategies-workspace-tab'
                          }
                          onClick={() =>
                            setActiveStrategyWorkspaceView(STRATEGY_WORKSPACE_VIEW_SIMULATOR)
                          }
                        >
                          Simulator
                        </button>
                      </div>

                      <div className="strategies-workspace-body">
                        {activeStrategyWorkspaceView === STRATEGY_WORKSPACE_VIEW_CANVAS ? (
                          <div className="strategies-inspector-shell">
                            <div className="strategies-inspector-grid strategies-inspector-grid--chart-only">
                              <div className="strategies-chart-zone">
                                <CandleChartPanel
                                  chartData={strategyChartData}
                                  isSectionsExpanded={isStrategyChartExpanded}
                                  setSectionsExpanded={setStrategyChartExpanded}
                                  focusMode={isPropStrategyMode ? 'prop' : 'pattern'}
                                  market={
                                    strategyChartData?.rust_patterns?.market ??
                                    selectedStrategy?.market ??
                                    'Bullish'
                                  }
                                  overlayTopOffset={chartOverlayTop}
                                  overlayTableProps={{
                                    density: 'compact',
                                    includeSizeColumn: true,
                                    patterns: strategyTrades,
                                    totalPatternCount: strategyTradeTotalCount,
                                    hasMorePatterns: hasMoreStrategyTrades,
                                    isLoadingMorePatterns: isFetchingMoreStrategyTrades,
                                    onLoadMorePatterns: loadMoreStrategyTrades,
                                    onSelectPattern: handleSelectStrategyTrade,
                                    setLoadingPatterns: setLoadingStrategyChart,
                                    setChartData: setStrategyChartData,
                                    selectedRowIndex: selectedStrategyTradeIndex,
                                    setSelectedRowIndex: setSelectedStrategyTradeIndex,
                                    updateSelectedPattern: updateStrategyPatternForChart,
                                  }}
                                />
                              </div>
                            </div>
                          </div>
                        ) : activeStrategyWorkspaceView === STRATEGY_WORKSPACE_VIEW_GRAPHS ? (
                          <div className="strategies-workspace-shell">
                            <StrategyInsightCharts
                              strategies={rankedStrategyTableSnapshots}
                              selectedStrategy={selectedStrategyForInsights}
                              selectedStrategyId={selectedStrategy?.id ?? ''}
                              isHydratingStrategy={isHydratingStrategy}
                              leaderChartStartIndex={leaderChartStartIndex}
                              onSelectStrategy={handleSelectStrategyFromChart}
                              onHoverStrategy={setHoveredStrategyId}
                            />
                          </div>
                        ) : activeStrategyWorkspaceView === STRATEGY_WORKSPACE_VIEW_FREQUENCY ? (
                          <div className="strategies-workspace-shell">
                            <StrategyFrequencyPanel strategy={selectedStrategyForInsights} />
                          </div>
                        ) : (
                          <div className="strategies-workspace-shell">
                            <TradeSimulatorPanel
                              selectedStrategy={selectedStrategy}
                              loadedTrades={strategyTrades}
                              totalTradeCount={strategyTradeTotalCount}
                            />
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
        </div>
      </div>
    </div>
  );
};

export default App;
