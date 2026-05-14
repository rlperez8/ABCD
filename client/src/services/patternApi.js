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
    const errorText = await response.text();
    const message = errorText || `${response.status} - ${response.statusText}`;
    console.error(`Server Error: ${message}`);
    throw new Error(message);
  }

  return response.json();
};

const parsePatternSummaryRecord = (pattern) => ({
  ...pattern,
  prop_strategy_id: pattern.prop_strategy_id ?? null,
  d_confirm_date: pattern.d_confirm_date ?? null,
  reversal_detect_date: pattern.reversal_detect_date ?? null,
  entry_date: pattern.entry_date ?? null,
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

const parsePatternFamilyRecord = (family = {}) => ({
  ...family,
  family_key: family?.family_key ?? null,
  family_name: family?.family_name ?? null,
  family_level: parseOptionalInt(family?.family_level) ?? 0,
  included_dimensions: family?.included_dimensions ?? null,
  outcome_model: family?.outcome_model ?? null,
  setup_count: parseOptionalInt(family?.setup_count) ?? 0,
  symbol_count: parseOptionalInt(family?.symbol_count) ?? 0,
  first_d_date: family?.first_d_date ?? null,
  last_d_date: family?.last_d_date ?? null,
});

const parsePhase1ResultRecord = (row = {}) => ({
  ...row,
  result_rank: parseOptionalInt(row?.result_rank) ?? 0,
  period_year: parseOptionalInt(row?.period_year) ?? 0,
  target_r: parseOptionalFloat(row?.target_r) ?? 0,
  max_hold_multiple: parseOptionalInt(row?.max_hold_multiple) ?? 0,
  setup_count: parseOptionalInt(row?.setup_count) ?? 0,
  trade_count: parseOptionalInt(row?.trade_count) ?? 0,
  no_entry_count: parseOptionalInt(row?.no_entry_count) ?? 0,
  win_count: parseOptionalInt(row?.win_count) ?? 0,
  loss_count: parseOptionalInt(row?.loss_count) ?? 0,
  win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
  profit_factor: parseOptionalFloat(row?.profit_factor) ?? 0,
  max_drawdown_r: parseOptionalFloat(row?.max_drawdown_r) ?? 0,
  worst_year_avg_r: parseOptionalFloat(row?.worst_year_avg_r) ?? 0,
  score: parseOptionalFloat(row?.score) ?? 0,
});

const parsePhase1SupplySymbolRecord = (row = {}) => ({
  ...row,
  setup_count: parseOptionalInt(row?.setup_count) ?? 0,
  pattern_count: parseOptionalInt(row?.pattern_count) ?? 0,
  family_count: parseOptionalInt(row?.family_count) ?? 0,
  contract_count: parseOptionalInt(row?.contract_count) ?? 0,
});

const parsePhase1SupplyFamilyRecord = (row = {}) => ({
  ...row,
  setup_count: parseOptionalInt(row?.setup_count) ?? 0,
  pattern_count: parseOptionalInt(row?.pattern_count) ?? 0,
  symbol_count: parseOptionalInt(row?.symbol_count) ?? 0,
});

const parsePhase1YearlyBreakdown = (data = null) => {
  if (!data) {
    return null;
  }

  return {
    route: parsePhase1ResultRecord(data?.route ?? {}),
    cached: Boolean(data?.cached),
    years: Array.isArray(data?.years)
      ? data.years.map((row) => ({
          ...row,
          year: parseOptionalInt(row?.year) ?? 0,
          setup_count: parseOptionalInt(row?.setup_count) ?? 0,
          trade_count: parseOptionalInt(row?.trade_count) ?? 0,
          no_entry_count: parseOptionalInt(row?.no_entry_count) ?? 0,
          win_count: parseOptionalInt(row?.win_count) ?? 0,
          loss_count: parseOptionalInt(row?.loss_count) ?? 0,
          win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
          avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
          sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
          profit_factor: parseOptionalFloat(row?.profit_factor) ?? 0,
          max_drawdown_r: parseOptionalFloat(row?.max_drawdown_r) ?? 0,
        }))
      : [],
  };
};

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

const parseSimulatorReplayResponse = (data) => ({
  family_key: data?.family_key ?? null,
  eligible_trade_count: parseOptionalInt(data?.eligible_trade_count) ?? 0,
  candidate_logic_applied: Boolean(data?.candidate_logic_applied),
  candidate_logic_filters: Array.isArray(data?.candidate_logic_filters)
    ? data.candidate_logic_filters
    : [],
  tests: Array.isArray(data?.tests)
    ? data.tests.map((test) => ({
        ...test,
        test_index: parseOptionalInt(test?.test_index) ?? 0,
        starting_balance: parseOptionalFloat(test?.starting_balance) ?? 0,
        ending_balance: parseOptionalFloat(test?.ending_balance) ?? 0,
        peak_balance: parseOptionalFloat(test?.peak_balance) ?? 0,
        max_drawdown: parseOptionalFloat(test?.max_drawdown) ?? 0,
        trade_count: parseOptionalInt(test?.trade_count) ?? 0,
        skipped_overlap_count: parseOptionalInt(test?.skipped_overlap_count) ?? 0,
      }))
    : [],
  trades: Array.isArray(data?.trades)
    ? data.trades.map((trade) => ({
        ...trade,
        trade_id: parseOptionalInt(trade?.trade_id),
        trade_uid: trade?.trade_uid ?? null,
        trade_direction: trade?.trade_direction ?? null,
        test_index: parseOptionalInt(trade?.test_index) ?? 0,
        trade_index: parseOptionalInt(trade?.trade_index) ?? 0,
        trade_result: parseOptionalInt(trade?.trade_result) ?? 0,
        exit_reason: trade?.exit_reason ?? null,
        trade_enter_price: parseOptionalFloat(trade?.trade_enter_price),
        trade_risk_exit_price: parseOptionalFloat(trade?.trade_risk_exit_price),
        trade_reward_exit_price: parseOptionalFloat(trade?.trade_reward_exit_price),
        exit_price: parseOptionalFloat(trade?.exit_price),
        result_r: parseOptionalFloat(trade?.result_r),
        risk_points: parseOptionalFloat(trade?.risk_points),
        pnl: parseOptionalFloat(trade?.pnl) ?? 0,
        closed_pnl: parseOptionalFloat(trade?.closed_pnl) ?? parseOptionalFloat(trade?.pnl) ?? 0,
        point_value: parseOptionalFloat(trade?.point_value) ?? 0,
        balance_before: parseOptionalFloat(trade?.balance_before) ?? null,
        balance: parseOptionalFloat(trade?.balance) ?? 0,
        closed_balance: parseOptionalFloat(trade?.closed_balance) ?? parseOptionalFloat(trade?.balance) ?? 0,
        intratrade_low_balance:
          parseOptionalFloat(trade?.intratrade_low_balance) ?? parseOptionalFloat(trade?.balance) ?? 0,
        intratrade_high_balance:
          parseOptionalFloat(trade?.intratrade_high_balance) ?? parseOptionalFloat(trade?.balance) ?? 0,
        intratrade_adverse_pnl: parseOptionalFloat(trade?.intratrade_adverse_pnl) ?? 0,
        intratrade_favorable_pnl: parseOptionalFloat(trade?.intratrade_favorable_pnl) ?? 0,
        drawdown: parseOptionalFloat(trade?.drawdown) ?? 0,
        trade_lowest_price: parseOptionalFloat(trade?.trade_lowest_price),
        trade_highest_price: parseOptionalFloat(trade?.trade_highest_price),
        trade_adverse_price: parseOptionalFloat(trade?.trade_adverse_price),
        trade_favorable_price: parseOptionalFloat(trade?.trade_favorable_price),
        max_adverse_points: parseOptionalFloat(trade?.max_adverse_points) ?? 0,
        max_favorable_points: parseOptionalFloat(trade?.max_favorable_points) ?? 0,
        failed_intratrade_drawdown: Boolean(trade?.failed_intratrade_drawdown),
        failure_reason: trade?.failure_reason ?? null,
        skipped_for_overlap: Boolean(trade?.skipped_for_overlap),
      }))
    : [],
});

const parsePatternDiscoverySummary = (summary = {}) => ({
  observations: parseOptionalInt(summary?.observations) ?? 0,
  complete_windows: parseOptionalInt(summary?.complete_windows) ?? 0,
  avg_bars_observed: parseOptionalFloat(summary?.avg_bars_observed) ?? 0,
  avg_mfe_r: parseOptionalFloat(summary?.avg_mfe_r) ?? 0,
  avg_mae_r: parseOptionalFloat(summary?.avg_mae_r) ?? 0,
  avg_end_return_r: parseOptionalFloat(summary?.avg_end_return_r) ?? 0,
  avg_return_1x_r: parseOptionalFloat(summary?.avg_return_1x_r) ?? 0,
  avg_return_2x_r: parseOptionalFloat(summary?.avg_return_2x_r) ?? 0,
  avg_return_3x_r: parseOptionalFloat(summary?.avg_return_3x_r) ?? 0,
  avg_return_5x_r: parseOptionalFloat(summary?.avg_return_5x_r) ?? 0,
  hit_pos_0_5r_rate: parseOptionalFloat(summary?.hit_pos_0_5r_rate) ?? 0,
  hit_pos_1_0r_rate: parseOptionalFloat(summary?.hit_pos_1_0r_rate) ?? 0,
  hit_pos_1_5r_rate: parseOptionalFloat(summary?.hit_pos_1_5r_rate) ?? 0,
  hit_pos_2_0r_rate: parseOptionalFloat(summary?.hit_pos_2_0r_rate) ?? 0,
  hit_neg_0_5r_rate: parseOptionalFloat(summary?.hit_neg_0_5r_rate) ?? 0,
  hit_neg_1_0r_rate: parseOptionalFloat(summary?.hit_neg_1_0r_rate) ?? 0,
  pos_1r_before_neg_1r_rate: parseOptionalFloat(summary?.pos_1r_before_neg_1r_rate) ?? 0,
  avg_pos_1r_bar: parseOptionalFloat(summary?.avg_pos_1r_bar) ?? 0,
  avg_neg_1r_bar: parseOptionalFloat(summary?.avg_neg_1r_bar) ?? 0,
});

const parsePatternDiscoveryBreakdownRow = (row = {}) => ({
  ...row,
  observations: parseOptionalInt(row?.observations) ?? 0,
  avg_mfe_r: parseOptionalFloat(row?.avg_mfe_r) ?? 0,
  avg_mae_r: parseOptionalFloat(row?.avg_mae_r) ?? 0,
  avg_return_3x_r: parseOptionalFloat(row?.avg_return_3x_r) ?? 0,
  avg_return_5x_r: parseOptionalFloat(row?.avg_return_5x_r) ?? 0,
  hit_pos_1_0r_rate: parseOptionalFloat(row?.hit_pos_1_0r_rate) ?? 0,
  hit_neg_1_0r_rate: parseOptionalFloat(row?.hit_neg_1_0r_rate) ?? 0,
  pos_1r_before_neg_1r_rate: parseOptionalFloat(row?.pos_1r_before_neg_1r_rate) ?? 0,
});

const parsePatternDiscoveryLogic = (logic = {}) => ({
  ...logic,
  id: parseOptionalInt(logic?.id) ?? 0,
  rule_json: logic?.rule_json ?? null,
  created_at: logic?.created_at ?? null,
  updated_at: logic?.updated_at ?? null,
});

const parsePatternDiscoveryResponse = (data = null) => {
  if (!data) {
    return null;
  }

  return {
    family: data?.family ?? null,
    summary: parsePatternDiscoverySummary(data?.summary),
    breakdowns: Array.isArray(data?.breakdowns)
      ? data.breakdowns.map((breakdown) => ({
          ...breakdown,
          rows: Array.isArray(breakdown?.rows)
            ? breakdown.rows.map(parsePatternDiscoveryBreakdownRow)
            : [],
        }))
      : [],
    examples: Array.isArray(data?.examples)
      ? data.examples.map((example) => ({
          ...example,
          reference_price: parseOptionalFloat(example?.reference_price) ?? 0,
          mfe_r: parseOptionalFloat(example?.mfe_r),
          mae_r: parseOptionalFloat(example?.mae_r),
          end_close_return_r: parseOptionalFloat(example?.end_close_return_r),
          close_return_3x_r: parseOptionalFloat(example?.close_return_3x_r),
          close_return_5x_r: parseOptionalFloat(example?.close_return_5x_r),
          hit_pos_1_0r_bar: parseOptionalInt(example?.hit_pos_1_0r_bar),
          hit_neg_1_0r_bar: parseOptionalInt(example?.hit_neg_1_0r_bar),
          hit_pos_1r_before_neg_1r:
            example?.hit_pos_1r_before_neg_1r === null ||
            example?.hit_pos_1r_before_neg_1r === undefined
              ? null
              : Boolean(example.hit_pos_1r_before_neg_1r),
        }))
      : [],
    saved_logic: Array.isArray(data?.saved_logic)
      ? data.saved_logic.map(parsePatternDiscoveryLogic)
      : [],
  };
};

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

export const fetchCandleStorageSummary = async ({
  includeStockSymbols = false,
} = {}) => {
  try {
    const data = await postJson('/storage/candle-summary', {
      include_stock_symbols: includeStockSymbols,
    });

    return {
      ...data,
      disk_path: data?.disk_path ?? null,
      disk_total_bytes: parseOptionalInt(data?.disk_total_bytes) ?? 0,
      disk_free_bytes: parseOptionalInt(data?.disk_free_bytes) ?? 0,
      disk_used_bytes: parseOptionalInt(data?.disk_used_bytes) ?? 0,
      disk_free_percent: parseOptionalFloat(data?.disk_free_percent) ?? 0,
      total_rows: parseOptionalInt(data?.total_rows) ?? 0,
      total_bytes: parseOptionalInt(data?.total_bytes) ?? 0,
      bytes_per_row: parseOptionalFloat(data?.bytes_per_row) ?? 0,
      tables: Array.isArray(data?.tables)
        ? data.tables.map((table) => ({
            ...table,
            exact_rows: parseOptionalInt(table?.exact_rows) ?? 0,
            data_bytes: parseOptionalInt(table?.data_bytes) ?? 0,
            index_bytes: parseOptionalInt(table?.index_bytes) ?? 0,
            total_bytes: parseOptionalInt(table?.total_bytes) ?? 0,
            bytes_per_row: parseOptionalFloat(table?.bytes_per_row) ?? 0,
          }))
        : [],
      futures_roots: Array.isArray(data?.futures_roots)
        ? data.futures_roots.map((root) => ({
            ...root,
            contract_count: parseOptionalInt(root?.contract_count) ?? 0,
            candle_count: parseOptionalInt(root?.candle_count) ?? 0,
            estimated_bytes: parseOptionalInt(root?.estimated_bytes) ?? 0,
          }))
        : [],
      engine_tables: Array.isArray(data?.engine_tables)
        ? data.engine_tables.map((table) => ({
            ...table,
            exact_rows: parseOptionalInt(table?.exact_rows) ?? 0,
            data_bytes: parseOptionalInt(table?.data_bytes) ?? 0,
            index_bytes: parseOptionalInt(table?.index_bytes) ?? 0,
            total_bytes: parseOptionalInt(table?.total_bytes) ?? 0,
            bytes_per_row: parseOptionalFloat(table?.bytes_per_row) ?? 0,
          }))
        : [],
      engine_total_rows: parseOptionalInt(data?.engine_total_rows) ?? 0,
      engine_total_bytes: parseOptionalInt(data?.engine_total_bytes) ?? 0,
      engine_bytes_per_row: parseOptionalFloat(data?.engine_bytes_per_row) ?? 0,
      rollup_tables: Array.isArray(data?.rollup_tables)
        ? data.rollup_tables.map((table) => ({
            ...table,
            exact_rows: parseOptionalInt(table?.exact_rows) ?? 0,
            data_bytes: parseOptionalInt(table?.data_bytes) ?? 0,
            index_bytes: parseOptionalInt(table?.index_bytes) ?? 0,
            total_bytes: parseOptionalInt(table?.total_bytes) ?? 0,
            bytes_per_row: parseOptionalFloat(table?.bytes_per_row) ?? 0,
          }))
        : [],
      rollup_total_rows: parseOptionalInt(data?.rollup_total_rows) ?? 0,
      rollup_total_bytes: parseOptionalInt(data?.rollup_total_bytes) ?? 0,
      rollup_bytes_per_row: parseOptionalFloat(data?.rollup_bytes_per_row) ?? 0,
      setup_tables: Array.isArray(data?.setup_tables)
        ? data.setup_tables.map((table) => ({
            ...table,
            exact_rows: parseOptionalInt(table?.exact_rows) ?? 0,
            data_bytes: parseOptionalInt(table?.data_bytes) ?? 0,
            index_bytes: parseOptionalInt(table?.index_bytes) ?? 0,
            total_bytes: parseOptionalInt(table?.total_bytes) ?? 0,
            bytes_per_row: parseOptionalFloat(table?.bytes_per_row) ?? 0,
          }))
        : [],
      setup_total_rows: parseOptionalInt(data?.setup_total_rows) ?? 0,
      setup_total_bytes: parseOptionalInt(data?.setup_total_bytes) ?? 0,
      setup_bytes_per_row: parseOptionalFloat(data?.setup_bytes_per_row) ?? 0,
      setup_roots: Array.isArray(data?.setup_roots)
        ? data.setup_roots.map((root) => ({
            ...root,
            contract_count: parseOptionalInt(root?.contract_count) ?? 0,
            setup_count: parseOptionalInt(root?.setup_count) ?? 0,
            estimated_bytes: parseOptionalInt(root?.estimated_bytes) ?? 0,
          }))
        : [],
      setup_markets: Array.isArray(data?.setup_markets)
        ? data.setup_markets.map((market) => ({
            ...market,
            setup_count: parseOptionalInt(market?.setup_count) ?? 0,
            estimated_bytes: parseOptionalInt(market?.estimated_bytes) ?? 0,
          }))
        : [],
      setup_contracts: Array.isArray(data?.setup_contracts)
        ? data.setup_contracts.map((contract) => ({
            ...contract,
            setup_count: parseOptionalInt(contract?.setup_count) ?? 0,
            estimated_bytes: parseOptionalInt(contract?.estimated_bytes) ?? 0,
          }))
        : [],
      setup_patterns: Array.isArray(data?.setup_patterns)
        ? data.setup_patterns.map((pattern) => ({
            ...pattern,
            setup_count: parseOptionalInt(pattern?.setup_count) ?? 0,
            estimated_bytes: parseOptionalInt(pattern?.estimated_bytes) ?? 0,
          }))
        : [],
    };
  } catch (error) {
    console.error(error);
    return null;
  }
};

export const fetchAdminStatus = async () => {
  try {
    const data = await postJson('/admin/status', {});
    return {
      ...data,
      operations: Array.isArray(data?.operations)
        ? data.operations.map((operation) => ({
            ...operation,
            id: parseOptionalInt(operation?.id) ?? 0,
            duration_ms: parseOptionalInt(operation?.duration_ms),
            exit_code: parseOptionalInt(operation?.exit_code),
          }))
        : [],
      table_snapshots: Array.isArray(data?.table_snapshots)
        ? data.table_snapshots.map((table) => ({
            ...table,
            exact_rows: parseOptionalInt(table?.exact_rows) ?? 0,
            total_bytes: parseOptionalInt(table?.total_bytes) ?? 0,
          }))
        : [],
      engine_phases: Array.isArray(data?.engine_phases)
        ? data.engine_phases.map((phase) => ({
            ...phase,
            row_count: parseOptionalInt(phase?.row_count),
            duration_ms: parseOptionalInt(phase?.duration_ms) ?? 0,
          }))
        : [],
      engine_progress: data?.engine_progress
        ? {
            ...data.engine_progress,
            total_symbols: parseOptionalInt(data.engine_progress?.total_symbols) ?? 0,
            queued_symbols: parseOptionalInt(data.engine_progress?.queued_symbols) ?? 0,
            completed_symbols:
              parseOptionalInt(data.engine_progress?.completed_symbols) ?? 0,
            percent_complete:
              parseOptionalFloat(data.engine_progress?.percent_complete) ?? 0,
            elapsed_ms: parseOptionalInt(data.engine_progress?.elapsed_ms) ?? 0,
            estimated_total_ms: parseOptionalInt(
              data.engine_progress?.estimated_total_ms
            ),
            estimated_remaining_ms: parseOptionalInt(
              data.engine_progress?.estimated_remaining_ms
            ),
          }
        : null,
      cache_states: Array.isArray(data?.cache_states)
        ? data.cache_states.map((cache) => ({
            ...cache,
            is_ready: Boolean(cache?.is_ready),
          }))
        : [],
    };
  } catch (error) {
    console.error(error);
    return null;
  }
};

export const runAdminAction = async (action, options = {}) => {
  try {
    return await postJson('/admin/run-action', {
      action,
      root_symbol: options.rootSymbol ?? null,
      contract_symbol: options.contractSymbol ?? null,
      source_timeframe: options.sourceTimeframe ?? null,
      scan_concurrency: parseOptionalInt(options.scanConcurrency),
      default_fit_only:
        typeof options.defaultFitOnly === 'boolean' ? options.defaultFitOnly : null,
      skip_processed_symbols:
        typeof options.skipProcessedSymbols === 'boolean' ? options.skipProcessedSymbols : null,
      defer_rebuild_indexes:
        typeof options.deferRebuildIndexes === 'boolean' ? options.deferRebuildIndexes : null,
      skip_prop_family_summaries:
        typeof options.skipPropFamilySummaries === 'boolean'
          ? options.skipPropFamilySummaries
          : null,
      confirm_text: options.confirmText ?? null,
    });
  } catch (error) {
    console.error(error);
    return { error: error.message || 'Action failed to start.' };
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

export const fetchSimulatorFamilyReplay = async ({
  familyId,
  firstStartDate,
  testsToChain,
  contracts,
  accountRules,
  drawdownModel,
  oneTradeAtATime,
  useCandidateLogic = false,
}) => {
  if (!familyId) {
    return null;
  }

  try {
    const data = await postJson('/simulator/family-replay', {
      prop_strategy_id: familyId,
      first_start_date: firstStartDate,
      tests_to_chain: testsToChain,
      contracts,
      starting_balance: accountRules?.startingBalance ?? null,
      profit_target: accountRules?.profitTarget ?? null,
      max_drawdown: accountRules?.maxDrawdown ?? null,
      daily_loss_limit: accountRules?.dailyLossLimit ?? null,
      drawdown_model: drawdownModel ?? null,
      one_trade_at_a_time: oneTradeAtATime,
      use_candidate_logic: useCandidateLogic,
    });

    const replay = parseSimulatorReplayResponse(data);
    return {
      ...replay,
      trades: replay.trades.map((trade) => ({
        ...trade,
        prop_outcome_mode: 'phase1-family',
      })),
    };
  } catch (error) {
    console.error(error);
    return null;
  }
};

export const fetchPhase1RouteReplay = async ({
  familyKey,
  runId,
  routeId,
  firstStartDate,
  testsToChain,
  contracts,
  accountRules,
  drawdownModel,
  oneTradeAtATime,
}) => {
  if (!familyKey || !runId || !routeId) {
    return null;
  }

  try {
    const data = await postJson('/simulator/phase1-route-replay', {
      family_key: familyKey,
      run_id: runId,
      route_id: routeId,
      first_start_date: firstStartDate,
      tests_to_chain: testsToChain,
      contracts,
      starting_balance: accountRules?.startingBalance ?? null,
      profit_target: accountRules?.profitTarget ?? null,
      max_drawdown: accountRules?.maxDrawdown ?? null,
      daily_loss_limit: accountRules?.dailyLossLimit ?? null,
      drawdown_model: drawdownModel ?? null,
      one_trade_at_a_time: oneTradeAtATime,
    });

    return parseSimulatorReplayResponse(data);
  } catch (error) {
    console.error(error);
    return null;
  }
};

export const fetchPhase1PatternRouteReplay = async ({
  familyKey,
  runId,
  routeId,
  patternId = null,
  patternGroupId = null,
  contracts = 1,
}) => {
  if (!familyKey || !runId || !routeId || (!patternId && !patternGroupId)) {
    return null;
  }

  try {
    const data = await postJson('/simulator/phase1-pattern-route-replay', {
      family_key: familyKey,
      run_id: runId,
      route_id: routeId,
      pattern_id: patternId,
      pattern_group_id: patternGroupId,
      contracts,
    });

    return parseSimulatorReplayResponse(data);
  } catch (error) {
    console.error(error);
    return null;
  }
};

export const fetchPatternDiscoveryFamily = async ({ familyId, limit = 40 } = {}) => {
  if (!familyId) {
    return null;
  }

  try {
    const data = await postJson('/pattern-discovery/family', {
      prop_strategy_id: familyId,
      limit,
    });
    return parsePatternDiscoveryResponse(data);
  } catch (error) {
    console.error(error);
    return null;
  }
};

export const savePatternDiscoveryLogic = async ({
  familyId,
  title,
  logicText,
  ruleJson = null,
} = {}) => {
  if (!familyId || !logicText) {
    return [];
  }

  try {
    const data = await postJson('/pattern-discovery/save-logic', {
      prop_strategy_id: familyId,
      title,
      logic_text: logicText,
      rule_json: ruleJson,
    });
    return Array.isArray(data) ? data.map(parsePatternDiscoveryLogic) : [];
  } catch (error) {
    console.error(error);
    return [];
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
    prop_strategy_id: strategy.propStrategyId ?? strategy.familyKey ?? strategy.id ?? null,
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
    first_start_date: options?.firstStartDate ?? null,
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
      earliest_entry_date: data?.earliest_entry_date ?? null,
      latest_entry_date: data?.latest_entry_date ?? null,
      entry_dates: Array.isArray(data?.entry_dates) ? data.entry_dates : [],
    };
  } catch (error) {
    console.error(error);
    return {
      patterns: [],
      total_count: 0,
      has_more: false,
      earliest_entry_date: null,
      latest_entry_date: null,
      entry_dates: [],
    };
  }
};

export const fetchPhase1FamilyPatterns = async (
  {
    familyKey = null,
    sourceScope = 'futures',
    year = null,
  } = {},
  pagination = { limit: 500, offset: 0, includeCount: true }
) => {
  if (!familyKey) {
    return {
      patterns: [],
      total_count: 0,
      has_more: false,
      earliest_entry_date: null,
      latest_entry_date: null,
      entry_dates: [],
    };
  }

  try {
    const data = await postJson('/phase1/family-patterns', {
      family_key: familyKey,
      source_scope: sourceScope,
      year: parseOptionalInt(year),
      limit: pagination.limit ?? 500,
      offset: pagination.offset ?? 0,
      include_count: pagination.includeCount ?? true,
    });

    const patterns = Array.isArray(data?.patterns)
      ? data.patterns.map((pattern) =>
          parsePatternSummaryRecord({
            ...pattern,
            prop_outcome_mode: 'phase1-family',
          })
        )
      : [];

    return {
      ...data,
      patterns,
      total_count: data?.total_count ?? patterns.length,
      has_more: Boolean(data?.has_more),
      earliest_entry_date: data?.earliest_entry_date ?? null,
      latest_entry_date: data?.latest_entry_date ?? null,
      entry_dates: Array.isArray(data?.entry_dates) ? data.entry_dates : [],
    };
  } catch (error) {
    console.error(error);
    return {
      patterns: [],
      total_count: 0,
      has_more: false,
      earliest_entry_date: null,
      latest_entry_date: null,
      entry_dates: [],
    };
  }
};

export const fetchAllStrategyTrades = async (
  strategy,
  options = { propMode: false, propOutcomeMode: DEFAULT_PROP_OUTCOME_MODE },
  pageSize = 500
) => {
  const patterns = [];
  let offset = 0;
  let totalCount = 0;
  let earliestEntryDate = null;
  let latestEntryDate = null;
  let entryDates = [];

  while (true) {
    const data = await fetchStrategyTrades(
      strategy,
      {
        limit: pageSize,
        offset,
        includeCount: offset === 0,
      },
      options
    );
    const nextPatterns = Array.isArray(data?.patterns) ? data.patterns : [];

    patterns.push(...nextPatterns);
    if (offset === 0) {
      totalCount = data?.total_count ?? nextPatterns.length;
      earliestEntryDate = data?.earliest_entry_date ?? null;
      latestEntryDate = data?.latest_entry_date ?? null;
      entryDates = Array.isArray(data?.entry_dates) ? data.entry_dates : [];
    }

    if (!data?.has_more || !nextPatterns.length) {
      break;
    }

    offset += nextPatterns.length;
  }

  return {
    patterns,
    total_count: totalCount > 0 ? totalCount : patterns.length,
    has_more: false,
    earliest_entry_date: earliestEntryDate,
    latest_entry_date: latestEntryDate,
    entry_dates: entryDates,
  };
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

  const request = patternSummary.pattern_id
    ? {
        pattern_id: patternSummary.pattern_id,
        pattern_group_id: patternSummary.pattern_group_id ?? '',
        prop_outcome_mode: patternSummary.prop_outcome_mode ?? null,
        x_length: patternSummary.x_length ?? null,
        a_length: patternSummary.a_length ?? null,
        b_length: patternSummary.b_length ?? null,
        c_length: patternSummary.c_length ?? null,
      }
    : {
        symbol: patternSummary.symbol ?? null,
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
  includeCount = false,
  propMode = false,
  propOutcomeMode = DEFAULT_PROP_OUTCOME_MODE,
  sort = null,
  familyFilters = null,
  bestPickFilters = null,
} = {}) => {
  const filter = {
    min_closed_trades: minClosedTrades,
    limit,
    include_count: includeCount,
    prop_mode: Boolean(propMode),
    prop_outcome_mode: propOutcomeMode,
    ...buildFamilyStrategyRequestOptions({ sort, familyFilters }),
    ...buildBestPickRequestOptions(bestPickFilters),
  };

  try {
    const data = await postJson('/strategy-candidates', filter);
    if (Array.isArray(data)) {
      const strategies = data.map(parseStrategyCandidateRecord);
      return {
        strategies,
        total_count: strategies.length,
        has_more: false,
      };
    }

    const strategies = Array.isArray(data?.strategies)
      ? data.strategies.map(parseStrategyCandidateRecord)
      : [];

    return {
      strategies,
      total_count: parseOptionalInt(data?.total_count) ?? strategies.length,
      has_more: Boolean(data?.has_more),
    };
  } catch (error) {
    console.error(error);
    return { strategies: [], total_count: 0, has_more: false };
  }
};

export const fetchPatternFamilies = async ({
  limit = 1000,
  minSetupCount = 1,
  year = null,
  sourceScope = 'all',
} = {}) => {
  try {
    const data = await postJson('/pattern-families', {
      limit,
      min_setup_count: minSetupCount,
      year: parseOptionalInt(year),
      source_scope: sourceScope,
    });

    return Array.isArray(data) ? data.map(parsePatternFamilyRecord) : [];
  } catch (error) {
    console.error(error);
    return [];
  }
};

export const fetchPhase1Results = async ({
  familyKey = null,
  sourceScope = 'futures',
  year = null,
  limit = 250,
} = {}) => {
  if (!familyKey) {
    return [];
  }

  try {
    const data = await postJson('/phase1/results', {
      family_key: familyKey,
      source_scope: sourceScope,
      year: parseOptionalInt(year),
      limit,
    });

    return Array.isArray(data) ? data.map(parsePhase1ResultRecord) : [];
  } catch (error) {
    console.error(error);
    return [];
  }
};

export const fetchPhase1Leaderboard = async ({
  sourceScope = 'futures',
  year = null,
  limit = 500,
  minTradeCount = 100,
  minSetupCount = 1,
  bestPerFamily = false,
  routeId = null,
} = {}) => {
  try {
    const data = await postJson('/phase1/leaderboard', {
      source_scope: sourceScope,
      year: parseOptionalInt(year),
      limit: parseOptionalInt(limit),
      min_trade_count: parseOptionalInt(minTradeCount),
      min_setup_count: parseOptionalInt(minSetupCount),
      best_per_family: Boolean(bestPerFamily),
      route_id: routeId,
    });

    return Array.isArray(data) ? data.map(parsePhase1ResultRecord) : [];
  } catch (error) {
    console.error(error);
    return [];
  }
};

export const fetchPhase1Supply = async ({
  sourceScope = 'futures',
  year = null,
  limit = 100,
} = {}) => {
  try {
    const data = await postJson('/phase1/supply', {
      source_scope: sourceScope,
      year: parseOptionalInt(year),
      limit: parseOptionalInt(limit),
    });

    return {
      source_scope: data?.source_scope ?? sourceScope,
      period_year: parseOptionalInt(data?.period_year) ?? 0,
      symbols: Array.isArray(data?.symbols)
        ? data.symbols.map(parsePhase1SupplySymbolRecord)
        : [],
      families: Array.isArray(data?.families)
        ? data.families.map(parsePhase1SupplyFamilyRecord)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { source_scope: sourceScope, period_year: 0, symbols: [], families: [] };
  }
};

export const fetchPhase1YearlyBreakdown = async ({
  familyKey = null,
  runId = null,
  routeId = null,
  cacheOnly = false,
} = {}) => {
  if (!familyKey || !runId || !routeId) {
    return null;
  }

  try {
    const data = await postJson('/phase1/yearly-breakdown', {
      family_key: familyKey,
      run_id: runId,
      route_id: routeId,
      cache_only: Boolean(cacheOnly),
    });

    return parsePhase1YearlyBreakdown(data);
  } catch (error) {
    console.error(error);
    return null;
  }
};

export const get_candles = getCandles;
export const get_support_resistance_lines = getSupportResistanceLines;
