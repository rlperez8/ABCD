const API_BASE_URL = 'http://localhost:8080';
const DEFAULT_PROP_OUTCOME_MODE = 'reversal';

const parseOptionalFloat = (value) => {
  if (value === null || value === undefined || value === '') {
    return null;
  }

  const parsed = parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
};

const parseOptionalInt = (value) => {
  if (value === null || value === undefined || value === '') {
    return null;
  }

  const parsed = parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
};

const normalizeTradeResult = (value) => {
  if (value === 0 || value === '0' || value === 'Open') return 0;
  if (value === 1 || value === '1' || value === 'Won') return 1;
  if (value === 2 || value === '2' || value === 'Lost') return 2;
  return null;
};

const buildDashboardFilters = (activeFilters = {}) => {
  const market =
    activeFilters?.market?.active && activeFilters.market.filter !== 'Both'
      ? activeFilters.market.filter
      : null;

  const tradeResult =
    activeFilters?.result?.active
      ? normalizeTradeResult(activeFilters.result.filter)
      : null;

  const retracement =
    activeFilters?.retracement?.active && activeFilters.retracement.filter
      ? activeFilters.retracement.filter
      : null;

  return {
    market,
    trade_result: tradeResult,
    retracement,
  };
};

const buildFamilyStrategyRequestOptions = ({ sort = null, familyFilters = null } = {}) => ({
  sort_by: sort?.key ?? null,
  sort_direction: sort?.direction ?? null,
  strategy_markets: familyFilters?.markets ?? null,
  strategy_harmonic_types: familyFilters?.harmonicTypes ?? null,
  strategy_bins: familyFilters?.bins ?? null,
  strategy_reversal_types: familyFilters?.reversalTypes ?? null,
  strategy_size_buckets: familyFilters?.sizeBuckets ?? null,
  strategy_time_bins: familyFilters?.timeBins ?? null,
  strategy_x_strictness: familyFilters?.xStrictness ?? null,
});

const buildBestPickRequestOptions = (bestPickFilters = null) =>
  Object.fromEntries(
    Object.entries({
      min_closed_trades: parseOptionalInt(bestPickFilters?.bestPickMinClosedTrades),
      min_expectancy: parseOptionalFloat(bestPickFilters?.bestPickMinExpectancy),
      max_down_years: parseOptionalInt(bestPickFilters?.bestPickMaxDownYears),
      min_worst_year_expectancy: parseOptionalFloat(bestPickFilters?.bestPickMinWorstYearExpectancy),
      min_score: parseOptionalFloat(bestPickFilters?.bestPickMinScore),
    }).filter(([, value]) => value !== null && value !== undefined)
  );

const postJson = async (path, body) => {
  const response = await fetch(`${API_BASE_URL}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    console.error(`Server Error: ${response.status} - ${response.statusText}`);
    throw new Error('Request failed');
  }

  return response.json();
};

const parsePatternSummaryRecord = (pattern) => ({
  ...pattern,
  prop_strategy_id: pattern.prop_strategy_id ?? null,
  d_confirm_date: pattern.d_confirm_date ?? null,
  reversal_detect_date: pattern.reversal_detect_date ?? null,
  target_date: pattern.target_date ?? null,
  trade_enter_price: parseFloat(pattern.trade_enter_price),
  trade_risk_exit_price: parseFloat(pattern.trade_risk_exit_price),
  trade_reward_exit_price: parseFloat(pattern.trade_reward_exit_price),
  target_open: parseOptionalFloat(pattern.target_open),
  target_high: parseOptionalFloat(pattern.target_high),
  target_low: parseOptionalFloat(pattern.target_low),
  target_close: parseOptionalFloat(pattern.target_close),
  x_length: parseOptionalInt(pattern.x_length),
  a_length: parseOptionalInt(pattern.a_length),
  b_length: parseOptionalInt(pattern.b_length),
  c_length: parseOptionalInt(pattern.c_length),
  d_length: parseOptionalInt(pattern.d_length),
  full_pattern_length: parseOptionalInt(pattern.full_pattern_length),
  reversal_type: pattern.reversal_type ?? 'None',
  x_strictness: pattern.x_strictness ?? null,
  time_accuracy: parseOptionalFloat(pattern.time_accuracy),
  three_month_trend: pattern.three_month_trend ?? null,
  six_month_trend: pattern.six_month_trend ?? null,
  twelve_month_trend: pattern.twelve_month_trend ?? null,
  bat_accuracy: parseOptionalFloat(pattern.bat_accuracy),
  butterfly_accuracy: parseOptionalFloat(pattern.butterfly_accuracy),
  gartley_accuracy: parseOptionalFloat(pattern.gartley_accuracy),
  crab_accuracy: parseOptionalFloat(pattern.crab_accuracy),
  shark_accuracy: parseOptionalFloat(pattern.shark_accuracy),
});

const DECIMAL_PATTERN_FIELDS = [
  'x_open',
  'x_high',
  'x_low',
  'x_close',
  'x_length',
  'x_min_max',
  'a_open',
  'a_high',
  'a_low',
  'a_close',
  'a_length',
  'a_min_max',
  'b_open',
  'b_high',
  'b_low',
  'b_close',
  'b_length',
  'b_min_max',
  'c_open',
  'c_high',
  'c_low',
  'c_close',
  'c_length',
  'c_min_max',
  'd_open',
  'd_high',
  'd_low',
  'd_close',
  'd_length',
  'd_min_max',
  'trade_risk_exit_price',
  'trade_reward_exit_price',
  'trade_enter_price',
  'trade_current_price',
  'trade_length',
  'trade_pnl',
  'trade_ab_price_retracement',
  'trade_bc_price_retracement',
  'trade_cd_bc_price_retracement',
  'trade_cd_price_retracement',
  'trade_cd_xa_price_retracement',
  'trade_bc_bar_retracement',
  'trade_cd_bar_retracement',
  'trade_snr',
  'target_open',
  'target_high',
  'target_low',
  'target_close',
  'target_close_vs_open_pct',
  'target_high_vs_open_pct',
  'target_low_vs_open_pct',
  'target_range_pct',
  'bat_accuracy',
  'butterfly_accuracy',
  'gartley_accuracy',
  'crab_accuracy',
  'shark_accuracy',
];

const INT_PATTERN_FIELDS = [
  'trade_year',
  'trade_month',
  'trade_day',
  'full_pattern_length',
  'volume',
  'target_volume',
];

const parsePatternDetailRecord = (pattern) => {
  if (!pattern) {
    return null;
  }

  const nextPattern = { ...pattern };

  DECIMAL_PATTERN_FIELDS.forEach((field) => {
    if (field in nextPattern) {
      nextPattern[field] = parseOptionalFloat(nextPattern[field]);
    }
  });

  INT_PATTERN_FIELDS.forEach((field) => {
    if (field in nextPattern) {
      nextPattern[field] = parseOptionalInt(nextPattern[field]);
    }
  });

  nextPattern.trade_result = parseOptionalInt(nextPattern.trade_result) ?? 0;
  nextPattern.trade_open = Boolean(nextPattern.trade_open);
  nextPattern.target_ready = Boolean(nextPattern.target_ready);
  nextPattern.target_is_green =
    nextPattern.target_is_green === null || nextPattern.target_is_green === undefined
      ? null
      : Boolean(nextPattern.target_is_green);
  nextPattern.target_breaks_d_high =
    nextPattern.target_breaks_d_high === null || nextPattern.target_breaks_d_high === undefined
      ? null
      : Boolean(nextPattern.target_breaks_d_high);
  nextPattern.target_breaks_d_low =
    nextPattern.target_breaks_d_low === null || nextPattern.target_breaks_d_low === undefined
      ? null
      : Boolean(nextPattern.target_breaks_d_low);
  nextPattern.trade_ab_bar_retracement = parseOptionalFloat(nextPattern.trade_ab_bar_retracement);
  nextPattern.trade_cd_bc_bar_retracement = parseOptionalFloat(nextPattern.trade_cd_bc_bar_retracement);
  nextPattern.trade_cd_xa_bar_retracement = parseOptionalFloat(nextPattern.trade_cd_xa_bar_retracement);
  nextPattern.time_accuracy = parseOptionalFloat(nextPattern.time_accuracy);

  return nextPattern;
};

const parseCandleRecord = (candle) => ({
  ...candle,
  candle_date: candle?.date ?? candle?.candle_date ?? null,
  open: parseOptionalFloat(candle?.open),
  high: parseOptionalFloat(candle?.high),
  low: parseOptionalFloat(candle?.low),
  close: parseOptionalFloat(candle?.close),
  volume: parseOptionalInt(candle?.volume),
  candle_open: parseOptionalFloat(candle?.open ?? candle?.candle_open),
  candle_high: parseOptionalFloat(candle?.high ?? candle?.candle_high),
  candle_low: parseOptionalFloat(candle?.low ?? candle?.candle_low),
  candle_close: parseOptionalFloat(candle?.close ?? candle?.candle_close),
  candle_volume: parseOptionalInt(candle?.volume ?? candle?.candle_volume),
});

const parseStrategyCandidateRecord = (strategy) => ({
  ...strategy,
  family_key: strategy?.family_key ?? strategy?.prop_strategy_id ?? null,
  family_name: strategy?.family_name ?? null,
  family_level: parseOptionalInt(strategy?.family_level),
  included_dimensions: strategy?.included_dimensions ?? null,
  outcome_model: strategy?.outcome_model ?? null,
  prop_strategy_id: strategy?.family_key ?? strategy?.prop_strategy_id ?? null,
  worst_year_expectancy: parseOptionalFloat(strategy?.worst_year_expectancy) ?? 0,
  down_years: parseOptionalInt(strategy?.down_years) ?? 0,
  total_count: parseOptionalInt(strategy?.total_count) ?? 0,
  closed_count: parseOptionalInt(strategy?.closed_count) ?? 0,
  open_count: parseOptionalInt(strategy?.open_count) ?? 0,
  win_count: parseOptionalInt(strategy?.win_count) ?? 0,
  loss_count: parseOptionalInt(strategy?.loss_count) ?? 0,
  expectancy: parseOptionalFloat(strategy?.expectancy) ?? 0,
  avg_return: parseOptionalFloat(strategy?.avg_return) ?? 0,
  win_rate: parseOptionalFloat(strategy?.win_rate) ?? 0,
  closed_rate: parseOptionalFloat(strategy?.closed_rate) ?? 0,
  avg_win: parseOptionalFloat(strategy?.avg_win) ?? 0,
  avg_loss: parseOptionalFloat(strategy?.avg_loss) ?? 0,
  avg_target_range: parseOptionalFloat(strategy?.avg_target_range) ?? 0,
  score: parseOptionalFloat(strategy?.score) ?? 0,
  x_strictness: strategy?.x_strictness ?? null,
  weeklyCadence: {
    totalCalendarWeeks: parseOptionalInt(strategy?.total_calendar_weeks) ?? 0,
    activeWeeks: parseOptionalInt(strategy?.active_weeks) ?? 0,
    zeroSetupWeeks: parseOptionalInt(strategy?.zero_setup_weeks) ?? 0,
    zeroSetupWeekRate: parseOptionalFloat(strategy?.zero_setup_week_rate) ?? 0,
    totalSetups: parseOptionalInt(strategy?.total_setups) ?? 0,
    avgSetupsPerWeek: parseOptionalFloat(strategy?.avg_setups_per_week) ?? 0,
    maxSetupsPerWeek: parseOptionalInt(strategy?.max_setups_per_week) ?? 0,
  },
});

const parseSetupComparisonResponse = (comparison) => {
  if (!comparison) {
    return null;
  }

  const summary = comparison.summary ?? {};
  const yearlyPerformance = Array.isArray(comparison.yearly_performance)
    ? comparison.yearly_performance.map((point) => ({
        ...point,
        year: parseOptionalInt(point?.year) ?? 0,
        total_count: parseOptionalInt(point?.total_count) ?? 0,
        closed_count: parseOptionalInt(point?.closed_count) ?? 0,
        open_count: parseOptionalInt(point?.open_count) ?? 0,
        win_count: parseOptionalInt(point?.win_count) ?? 0,
        loss_count: parseOptionalInt(point?.loss_count) ?? 0,
        expectancy: parseOptionalFloat(point?.expectancy) ?? 0,
        avg_return: parseOptionalFloat(point?.avg_return) ?? 0,
        win_rate: parseOptionalFloat(point?.win_rate) ?? 0,
      }))
    : [];
  const recentExamples = Array.isArray(comparison.recent_examples)
    ? comparison.recent_examples.map((example) => ({
        ...example,
        trade_result: parseOptionalInt(example?.trade_result) ?? 0,
        trade_pnl: parseOptionalFloat(example?.trade_pnl) ?? 0,
        trade_length: parseOptionalFloat(example?.trade_length) ?? 0,
        trade_enter_price: parseOptionalFloat(example?.trade_enter_price) ?? 0,
      }))
    : [];

  return {
    ...comparison,
    summary: {
      total_count: parseOptionalInt(summary?.total_count) ?? 0,
      closed_count: parseOptionalInt(summary?.closed_count) ?? 0,
      open_count: parseOptionalInt(summary?.open_count) ?? 0,
      win_count: parseOptionalInt(summary?.win_count) ?? 0,
      loss_count: parseOptionalInt(summary?.loss_count) ?? 0,
      expectancy: parseOptionalFloat(summary?.expectancy) ?? 0,
      avg_return: parseOptionalFloat(summary?.avg_return) ?? 0,
      win_rate: parseOptionalFloat(summary?.win_rate) ?? 0,
      avg_win: parseOptionalFloat(summary?.avg_win) ?? 0,
      avg_loss: parseOptionalFloat(summary?.avg_loss) ?? 0,
      avg_trade_length: parseOptionalFloat(summary?.avg_trade_length) ?? 0,
      avg_ab_xa: parseOptionalFloat(summary?.avg_ab_xa) ?? 0,
      avg_bc_ab: parseOptionalFloat(summary?.avg_bc_ab) ?? 0,
      avg_cd_bc: parseOptionalFloat(summary?.avg_cd_bc) ?? 0,
      avg_cd_xa: parseOptionalFloat(summary?.avg_cd_xa) ?? 0,
    },
    yearly_performance: yearlyPerformance,
    recent_examples: recentExamples,
  };
};

const parseStrategyContractWeekRecord = (row) => ({
  ...row,
  family_key: row?.family_key ?? null,
  symbol: row?.symbol ?? 'Unknown',
  contract_week_index: parseOptionalInt(row?.contract_week_index) ?? 0,
  total_count: parseOptionalInt(row?.total_count) ?? 0,
  closed_count: parseOptionalInt(row?.closed_count) ?? 0,
  open_count: parseOptionalInt(row?.open_count) ?? 0,
  win_count: parseOptionalInt(row?.win_count) ?? 0,
  loss_count: parseOptionalInt(row?.loss_count) ?? 0,
  expectancy: parseOptionalFloat(row?.expectancy) ?? 0,
  avg_return: parseOptionalFloat(row?.avg_return) ?? 0,
  win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
});

export const getCandles = async (symbol, { startDate = null, endDate = null } = {}) => {
  if (!symbol) {
    return [];
  }

  try {
    const candles = await postJson('/candles', {
      symbol,
      start_date: startDate,
      end_date: endDate,
    });
    return Array.isArray(candles) ? candles.map(parseCandleRecord).reverse() : [];
  } catch (error) {
    console.error(error);
    return [];
  }
};

export const fetchStrategyContractWeeks = async (strategy, { limit = 2000 } = {}) => {
  if (!strategy) {
    return [];
  }

  try {
    const data = await postJson('/strategy-contract-weeks', {
      prop_strategy_id: strategy.propStrategyId ?? strategy.familyKey ?? strategy.id ?? null,
      limit,
    });

    return Array.isArray(data) ? data.map(parseStrategyContractWeekRecord) : [];
  } catch (error) {
    console.error(error);
    return [];
  }
};

export const getSupportResistanceLines = async () => [];

export const fetchCurrentSetups = async (
  filters,
  activeFilters = {},
  options = { limit: 1000, maxDaysOpen: 7, propOutcomeMode: DEFAULT_PROP_OUTCOME_MODE }
) => {
  const dashboardFilters = buildDashboardFilters({
    ...activeFilters,
    result: { active: false, filter: null },
  });

  const filter = {
    bin: filters.bin,
    harmonic_type: filters.harmonicType,
    max_days_open: options.maxDaysOpen ?? null,
    limit: options.limit,
    offset: 0,
    include_count: false,
    ...dashboardFilters,
    trade_result: 0,
    prop_outcome_mode: options?.propOutcomeMode ?? DEFAULT_PROP_OUTCOME_MODE,
  };

  try {
    const data = await postJson('/current-open-setups', filter);
    data.patterns = data.patterns.map((pattern) =>
      parsePatternSummaryRecord({
        ...pattern,
        prop_outcome_mode: options?.propOutcomeMode ?? DEFAULT_PROP_OUTCOME_MODE,
      })
    );

    return {
      ...data,
      total_count: data.total_count ?? data.patterns.length,
      has_more: Boolean(data.has_more),
    };
  } catch (error) {
    console.error(error);
    return { patterns: [], total_count: 0, has_more: false };
  }
};

export const fetchStrategyTrades = async (
  strategy,
  pagination = { limit: 250, offset: 0, includeCount: true },
  options = { propMode: false, propOutcomeMode: DEFAULT_PROP_OUTCOME_MODE }
) => {
  if (!strategy) {
    return { patterns: [], total_count: 0, has_more: false };
  }

  const filter = {
    prop_strategy_id: strategy.propStrategyId ?? strategy.id ?? null,
    harmonic_type: strategy.harmonicType,
    market: strategy.market,
    bin: strategy.bin,
    reversal_type: strategy.reversalType ?? 'None',
    size_bucket: strategy.sizeBucket,
    time_bin: strategy.timeBin,
    include_count: pagination.includeCount ?? true,
    limit: pagination.limit ?? 250,
    offset: pagination.offset ?? 0,
    prop_mode: Boolean(options?.propMode),
    prop_outcome_mode: options?.propOutcomeMode ?? DEFAULT_PROP_OUTCOME_MODE,
  };

  try {
    const data = await postJson('/strategy-trades', filter);
    data.patterns = Array.isArray(data?.patterns)
      ? data.patterns.map((pattern) =>
          parsePatternSummaryRecord({
            ...pattern,
            prop_outcome_mode: options?.propOutcomeMode ?? DEFAULT_PROP_OUTCOME_MODE,
          })
        )
      : [];

    return {
      ...data,
      total_count: data?.total_count ?? data.patterns.length,
      has_more: Boolean(data?.has_more),
    };
  } catch (error) {
    console.error(error);
    return { patterns: [], total_count: 0, has_more: false };
  }
};

export const fetchCurrentSetupStrategies = async (
  filters,
  activeFilters = {},
  options = { maxDaysOpen: 7, propOutcomeMode: DEFAULT_PROP_OUTCOME_MODE }
) => {
  const dashboardFilters = buildDashboardFilters({
    ...activeFilters,
    result: { active: false, filter: null },
  });

  const filter = {
    bin: filters.bin,
    harmonic_type: filters.harmonicType,
    max_days_open: options.maxDaysOpen ?? null,
    ...dashboardFilters,
    ...buildFamilyStrategyRequestOptions(options),
    ...buildBestPickRequestOptions(options?.bestPickFilters),
    trade_result: 0,
    prop_outcome_mode: options?.propOutcomeMode ?? DEFAULT_PROP_OUTCOME_MODE,
  };

  try {
    const data = await postJson('/current-setup-strategies', filter);
    return Array.isArray(data) ? data.map(parseStrategyCandidateRecord) : [];
  } catch (error) {
    console.error(error);
    return [];
  }
};

export const fetchPatternDetail = async (patternSummary) => {
  if (!patternSummary?.pattern_id && !patternSummary?.pattern_group_id) {
    return null;
  }

  const request = {
    symbol: patternSummary.symbol ?? null,
    pattern_id: patternSummary.pattern_id ?? null,
    pattern_group_id: patternSummary.pattern_group_id,
    x_date: patternSummary.x_date ?? null,
    d_date: patternSummary.d_date ?? null,
    market: patternSummary.market ?? null,
    harmonic_type: patternSummary.harmonic_type ?? null,
    size_bucket: patternSummary.size_bucket ?? null,
    balance_bucket: patternSummary.balance_bucket ?? null,
    trade_enter_price: patternSummary.trade_enter_price ?? null,
    trade_risk_exit_price: patternSummary.trade_risk_exit_price ?? null,
    trade_reward_exit_price: patternSummary.trade_reward_exit_price ?? null,
    x_length: patternSummary.x_length ?? null,
    a_length: patternSummary.a_length ?? null,
    b_length: patternSummary.b_length ?? null,
    c_length: patternSummary.c_length ?? null,
    prop_outcome_mode: patternSummary.prop_outcome_mode ?? null,
  };

  try {
    const pattern = await postJson('/pattern-detail', request);
    return parsePatternDetailRecord(pattern);
  } catch (error) {
    console.error(error);
    return null;
  }
};

export const fetchSetupComparison = async ({
  propStrategyId = null,
  harmonicType,
  market,
  bin,
  reversalType = null,
  sizeBucket = null,
  timeBin = null,
  includeExamples = true,
  propMode = false,
  propOutcomeMode = DEFAULT_PROP_OUTCOME_MODE,
}) => {
  const filter = {
    harmonic_type: harmonicType,
    prop_strategy_id: propStrategyId,
    market,
    bin,
    reversal_type: reversalType,
    size_bucket: sizeBucket,
    time_bin: timeBin,
    include_examples: includeExamples,
    prop_mode: Boolean(propMode),
    prop_outcome_mode: propOutcomeMode,
  };

  try {
    const data = await postJson('/setup-comparison', filter);
    return parseSetupComparisonResponse(data);
  } catch (error) {
    console.error(error);
    return null;
  }
};

export const fetchStrategyCandidates = async ({
  minClosedTrades = 100,
  limit = 250,
  propMode = false,
  propOutcomeMode = DEFAULT_PROP_OUTCOME_MODE,
  sort = null,
  familyFilters = null,
  bestPickFilters = null,
} = {}) => {
  const filter = {
    min_closed_trades: minClosedTrades,
    limit,
    prop_mode: Boolean(propMode),
    prop_outcome_mode: propOutcomeMode,
    ...buildFamilyStrategyRequestOptions({ sort, familyFilters }),
    ...buildBestPickRequestOptions(bestPickFilters),
  };

  try {
    const data = await postJson('/strategy-candidates', filter);
    return Array.isArray(data) ? data.map(parseStrategyCandidateRecord) : [];
  } catch (error) {
    console.error(error);
    return [];
  }
};

export const get_candles = getCandles;
export const get_support_resistance_lines = getSupportResistanceLines;
