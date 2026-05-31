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

const parseBooleanFlag = (value) =>
  value === true || value === 1 || value === '1' || value === 'true';

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
  event_id: pattern.event_id ?? null,
  event_rank: parseOptionalInt(pattern.event_rank),
  is_event_primary:
    pattern.is_event_primary === null || pattern.is_event_primary === undefined
      ? null
      : parseBooleanFlag(pattern.is_event_primary),
  event_sister_count: parseOptionalInt(pattern.event_sister_count),
  event_similarity_score: parseOptionalFloat(pattern.event_similarity_score),
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
  nextPattern.event_id = nextPattern.event_id ?? null;
  nextPattern.event_rank = parseOptionalInt(nextPattern.event_rank);
  nextPattern.is_event_primary =
    nextPattern.is_event_primary === null || nextPattern.is_event_primary === undefined
      ? null
      : parseBooleanFlag(nextPattern.is_event_primary);
  nextPattern.event_sister_count = parseOptionalInt(nextPattern.event_sister_count);
  nextPattern.event_similarity_score = parseOptionalFloat(nextPattern.event_similarity_score);
  nextPattern.twin_pattern_ids = Array.isArray(nextPattern.twin_pattern_ids)
    ? nextPattern.twin_pattern_ids.filter(Boolean)
    : typeof nextPattern.twin_pattern_ids === 'string'
      ? nextPattern.twin_pattern_ids.split('|').map((value) => value.trim()).filter(Boolean)
      : [];

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

const parseEntryExitTemplateRecord = (row = {}) => ({
  ...row,
  risk_multiple: parseOptionalFloat(row?.risk_multiple) ?? 0,
  target_r: parseOptionalFloat(row?.target_r) ?? 0,
  max_hold_multiple: parseOptionalInt(row?.max_hold_multiple) ?? 0,
  first_result_r: parseOptionalFloat(row?.first_result_r),
  eval_count: parseOptionalInt(row?.eval_count) ?? 0,
  pass_count: parseOptionalInt(row?.pass_count) ?? 0,
  fail_count: parseOptionalInt(row?.fail_count) ?? 0,
  no_entry_count: parseOptionalInt(row?.no_entry_count) ?? 0,
  avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
  bullish_eval_count: parseOptionalInt(row?.bullish_eval_count) ?? 0,
  bullish_pass_count: parseOptionalInt(row?.bullish_pass_count) ?? 0,
  bullish_fail_count: parseOptionalInt(row?.bullish_fail_count) ?? 0,
  bullish_no_entry_count: parseOptionalInt(row?.bullish_no_entry_count) ?? 0,
  bullish_avg_r: parseOptionalFloat(row?.bullish_avg_r) ?? 0,
  bearish_eval_count: parseOptionalInt(row?.bearish_eval_count) ?? 0,
  bearish_pass_count: parseOptionalInt(row?.bearish_pass_count) ?? 0,
  bearish_fail_count: parseOptionalInt(row?.bearish_fail_count) ?? 0,
  bearish_no_entry_count: parseOptionalInt(row?.bearish_no_entry_count) ?? 0,
  bearish_avg_r: parseOptionalFloat(row?.bearish_avg_r) ?? 0,
  market_edge_label: row?.market_edge_label ?? 'Flat',
  market_edge_score: parseOptionalFloat(row?.market_edge_score) ?? 0,
});

const parseEntryExitTemplateMarketBreakdown = (row = {}) => ({
  ...row,
  eval_count: parseOptionalInt(row?.eval_count) ?? 0,
  pass_count: parseOptionalInt(row?.pass_count) ?? 0,
  fail_count: parseOptionalInt(row?.fail_count) ?? 0,
  no_entry_count: parseOptionalInt(row?.no_entry_count) ?? 0,
  avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
  best_r: parseOptionalFloat(row?.best_r) ?? 0,
  worst_r: parseOptionalFloat(row?.worst_r) ?? 0,
});

const parseEntryExitTemplateConditionBreakdown = (row = {}) => ({
  ...parseEntryExitTemplateMarketBreakdown(row),
  condition_type: row?.condition_type ?? 'unknown',
  condition_value: row?.condition_value ?? 'Unknown',
});

const parseEntryExitTemplateComboBreakdown = (row = {}) => ({
  ...parseEntryExitTemplateMarketBreakdown(row),
  feature_a_type: row?.feature_a_type ?? 'unknown',
  feature_a_value: row?.feature_a_value ?? 'Unknown',
  feature_b_type: row?.feature_b_type ?? 'unknown',
  feature_b_value: row?.feature_b_value ?? 'Unknown',
});

const parseEntryExitTemplateFamilyBreakdown = (row = {}) => ({
  ...parseEntryExitTemplateMarketBreakdown(row),
  family_key: row?.family_key ?? 'Unknown',
  harmonic_type: row?.harmonic_type ?? 'Unknown',
  market: row?.market ?? 'Unknown',
  family_bin: row?.family_bin ?? 'Unknown',
  family_size_bucket: row?.family_size_bucket ?? 'Unknown',
  family_time_bin: row?.family_time_bin ?? 'Unknown',
  family_x_strictness: row?.family_x_strictness ?? 'Unknown',
});

const parseEntryExitTemplateRun = (run = null) =>
  run
    ? {
        ...run,
        period_year: parseOptionalInt(run?.period_year) ?? 0,
        requested_limit: parseOptionalInt(run?.requested_limit) ?? 0,
        scanned_patterns: parseOptionalInt(run?.scanned_patterns) ?? 0,
        templates_created: parseOptionalInt(run?.templates_created) ?? 0,
        existing_template_passes: parseOptionalInt(run?.existing_template_passes) ?? 0,
        failed_to_create: parseOptionalInt(run?.failed_to_create) ?? 0,
        result_rows: parseOptionalInt(run?.result_rows) ?? 0,
        elapsed_ms: parseOptionalInt(run?.elapsed_ms) ?? 0,
      }
    : null;

const parseEntryExitTemplateBuildSummary = (summary = null) =>
  summary
    ? {
        ...summary,
        scan_year_start: parseOptionalInt(summary?.scan_year_start),
        scan_year_end: parseOptionalInt(summary?.scan_year_end),
        patterns_scanned: parseOptionalInt(summary?.patterns_scanned) ?? 0,
        templates_created: parseOptionalInt(summary?.templates_created) ?? 0,
        coverage_patterns: parseOptionalInt(summary?.coverage_patterns) ?? 0,
        root_count: parseOptionalInt(summary?.root_count) ?? 0,
        exchange_count: parseOptionalInt(summary?.exchange_count) ?? 0,
        requested_limit: parseOptionalInt(summary?.requested_limit) ?? 0,
        result_rows: parseOptionalInt(summary?.result_rows) ?? 0,
        elapsed_ms: parseOptionalInt(summary?.elapsed_ms) ?? 0,
      }
    : null;

const parseEntryExitTemplateCoverageRow = (row = {}) => ({
  ...row,
  test_id: row?.test_id ?? null,
  run_id: row?.run_id ?? null,
  root_symbol: row?.root_symbol ?? 'Unknown',
  source_scope: row?.source_scope ?? null,
  exchange_name: row?.exchange_name ?? row?.exchange ?? null,
  contract_symbol: row?.contract_symbol ?? row?.root_symbol ?? 'Unknown',
  source_timeframe: row?.source_timeframe ?? 'unknown',
  pattern_count: parseOptionalInt(row?.scanned_pattern_count ?? row?.pattern_count) ?? 0,
  scanned_pattern_count: parseOptionalInt(row?.scanned_pattern_count ?? row?.pattern_count) ?? 0,
  universe_pattern_count: parseOptionalInt(row?.universe_pattern_count) ?? 0,
  contract_count: parseOptionalInt(row?.contract_count),
  status: row?.status ?? null,
  sort_order: parseOptionalInt(row?.sort_order),
  first_d_confirm_date: row?.first_d_confirm_date ?? null,
  last_d_confirm_date: row?.last_d_confirm_date ?? null,
});

const parseEntryExitRouterPropStats = (stats = {}) => ({
  profit_target_r: parseOptionalFloat(stats?.profit_target_r) ?? 0,
  max_drawdown_r_limit: parseOptionalFloat(stats?.max_drawdown_r_limit) ?? 0,
  daily_loss_r_limit: parseOptionalFloat(stats?.daily_loss_r_limit) ?? 0,
  sim_plays_used: parseOptionalInt(stats?.sim_plays_used),
  cycles: parseOptionalInt(stats?.cycles) ?? 0,
  passed: parseOptionalInt(stats?.passed) ?? 0,
  daily_fails: parseOptionalInt(stats?.daily_fails) ?? 0,
  drawdown_fails: parseOptionalInt(stats?.drawdown_fails) ?? 0,
  incomplete: parseOptionalInt(stats?.incomplete) ?? 0,
  pass_rate: parseOptionalFloat(stats?.pass_rate) ?? 0,
  closed_pass_rate: parseOptionalFloat(stats?.closed_pass_rate) ?? 0,
  max_drawdown_r: parseOptionalFloat(stats?.max_drawdown_r) ?? 0,
  max_loss_streak: parseOptionalInt(stats?.max_loss_streak) ?? 0,
});

const parseEntryExitRouterRunRecord = (run = null) =>
  run
    ? {
        ...run,
        test_year: parseOptionalInt(run?.test_year) ?? 0,
        min_train_tests: parseOptionalInt(run?.min_train_tests) ?? 0,
        sister_window_minutes: parseOptionalInt(run?.sister_window_minutes) ?? 0,
        families_selected: parseOptionalInt(run?.families_selected) ?? 0,
        patterns_scanned: parseOptionalInt(run?.patterns_scanned) ?? 0,
        routed_patterns: parseOptionalInt(run?.routed_patterns) ?? 0,
        no_route_patterns: parseOptionalInt(run?.no_route_patterns) ?? 0,
        skipped_non_trade_patterns: parseOptionalInt(run?.skipped_non_trade_patterns) ?? 0,
        skipped_symbol_patterns: parseOptionalInt(run?.skipped_symbol_patterns) ?? 0,
        skipped_overlap_patterns: parseOptionalInt(run?.skipped_overlap_patterns) ?? 0,
        trade_choices: parseOptionalInt(run?.trade_choices) ?? 0,
        watchlist_choices: parseOptionalInt(run?.watchlist_choices) ?? 0,
        skip_choices: parseOptionalInt(run?.skip_choices) ?? 0,
        manual_family_bans_applied: parseOptionalInt(run?.manual_family_bans_applied) ?? 0,
        symbol_filter_enabled: parseBooleanFlag(run?.symbol_filter_enabled),
        one_trade_at_a_time: parseBooleanFlag(run?.one_trade_at_a_time),
        one_trade_per_root_symbol: parseBooleanFlag(run?.one_trade_per_root_symbol),
        one_trade_per_minute: parseBooleanFlag(run?.one_trade_per_minute),
        trade_cooldown_minutes: parseOptionalInt(run?.trade_cooldown_minutes) ?? 0,
        daily_loss_lockout: parseBooleanFlag(run?.daily_loss_lockout),
        near_pass_protection: parseBooleanFlag(run?.near_pass_protection),
        near_pass_within_r: parseOptionalFloat(run?.near_pass_within_r) ?? 0,
        near_pass_daily_loss_r: parseOptionalFloat(run?.near_pass_daily_loss_r) ?? 0,
        loss_cluster_day_lockout: parseBooleanFlag(run?.loss_cluster_day_lockout),
        loss_cluster_loss_count: parseOptionalInt(run?.loss_cluster_loss_count) ?? 0,
        loss_cluster_window_minutes: parseOptionalInt(run?.loss_cluster_window_minutes) ?? 0,
        playbook_description: typeof run?.playbook_description === 'string' ? run.playbook_description : '',
        symbol_trade_roots: parseOptionalInt(run?.symbol_trade_roots) ?? 0,
        symbol_skip_roots: parseOptionalInt(run?.symbol_skip_roots) ?? 0,
        symbol_min_tests: parseOptionalInt(run?.symbol_min_tests) ?? 0,
        symbol_min_win_rate: parseOptionalFloat(run?.symbol_min_win_rate) ?? 0,
        symbol_min_avg_r: parseOptionalFloat(run?.symbol_min_avg_r) ?? 0,
        prop_filter_enabled: parseBooleanFlag(run?.prop_filter_enabled),
        trade_min_tests: parseOptionalInt(run?.trade_min_tests) ?? 0,
        trade_min_win_rate: parseOptionalFloat(run?.trade_min_win_rate) ?? 0,
        trade_min_avg_r: parseOptionalFloat(run?.trade_min_avg_r) ?? 0,
        watchlist_min_tests: parseOptionalInt(run?.watchlist_min_tests) ?? 0,
        watchlist_min_win_rate: parseOptionalFloat(run?.watchlist_min_win_rate) ?? 0,
        watchlist_min_avg_r: parseOptionalFloat(run?.watchlist_min_avg_r) ?? 0,
        win_count: parseOptionalInt(run?.win_count) ?? 0,
        loss_count: parseOptionalInt(run?.loss_count) ?? 0,
        no_entry_count: parseOptionalInt(run?.no_entry_count) ?? 0,
        trade_win_rate: parseOptionalFloat(run?.trade_win_rate),
        avg_r: parseOptionalFloat(run?.avg_r) ?? 0,
        sum_r: parseOptionalFloat(run?.sum_r) ?? 0,
        best_r: parseOptionalFloat(run?.best_r) ?? 0,
        worst_r: parseOptionalFloat(run?.worst_r) ?? 0,
        elapsed_ms: parseOptionalInt(run?.elapsed_ms) ?? 0,
        prop: parseEntryExitRouterPropStats(run?.prop ?? {}),
      }
    : null;

const parseEntryExitRouterSymbolRecord = (row = {}) => ({
  ...row,
  train_eval_count: parseOptionalInt(row?.train_eval_count) ?? 0,
  train_pass_count: parseOptionalInt(row?.train_pass_count) ?? 0,
  train_fail_count: parseOptionalInt(row?.train_fail_count) ?? 0,
  train_no_entry_count: parseOptionalInt(row?.train_no_entry_count) ?? 0,
  train_win_rate: parseOptionalFloat(row?.train_win_rate) ?? 0,
  train_fail_rate: parseOptionalFloat(row?.train_fail_rate) ?? 0,
  train_avg_r: parseOptionalFloat(row?.train_avg_r) ?? 0,
  train_sum_r: parseOptionalFloat(row?.train_sum_r) ?? 0,
});

const parseEntryExitRouterFamilyRouteRecord = (row = {}) => ({
  ...row,
  template_rank: parseOptionalInt(row?.template_rank) ?? 0,
  score: parseOptionalFloat(row?.score) ?? 0,
  train_eval_count: parseOptionalInt(row?.train_eval_count) ?? 0,
  train_pass_count: parseOptionalInt(row?.train_pass_count) ?? 0,
  train_fail_count: parseOptionalInt(row?.train_fail_count) ?? 0,
  train_no_entry_count: parseOptionalInt(row?.train_no_entry_count) ?? 0,
  train_avg_r: parseOptionalFloat(row?.train_avg_r) ?? 0,
  train_win_rate: parseOptionalFloat(row?.train_win_rate) ?? 0,
  train_fail_rate: parseOptionalFloat(row?.train_fail_rate) ?? 0,
  test_eval_count: parseOptionalInt(row?.test_eval_count) ?? 0,
  test_pass_count: parseOptionalInt(row?.test_pass_count) ?? 0,
  test_fail_count: parseOptionalInt(row?.test_fail_count) ?? 0,
  test_no_entry_count: parseOptionalInt(row?.test_no_entry_count) ?? 0,
  test_avg_r: parseOptionalFloat(row?.test_avg_r) ?? 0,
  test_sum_r: parseOptionalFloat(row?.test_sum_r) ?? 0,
  test_best_r: parseOptionalFloat(row?.test_best_r) ?? 0,
  test_worst_r: parseOptionalFloat(row?.test_worst_r) ?? 0,
});

const parseEntryExitSimTemplatePerformanceRecord = (row = {}) => ({
  ...row,
  eval_count: parseOptionalInt(row?.eval_count) ?? 0,
  pass_count: parseOptionalInt(row?.pass_count) ?? 0,
  fail_count: parseOptionalInt(row?.fail_count) ?? 0,
  no_entry_count: parseOptionalInt(row?.no_entry_count) ?? 0,
  win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
  best_r: parseOptionalFloat(row?.best_r) ?? 0,
  worst_r: parseOptionalFloat(row?.worst_r) ?? 0,
  daily_loss_day_trades: parseOptionalInt(row?.daily_loss_day_trades) ?? 0,
  daily_loss_day_count: parseOptionalInt(row?.daily_loss_day_count) ?? 0,
  family_count: parseOptionalInt(row?.family_count) ?? 0,
  symbol_count: parseOptionalInt(row?.symbol_count) ?? 0,
  contract_count: parseOptionalInt(row?.contract_count) ?? 0,
});

const parseEntryExitSimEquityPoint = (row = {}) => ({
  ...row,
  point_index: parseOptionalInt(row?.point_index) ?? 0,
  result_r: parseOptionalFloat(row?.result_r) ?? 0,
  cumulative_r: parseOptionalFloat(row?.cumulative_r) ?? 0,
  drawdown_r: parseOptionalFloat(row?.drawdown_r) ?? 0,
  daily_r: parseOptionalFloat(row?.daily_r) ?? 0,
});

const parseEntryExitSimDailyRRow = (row = {}) => ({
  ...row,
  total_r: parseOptionalFloat(row?.total_r) ?? 0,
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  best_trade_r: parseOptionalFloat(row?.best_trade_r) ?? 0,
  worst_trade_r: parseOptionalFloat(row?.worst_trade_r) ?? 0,
  worst_intraday_r: parseOptionalFloat(row?.worst_intraday_r) ?? 0,
  hit_daily_loss: Boolean(parseOptionalInt(row?.hit_daily_loss) ?? row?.hit_daily_loss),
  tp_progress_pct: parseOptionalFloat(row?.tp_progress_pct) ?? 0,
  drawdown_progress_pct: parseOptionalFloat(row?.drawdown_progress_pct) ?? 0,
});

const parseEntryExitSimDailyTradeRow = (row = {}) => ({
  ...row,
  id: parseOptionalInt(row?.id) ?? 0,
  result_r: parseOptionalFloat(row?.result_r) ?? 0,
  family_key: row?.family_key ?? '',
  template_uid: row?.template_uid ?? '',
  template_label: row?.template_label ?? '',
  symbol: row?.symbol ?? '',
  source_timeframe: row?.source_timeframe ?? '',
  market: row?.market ?? '',
  outcome: row?.outcome ?? '',
  exit_reason: row?.exit_reason ?? '',
  trade_direction: row?.trade_direction ?? null,
  cycle_number: parseOptionalInt(row?.cycle_number),
  cycle_equity_r_before: parseOptionalFloat(row?.cycle_equity_r_before),
  cycle_equity_r_after: parseOptionalFloat(row?.cycle_equity_r_after),
  cycle_drawdown_r_before: parseOptionalFloat(row?.cycle_drawdown_r_before),
  cycle_drawdown_r_after: parseOptionalFloat(row?.cycle_drawdown_r_after),
  tp_progress_pct_before: parseOptionalFloat(row?.tp_progress_pct_before),
  tp_progress_pct_after: parseOptionalFloat(row?.tp_progress_pct_after),
  tp_progress_pct_delta: parseOptionalFloat(row?.tp_progress_pct_delta),
  drawdown_progress_pct_before: parseOptionalFloat(row?.drawdown_progress_pct_before),
  drawdown_progress_pct_after: parseOptionalFloat(row?.drawdown_progress_pct_after),
  drawdown_progress_pct_delta: parseOptionalFloat(row?.drawdown_progress_pct_delta),
});

const parseEntryExitSimRawTradeRow = (row = {}) => ({
  ...parseEntryExitSimDailyTradeRow(row),
  template_name: row?.template_name ?? '',
  entry_kind: row?.entry_kind ?? '',
  direction_mode: row?.direction_mode ?? '',
  risk_basis: row?.risk_basis ?? '',
  risk_multiple: parseOptionalFloat(row?.risk_multiple) ?? 0,
  target_r: parseOptionalFloat(row?.target_r) ?? 0,
  max_hold_multiple: parseOptionalInt(row?.max_hold_multiple) ?? 0,
  rule_json: row?.rule_json ?? '',
  pattern_group_id: row?.pattern_group_id ?? '',
  event_id: row?.event_id ?? null,
  event_rank: parseOptionalInt(row?.event_rank),
  event_sister_count: parseOptionalInt(row?.event_sister_count) ?? 0,
  event_candidate_count: parseOptionalInt(row?.event_candidate_count) ?? 0,
  event_live_candidate_count: parseOptionalInt(row?.event_live_candidate_count) ?? 0,
  duration_minutes: parseOptionalFloat(row?.duration_minutes),
  entry_price: parseOptionalFloat(row?.entry_price),
  stop_price: parseOptionalFloat(row?.stop_price),
  target_price: parseOptionalFloat(row?.target_price),
  exit_price: parseOptionalFloat(row?.exit_price),
  risk_points: parseOptionalFloat(row?.risk_points),
});

const parseEntryExitSimHourlyRow = (row = {}) => ({
  ...row,
  entry_hour: parseOptionalInt(row?.entry_hour) ?? 0,
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
  best_r: parseOptionalFloat(row?.best_r) ?? 0,
  worst_r: parseOptionalFloat(row?.worst_r) ?? 0,
  daily_loss_day_trades: parseOptionalInt(row?.daily_loss_day_trades) ?? 0,
  daily_loss_day_count: parseOptionalInt(row?.daily_loss_day_count) ?? 0,
});

const parseEntryExitSimTradeCadence = (row = null) => {
  if (!row) {
    return null;
  }

  return {
    ...row,
    trades: parseOptionalInt(row?.trades) ?? 0,
    active_hours: parseOptionalInt(row?.active_hours) ?? 0,
    trade_days: parseOptionalInt(row?.trade_days) ?? 0,
    active_weeks: parseOptionalInt(row?.active_weeks) ?? 0,
    active_months: parseOptionalInt(row?.active_months) ?? 0,
    gap_count: parseOptionalInt(row?.gap_count) ?? 0,
    avg_gap_minutes: parseOptionalFloat(row?.avg_gap_minutes) ?? 0,
    median_gap_minutes: parseOptionalFloat(row?.median_gap_minutes) ?? 0,
    min_gap_minutes: parseOptionalFloat(row?.min_gap_minutes) ?? 0,
    max_gap_minutes: parseOptionalFloat(row?.max_gap_minutes) ?? 0,
    avg_trades_per_hour: parseOptionalFloat(row?.avg_trades_per_hour) ?? 0,
    avg_trades_per_day: parseOptionalFloat(row?.avg_trades_per_day) ?? 0,
    avg_trades_per_week: parseOptionalFloat(row?.avg_trades_per_week) ?? 0,
    avg_trades_per_month: parseOptionalFloat(row?.avg_trades_per_month) ?? 0,
    max_trades_per_day: parseOptionalInt(row?.max_trades_per_day) ?? 0,
    max_trades_per_hour: parseOptionalInt(row?.max_trades_per_hour) ?? 0,
    max_trades_per_week: parseOptionalInt(row?.max_trades_per_week) ?? 0,
    max_trades_per_month: parseOptionalInt(row?.max_trades_per_month) ?? 0,
    hours_over_5_trades: parseOptionalInt(row?.hours_over_5_trades) ?? 0,
    days_over_20_trades: parseOptionalInt(row?.days_over_20_trades) ?? 0,
    max_trades_5m_window: parseOptionalInt(row?.max_trades_5m_window) ?? 0,
    max_trades_15m_window: parseOptionalInt(row?.max_trades_15m_window) ?? 0,
    gap_0_1m: parseOptionalInt(row?.gap_0_1m) ?? 0,
    gap_1_5m: parseOptionalInt(row?.gap_1_5m) ?? 0,
    gap_5_15m: parseOptionalInt(row?.gap_5_15m) ?? 0,
    gap_15_30m: parseOptionalInt(row?.gap_15_30m) ?? 0,
    gap_30_60m: parseOptionalInt(row?.gap_30_60m) ?? 0,
    gap_over_60m: parseOptionalInt(row?.gap_over_60m) ?? 0,
  };
};

const parseEntryExitSimTradeGapRow = (row = {}) => ({
  ...row,
  sequence_number: parseOptionalInt(row?.sequence_number) ?? 0,
  gap_minutes: parseOptionalFloat(row?.gap_minutes) ?? 0,
  bucket_key: row?.bucket_key ?? '',
  bucket_label: row?.bucket_label ?? '',
  previous_cycle_number: parseOptionalInt(row?.previous_cycle_number) ?? 0,
  cycle_number: parseOptionalInt(row?.cycle_number) ?? 0,
  starts_new_cycle: Boolean(parseOptionalInt(row?.starts_new_cycle) ?? row?.starts_new_cycle),
});

const parseEntryExitSimTradeWorkload = (row = null) => {
  if (!row) {
    return null;
  }

  return {
    ...row,
    trades: parseOptionalInt(row?.trades) ?? 0,
    active_hours: parseOptionalInt(row?.active_hours) ?? 0,
    active_days: parseOptionalInt(row?.active_days) ?? 0,
    active_weeks: parseOptionalInt(row?.active_weeks) ?? 0,
    active_months: parseOptionalInt(row?.active_months) ?? 0,
    avg_trades_per_hour: parseOptionalFloat(row?.avg_trades_per_hour) ?? 0,
    avg_trades_per_day: parseOptionalFloat(row?.avg_trades_per_day) ?? 0,
    avg_trades_per_week: parseOptionalFloat(row?.avg_trades_per_week) ?? 0,
    avg_trades_per_month: parseOptionalFloat(row?.avg_trades_per_month) ?? 0,
    min_trades_per_day: parseOptionalInt(row?.min_trades_per_day) ?? 0,
    max_trades_per_hour: parseOptionalInt(row?.max_trades_per_hour) ?? 0,
    max_trades_per_day: parseOptionalInt(row?.max_trades_per_day) ?? 0,
    max_trades_per_week: parseOptionalInt(row?.max_trades_per_week) ?? 0,
    max_trades_per_month: parseOptionalInt(row?.max_trades_per_month) ?? 0,
    hours_over_5_trades: parseOptionalInt(row?.hours_over_5_trades) ?? 0,
    days_over_20_trades: parseOptionalInt(row?.days_over_20_trades) ?? 0,
  };
};

const parseEntryExitSimTestFrequencyRow = (row = {}) => ({
  ...row,
  cycle_number: parseOptionalInt(row?.cycle_number) ?? 0,
  duration_minutes: parseOptionalFloat(row?.duration_minutes) ?? 0,
  calendar_days: parseOptionalInt(row?.calendar_days) ?? 0,
  active_trade_days: parseOptionalInt(row?.active_trade_days) ?? 0,
  events: parseOptionalInt(row?.events) ?? 0,
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
  avg_trades_per_calendar_day: parseOptionalFloat(row?.avg_trades_per_calendar_day) ?? 0,
  avg_trades_per_active_day: parseOptionalFloat(row?.avg_trades_per_active_day) ?? 0,
  avg_trades_per_hour: parseOptionalFloat(row?.avg_trades_per_hour) ?? 0,
  max_drawdown_r: parseOptionalFloat(row?.max_drawdown_r) ?? 0,
  worst_day_r: parseOptionalFloat(row?.worst_day_r) ?? 0,
});

const parseEntryExitBuildRawRow = (row = {}) => ({
  ...row,
  id: parseOptionalInt(row?.id) ?? 0,
  event_rank: parseOptionalInt(row?.event_rank),
  event_sister_count: parseOptionalInt(row?.event_sister_count) ?? 0,
  evaluation_order: parseOptionalInt(row?.evaluation_order) ?? 0,
  was_created_for_setup: Boolean(parseOptionalInt(row?.was_created_for_setup) ?? row?.was_created_for_setup),
  result_r: parseOptionalFloat(row?.result_r),
  entry_price: parseOptionalFloat(row?.entry_price),
  stop_price: parseOptionalFloat(row?.stop_price),
  target_price: parseOptionalFloat(row?.target_price),
  exit_price: parseOptionalFloat(row?.exit_price),
  risk_points: parseOptionalFloat(row?.risk_points),
});

const parseEntryExitDayTradingSummary = (row = null) => {
  if (!row) {
    return null;
  }

  return {
    ...row,
    starting_equity_r: parseOptionalFloat(row?.starting_equity_r) ?? 0,
    ending_equity_r: parseOptionalFloat(row?.ending_equity_r) ?? 0,
    peak_equity_r: parseOptionalFloat(row?.peak_equity_r) ?? 0,
    max_drawdown_r: parseOptionalFloat(row?.max_drawdown_r) ?? 0,
    total_trades: parseOptionalInt(row?.total_trades) ?? 0,
    wins: parseOptionalInt(row?.wins) ?? 0,
    losses: parseOptionalInt(row?.losses) ?? 0,
    no_entries: parseOptionalInt(row?.no_entries) ?? 0,
    win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
    avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
    total_r: parseOptionalFloat(row?.total_r) ?? 0,
    gross_profit_r: parseOptionalFloat(row?.gross_profit_r) ?? 0,
    gross_loss_r: parseOptionalFloat(row?.gross_loss_r) ?? 0,
    profit_factor: parseOptionalFloat(row?.profit_factor) ?? 0,
    best_trade_r: parseOptionalFloat(row?.best_trade_r) ?? 0,
    worst_trade_r: parseOptionalFloat(row?.worst_trade_r) ?? 0,
    best_day_r: parseOptionalFloat(row?.best_day_r) ?? 0,
    worst_day_r: parseOptionalFloat(row?.worst_day_r) ?? 0,
    trading_days: parseOptionalInt(row?.trading_days) ?? 0,
    profitable_days: parseOptionalInt(row?.profitable_days) ?? 0,
    losing_days: parseOptionalInt(row?.losing_days) ?? 0,
    avg_day_r: parseOptionalFloat(row?.avg_day_r) ?? 0,
    max_trades_per_day: parseOptionalInt(row?.max_trades_per_day) ?? 0,
    avg_trade_duration_minutes: parseOptionalFloat(row?.avg_trade_duration_minutes) ?? 0,
    median_trade_duration_minutes: parseOptionalFloat(row?.median_trade_duration_minutes) ?? 0,
    longest_trade_duration_minutes: parseOptionalFloat(row?.longest_trade_duration_minutes) ?? 0,
  };
};

const parseEntryExitDayTradingDailyRow = (row = {}) => ({
  ...row,
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  gross_profit_r: parseOptionalFloat(row?.gross_profit_r) ?? 0,
  gross_loss_r: parseOptionalFloat(row?.gross_loss_r) ?? 0,
  net_r: parseOptionalFloat(row?.net_r) ?? 0,
  end_equity_r: parseOptionalFloat(row?.end_equity_r) ?? 0,
  intraday_drawdown_r: parseOptionalFloat(row?.intraday_drawdown_r) ?? 0,
  best_trade_r: parseOptionalFloat(row?.best_trade_r) ?? 0,
  worst_trade_r: parseOptionalFloat(row?.worst_trade_r) ?? 0,
});

const parseEntryExitDayTradingMonthlyRow = (row = {}) => ({
  ...row,
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  gross_profit_r: parseOptionalFloat(row?.gross_profit_r) ?? 0,
  gross_loss_r: parseOptionalFloat(row?.gross_loss_r) ?? 0,
  net_r: parseOptionalFloat(row?.net_r) ?? 0,
  end_equity_r: parseOptionalFloat(row?.end_equity_r) ?? 0,
  max_drawdown_r: parseOptionalFloat(row?.max_drawdown_r) ?? 0,
});

const parseEntryExitSimMarketTrendPerformanceRow = (row = {}) => ({
  ...row,
  timeframe: row?.timeframe ?? '',
  trend_label: row?.trend_label ?? '',
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
  best_r: parseOptionalFloat(row?.best_r) ?? 0,
  worst_r: parseOptionalFloat(row?.worst_r) ?? 0,
  symbol_count: parseOptionalInt(row?.symbol_count) ?? 0,
  avg_strength_pct: parseOptionalFloat(row?.avg_strength_pct) ?? 0,
});

const parseEntryExitSimMarketTrendAlignmentRow = (row = {}) => ({
  ...row,
  trend_5m: row?.trend_5m ?? '',
  trend_15m: row?.trend_15m ?? '',
  trend_1h: row?.trend_1h ?? '',
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
  best_r: parseOptionalFloat(row?.best_r) ?? 0,
  worst_r: parseOptionalFloat(row?.worst_r) ?? 0,
});

const parseEntryExitSimMarketTrendDirectionRow = (row = {}) => ({
  ...row,
  timeframe: row?.timeframe ?? '',
  trade_direction: row?.trade_direction ?? '',
  trend_label: row?.trend_label ?? '',
  trend_alignment: row?.trend_alignment ?? '',
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
  best_r: parseOptionalFloat(row?.best_r) ?? 0,
  worst_r: parseOptionalFloat(row?.worst_r) ?? 0,
});

const parseEntryExitSimSymbolContributionRow = (row = {}) => ({
  ...row,
  root_symbol: row?.root_symbol ?? '',
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
  best_r: parseOptionalFloat(row?.best_r) ?? 0,
  worst_r: parseOptionalFloat(row?.worst_r) ?? 0,
  daily_loss_day_trades: parseOptionalInt(row?.daily_loss_day_trades) ?? 0,
  daily_loss_day_count: parseOptionalInt(row?.daily_loss_day_count) ?? 0,
  family_count: parseOptionalInt(row?.family_count) ?? 0,
  template_count: parseOptionalInt(row?.template_count) ?? 0,
  contract_count: parseOptionalInt(row?.contract_count) ?? 0,
});

const parseEntryExitSimFamilyContributionRow = (row = {}) => ({
  ...row,
  family_key: row?.family_key ?? '',
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row?.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row?.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
  best_r: parseOptionalFloat(row?.best_r) ?? 0,
  worst_r: parseOptionalFloat(row?.worst_r) ?? 0,
  daily_loss_day_trades: parseOptionalInt(row?.daily_loss_day_trades) ?? 0,
  daily_loss_day_count: parseOptionalInt(row?.daily_loss_day_count) ?? 0,
  symbol_count: parseOptionalInt(row?.symbol_count) ?? 0,
  template_count: parseOptionalInt(row?.template_count) ?? 0,
  contract_count: parseOptionalInt(row?.contract_count) ?? 0,
});

const parseEntryExitSimStreakRow = (row = {}) => ({
  ...row,
  streak_number: parseOptionalInt(row?.streak_number) ?? 0,
  start_index: parseOptionalInt(row?.start_index) ?? 0,
  end_index: parseOptionalInt(row?.end_index) ?? 0,
  streak_length: parseOptionalInt(row?.streak_length) ?? 0,
  sum_r: parseOptionalFloat(row?.sum_r) ?? 0,
});

const parseEntryExitSimLossSummary = (row = null) => {
  if (!row) {
    return null;
  }

  return {
    ...row,
    losses: parseOptionalInt(row?.losses) ?? 0,
    loss_gap_count: parseOptionalInt(row?.loss_gap_count) ?? 0,
    avg_loss_gap_minutes: parseOptionalFloat(row?.avg_loss_gap_minutes) ?? 0,
    median_loss_gap_minutes: parseOptionalFloat(row?.median_loss_gap_minutes) ?? 0,
    min_loss_gap_minutes: parseOptionalFloat(row?.min_loss_gap_minutes) ?? 0,
    max_loss_gap_minutes: parseOptionalFloat(row?.max_loss_gap_minutes) ?? 0,
    clustered_60m_loss_pairs: parseOptionalInt(row?.clustered_60m_loss_pairs) ?? 0,
    same_day_loss_pairs: parseOptionalInt(row?.same_day_loss_pairs) ?? 0,
    clustered_60m_rate: parseOptionalFloat(row?.clustered_60m_rate) ?? 0,
    loss_days: parseOptionalInt(row?.loss_days) ?? 0,
    loss_days_5_plus: parseOptionalInt(row?.loss_days_5_plus) ?? 0,
    worst_loss_day_losses: parseOptionalInt(row?.worst_loss_day_losses) ?? 0,
    worst_loss_day_r: parseOptionalFloat(row?.worst_loss_day_r) ?? 0,
    worst_loss_hour: parseOptionalInt(row?.worst_loss_hour),
    worst_loss_hour_losses: parseOptionalInt(row?.worst_loss_hour_losses) ?? 0,
    worst_loss_hour_r: parseOptionalFloat(row?.worst_loss_hour_r) ?? 0,
    max_loss_streak: parseOptionalInt(row?.max_loss_streak) ?? 0,
  };
};

const parseEntryExitSimLossGapBucket = (row = {}) => ({
  ...row,
  bucket_key: row?.bucket_key ?? '',
  bucket_label: row?.bucket_label ?? '',
  sort_order: parseOptionalInt(row?.sort_order) ?? 0,
  gap_count: parseOptionalInt(row?.gap_count) ?? 0,
  gap_percent: parseOptionalFloat(row?.gap_percent) ?? 0,
});

const parseEntryExitSimLossWindow = (row = {}) => ({
  ...row,
  entry_hour: parseOptionalInt(row?.entry_hour) ?? 0,
  trades: parseOptionalInt(row?.trades) ?? 0,
  wins: parseOptionalInt(row?.wins) ?? 0,
  losses: parseOptionalInt(row?.losses) ?? 0,
  no_entries: parseOptionalInt(row?.no_entries) ?? 0,
  total_r: parseOptionalFloat(row?.total_r) ?? 0,
  loss_rate: parseOptionalFloat(row?.loss_rate) ?? 0,
  top_root_symbol: row?.top_root_symbol ?? '',
  top_family_key: row?.top_family_key ?? '',
  top_template_uid: row?.top_template_uid ?? '',
});

const parseEntryExitManualFamilyBanRecord = (row = {}) => ({
  ...row,
  family_key: row?.family_key ?? '',
  reason: row?.reason ?? '',
  created_at: row?.created_at ?? null,
  updated_at: row?.updated_at ?? null,
});

const parseEntryExitManualSymbolBanRecord = (row = {}) => ({
  ...row,
  root_symbol: row?.root_symbol ?? '',
  reason: row?.reason ?? '',
  created_at: row?.created_at ?? null,
  updated_at: row?.updated_at ?? null,
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

export const getCandles = async (
  symbol,
  { startDate = null, endDate = null, sourceTimeframe = null } = {}
) => {
  if (!symbol) {
    return [];
  }

  try {
    const candles = await postJson('/candles', {
      symbol,
      start_date: startDate,
      end_date: endDate,
      source_timeframe: sourceTimeframe || null,
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
    allFamilies = false,
    sourceScope = 'futures',
    sourceTimeframe = null,
    symbol = null,
    year = null,
  } = {},
  pagination = { limit: 500, offset: 0, includeCount: true }
) => {
  if (!familyKey && !allFamilies) {
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
      all_families: Boolean(allFamilies),
      source_scope: sourceScope,
      source_timeframe: sourceTimeframe || null,
      symbol: symbol || null,
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
  sourceTimeframe = null,
} = {}) => {
  try {
    const data = await postJson('/pattern-families', {
      limit,
      min_setup_count: minSetupCount,
      year: parseOptionalInt(year),
      source_scope: sourceScope,
      source_timeframe: sourceTimeframe || null,
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

export const fetchEntryExitTemplates = async ({
  runId = null,
  sourceScope = 'futures',
  year = null,
  limit = 250,
} = {}) => {
  try {
    const data = await postJson('/entry-exit/templates', {
      run_id: runId,
      source_scope: sourceScope,
      year: parseOptionalInt(year),
      limit: parseOptionalInt(limit),
    });

    return {
      run: parseEntryExitTemplateRun(data?.run ?? null),
      build_summary: parseEntryExitTemplateBuildSummary(data?.build_summary ?? null),
      templates: Array.isArray(data?.templates)
        ? data.templates.map(parseEntryExitTemplateRecord)
        : [],
      coverage: Array.isArray(data?.coverage)
        ? data.coverage.map(parseEntryExitTemplateCoverageRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { run: null, build_summary: null, templates: [], coverage: [] };
  }
};

export const fetchEntryExitBuildDashboard = async ({ runId = null } = {}) => {
  try {
    const data = await postJson('/entry-exit/build-dashboard', {
      run_id: runId,
    });
    const buildSummary = parseEntryExitTemplateBuildSummary(data?.build_summary ?? null);

    return {
      run: buildSummary
        ? {
            run_id: buildSummary.run_id,
            source_scope: buildSummary.source_scope,
            period_year:
              buildSummary.scan_year_start && buildSummary.scan_year_start === buildSummary.scan_year_end
                ? buildSummary.scan_year_start
                : 0,
            requested_limit: buildSummary.requested_limit,
            scanned_patterns: buildSummary.patterns_scanned,
            templates_created: buildSummary.templates_created,
            existing_template_passes: 0,
            failed_to_create: 0,
            result_rows: buildSummary.result_rows,
            elapsed_ms: buildSummary.elapsed_ms,
            created_at: buildSummary.created_at ?? null,
          }
        : null,
      build_summary: buildSummary,
      templates: [],
      coverage: Array.isArray(data?.coverage)
        ? data.coverage.map(parseEntryExitTemplateCoverageRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { run: null, build_summary: null, templates: [], coverage: [] };
  }
};

export const fetchEntryExitBuildRawRows = async ({
  runId = null,
  limit = 300,
  offset = 0,
} = {}) => {
  if (!runId) {
    return { run_id: '', table_name: null, total_rows: 0, limit, offset, rows: [] };
  }

  try {
    const data = await postJson('/entry-exit/build-raw-rows', {
      run_id: runId,
      limit: parseOptionalInt(limit),
      offset: parseOptionalInt(offset),
    });

    return {
      run_id: data?.run_id ?? runId,
      table_name: data?.table_name ?? null,
      total_rows: parseOptionalInt(data?.total_rows) ?? 0,
      limit: parseOptionalInt(data?.limit) ?? limit,
      offset: parseOptionalInt(data?.offset) ?? offset,
      rows: Array.isArray(data?.rows)
        ? data.rows.map(parseEntryExitBuildRawRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { run_id: runId, table_name: null, total_rows: 0, limit, offset, rows: [] };
  }
};

export const fetchEntryExitBuilds = async ({ limit = 25 } = {}) => {
  try {
    const data = await postJson('/entry-exit/builds', {
      limit: parseOptionalInt(limit),
    });

    return Array.isArray(data?.builds)
      ? data.builds.map(parseEntryExitTemplateBuildSummary).filter(Boolean)
      : [];
  } catch (error) {
    console.error(error);
    return [];
  }
};

const parsePatternXaOutcomeRun = (run = null) => {
  if (!run) return null;
  return {
    run_id: run.run_id ?? '',
    source_scope: run.source_scope ?? '',
    source_timeframe: run.source_timeframe ?? '',
    scan_year_start: parseOptionalInt(run.scan_year_start) ?? 0,
    scan_year_end: parseOptionalInt(run.scan_year_end) ?? 0,
    scan_year_label: run.scan_year_label ?? '',
    requested_limit: parseOptionalInt(run.requested_limit) ?? 0,
    max_forward_multiple: parseOptionalInt(run.max_forward_multiple) ?? 0,
    xa_multiple: parseOptionalFloat(run.xa_multiple) ?? 0,
    patterns_scanned: parseOptionalInt(run.patterns_scanned) ?? 0,
    reversal_count: parseOptionalInt(run.reversal_count) ?? 0,
    continuation_count: parseOptionalInt(run.continuation_count) ?? 0,
    ambiguous_count: parseOptionalInt(run.ambiguous_count) ?? 0,
    none_count: parseOptionalInt(run.none_count) ?? 0,
    elapsed_ms: parseOptionalInt(run.elapsed_ms) ?? 0,
    created_at: run.created_at ?? null,
  };
};

const parsePatternXaOutcomeRow = (row = {}) => ({
  setup_id: row.setup_id ?? '',
  pattern_id: row.pattern_id ?? '',
  pattern_group_id: row.pattern_group_id ?? '',
  event_id: row.event_id ?? '',
  symbol: row.symbol ?? '',
  root_symbol: row.root_symbol ?? '',
  contract_symbol: row.contract_symbol ?? '',
  source_timeframe: row.source_timeframe ?? '',
  market: row.market ?? '',
  pattern_family_key: row.pattern_family_key ?? '',
  d_date: row.d_date ?? null,
  d_confirm_date: row.d_confirm_date ?? null,
  d_price: parseOptionalFloat(row.d_price),
  xa_distance: parseOptionalFloat(row.xa_distance),
  xa_multiple: parseOptionalFloat(row.xa_multiple),
  reversal_target_price: parseOptionalFloat(row.reversal_target_price),
  continuation_target_price: parseOptionalFloat(row.continuation_target_price),
  outcome: row.outcome ?? '',
  hit_date: row.hit_date ?? null,
  bars_to_hit: parseOptionalInt(row.bars_to_hit),
  minutes_to_hit: parseOptionalInt(row.minutes_to_hit),
  max_reversal_excursion: parseOptionalFloat(row.max_reversal_excursion),
  max_continuation_excursion: parseOptionalFloat(row.max_continuation_excursion),
  candles_scanned: parseOptionalInt(row.candles_scanned) ?? 0,
});

const parsePatternXaOutcomeFamilyRow = (row = {}) => ({
  pattern_family_key: row.pattern_family_key ?? '',
  harmonic_type: row.harmonic_type ?? '',
  family_bin: row.family_bin ?? '',
  family_size_bucket: row.family_size_bucket ?? '',
  family_time_bin: row.family_time_bin ?? '',
  family_x_strictness: row.family_x_strictness ?? '',
  total_count: parseOptionalInt(row.total_count) ?? 0,
  reversal_count: parseOptionalInt(row.reversal_count) ?? 0,
  continuation_count: parseOptionalInt(row.continuation_count) ?? 0,
  ambiguous_count: parseOptionalInt(row.ambiguous_count) ?? 0,
  none_count: parseOptionalInt(row.none_count) ?? 0,
  avg_bars_to_hit: parseOptionalFloat(row.avg_bars_to_hit),
  avg_minutes_to_hit: parseOptionalFloat(row.avg_minutes_to_hit),
});

export const fetchPatternXaOutcomes = async ({
  runId = null,
  limit = 300,
  offset = 0,
} = {}) => {
  try {
    const data = await postJson('/patterns/xa-outcomes', {
      run_id: runId,
      limit: parseOptionalInt(limit),
      offset: parseOptionalInt(offset),
    });

    return {
      run: parsePatternXaOutcomeRun(data?.run ?? null),
      total_rows: parseOptionalInt(data?.total_rows) ?? 0,
      limit: parseOptionalInt(data?.limit) ?? limit,
      offset: parseOptionalInt(data?.offset) ?? offset,
      family_rows: Array.isArray(data?.family_rows)
        ? data.family_rows.map(parsePatternXaOutcomeFamilyRow)
        : [],
      rows: Array.isArray(data?.rows)
        ? data.rows.map(parsePatternXaOutcomeRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { run: null, total_rows: 0, limit, offset, family_rows: [], rows: [] };
  }
};

const parsePatternReversalAiRun = (run = null) => {
  if (!run) return null;
  return {
    ai_run_id: run.ai_run_id ?? '',
    source_xa_run_id: run.source_xa_run_id ?? '',
    model_type: run.model_type ?? '',
    feature_set_version: run.feature_set_version ?? '',
    decision_threshold: parseOptionalFloat(run.decision_threshold) ?? 0,
    score_rows: parseOptionalInt(run.score_rows) ?? 0,
    predicted_reversal_count: parseOptionalInt(run.predicted_reversal_count) ?? 0,
    predicted_reversal_actual_reversal_count: parseOptionalInt(run.predicted_reversal_actual_reversal_count) ?? 0,
    baseline_reversal_rate: parseOptionalFloat(run.baseline_reversal_rate) ?? 0,
    predicted_reversal_actual_rate: parseOptionalFloat(run.predicted_reversal_actual_rate) ?? 0,
    lift_vs_baseline: parseOptionalFloat(run.lift_vs_baseline) ?? 0,
    elapsed_ms: parseOptionalInt(run.elapsed_ms) ?? 0,
    created_at: run.created_at ?? null,
  };
};

const parsePatternReversalAiBucketRow = (row = {}) => ({
  confidence_bucket: row.confidence_bucket ?? '',
  row_count: parseOptionalInt(row.row_count) ?? 0,
  actual_reversal_count: parseOptionalInt(row.actual_reversal_count) ?? 0,
  actual_reversal_rate: parseOptionalFloat(row.actual_reversal_rate) ?? 0,
  avg_predicted_reversal_probability: parseOptionalFloat(row.avg_predicted_reversal_probability) ?? 0,
});

const parsePatternReversalAiThresholdRow = (row = {}) => ({
  threshold_value: parseOptionalFloat(row.threshold_value) ?? 0,
  row_count: parseOptionalInt(row.row_count) ?? 0,
  actual_reversal_count: parseOptionalInt(row.actual_reversal_count) ?? 0,
  actual_reversal_rate: parseOptionalFloat(row.actual_reversal_rate) ?? 0,
  avg_predicted_reversal_probability: parseOptionalFloat(row.avg_predicted_reversal_probability) ?? 0,
  lift_vs_baseline: parseOptionalFloat(row.lift_vs_baseline) ?? 0,
});

const parsePatternReversalAiScoreRow = (row = {}) => ({
  setup_id: row.setup_id ?? '',
  pattern_id: row.pattern_id ?? '',
  pattern_group_id: row.pattern_group_id ?? '',
  symbol: row.symbol ?? '',
  root_symbol: row.root_symbol ?? '',
  source_timeframe: row.source_timeframe ?? '',
  market: row.market ?? '',
  pattern_family_key: row.pattern_family_key ?? '',
  d_confirm_date: row.d_confirm_date ?? null,
  actual_outcome: row.actual_outcome ?? '',
  actual_reversed: parseOptionalInt(row.actual_reversed) ?? 0,
  predicted_reversal_probability: parseOptionalFloat(row.predicted_reversal_probability) ?? 0,
  predicted_continuation_probability: parseOptionalFloat(row.predicted_continuation_probability) ?? 0,
  ai_decision: row.ai_decision ?? '',
  was_correct: parseOptionalInt(row.was_correct) ?? 0,
  confidence_bucket: row.confidence_bucket ?? '',
  harmonic_type: row.harmonic_type ?? '',
  family_bin: row.family_bin ?? '',
  family_size_bucket: row.family_size_bucket ?? '',
  family_time_bin: row.family_time_bin ?? '',
  family_x_strictness: row.family_x_strictness ?? '',
});

export const fetchPatternReversalAiScores = async ({
  aiRunId = null,
  limit = 300,
  offset = 0,
} = {}) => {
  try {
    const data = await postJson('/patterns/reversal-ai-scores', {
      ai_run_id: aiRunId,
      limit: parseOptionalInt(limit),
      offset: parseOptionalInt(offset),
    });

    return {
      run: parsePatternReversalAiRun(data?.run ?? null),
      total_rows: parseOptionalInt(data?.total_rows) ?? 0,
      limit: parseOptionalInt(data?.limit) ?? limit,
      offset: parseOptionalInt(data?.offset) ?? offset,
      thresholds: Array.isArray(data?.thresholds)
        ? data.thresholds.map(parsePatternReversalAiThresholdRow)
        : [],
      buckets: Array.isArray(data?.buckets)
        ? data.buckets.map(parsePatternReversalAiBucketRow)
        : [],
      rows: Array.isArray(data?.rows)
        ? data.rows.map(parsePatternReversalAiScoreRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { run: null, total_rows: 0, limit, offset, thresholds: [], buckets: [], rows: [] };
  }
};

const parsePatternAiStage1TradeRun = (run = null) => {
  if (!run) return null;
  return {
    multi_valid_eval_run_id: run.multi_valid_eval_run_id ?? '',
    source_run_id: run.source_run_id ?? '',
    results_table: run.results_table ?? '',
    train_start_year: parseOptionalInt(run.train_start_year) ?? 0,
    train_end_year: parseOptionalInt(run.train_end_year) ?? 0,
    valid_year: parseOptionalInt(run.valid_year) ?? 0,
    train_sample_slot: parseOptionalInt(run.train_sample_slot) ?? 0,
    valid_sample_slots: run.valid_sample_slots ?? '',
    train_setups: parseOptionalInt(run.train_setups) ?? 0,
    train_rows: parseOptionalInt(run.train_rows) ?? 0,
    iterations: parseOptionalInt(run.iterations) ?? 0,
    depth: parseOptionalInt(run.depth) ?? 0,
    learning_rate: parseOptionalFloat(run.learning_rate) ?? 0,
    l2_leaf_reg: parseOptionalFloat(run.l2_leaf_reg) ?? 0,
    random_strength: parseOptionalFloat(run.random_strength) ?? 0,
    random_seed: parseOptionalInt(run.random_seed) ?? 0,
    pre_feature_set: run.pre_feature_set ?? '',
    aggregate_feature_set: run.aggregate_feature_set ?? '',
    excluded_roots: run.excluded_roots ?? '',
    model_path: run.model_path ?? '',
    created_at: run.created_at ?? null,
  };
};

const parsePatternAiStage1TradeSummary = (summary = null) => {
  if (!summary) return null;
  return {
    total_trades: parseOptionalInt(summary.total_trades) ?? 0,
    wins: parseOptionalInt(summary.wins) ?? 0,
    losses: parseOptionalInt(summary.losses) ?? 0,
    no_entries: parseOptionalInt(summary.no_entries) ?? 0,
    win_rate: parseOptionalFloat(summary.win_rate) ?? 0,
    avg_r: parseOptionalFloat(summary.avg_r) ?? 0,
    sum_r: parseOptionalFloat(summary.sum_r) ?? 0,
    best_r: parseOptionalFloat(summary.best_r) ?? 0,
    worst_r: parseOptionalFloat(summary.worst_r) ?? 0,
    first_trade_date: summary.first_trade_date ?? null,
    last_trade_date: summary.last_trade_date ?? null,
    slot_count: parseOptionalInt(summary.slot_count) ?? 0,
    symbol_count: parseOptionalInt(summary.symbol_count) ?? 0,
    family_count: parseOptionalInt(summary.family_count) ?? 0,
    template_count: parseOptionalInt(summary.template_count) ?? 0,
    avg_predicted_expected_r: parseOptionalFloat(summary.avg_predicted_expected_r) ?? 0,
    avg_score_margin_top2: parseOptionalFloat(summary.avg_score_margin_top2) ?? 0,
  };
};

const parsePatternAiStage1TemplatePerformanceRow = (row = {}) => ({
  template_uid: row.template_uid ?? '',
  template_name: row.template_name ?? '',
  trades: parseOptionalInt(row.trades) ?? 0,
  wins: parseOptionalInt(row.wins) ?? 0,
  losses: parseOptionalInt(row.losses) ?? 0,
  no_entries: parseOptionalInt(row.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row.sum_r) ?? 0,
  best_r: parseOptionalFloat(row.best_r) ?? 0,
  worst_r: parseOptionalFloat(row.worst_r) ?? 0,
  family_count: parseOptionalInt(row.family_count) ?? 0,
  symbol_count: parseOptionalInt(row.symbol_count) ?? 0,
  avg_predicted_expected_r: parseOptionalFloat(row.avg_predicted_expected_r) ?? 0,
  avg_score_margin_top2: parseOptionalFloat(row.avg_score_margin_top2) ?? 0,
});

const parsePatternAiStage1DailyRow = (row = {}) => ({
  trade_date: row.trade_date ?? '',
  trades: parseOptionalInt(row.trades) ?? 0,
  wins: parseOptionalInt(row.wins) ?? 0,
  losses: parseOptionalInt(row.losses) ?? 0,
  no_entries: parseOptionalInt(row.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row.win_rate) ?? 0,
  total_r: parseOptionalFloat(row.total_r) ?? 0,
  avg_r: parseOptionalFloat(row.avg_r) ?? 0,
  best_r: parseOptionalFloat(row.best_r) ?? 0,
  worst_r: parseOptionalFloat(row.worst_r) ?? 0,
  cumulative_r: parseOptionalFloat(row.cumulative_r) ?? 0,
});

const parsePatternAiStage1HourlyRow = (row = {}) => ({
  entry_hour: parseOptionalInt(row.entry_hour) ?? 0,
  trades: parseOptionalInt(row.trades) ?? 0,
  wins: parseOptionalInt(row.wins) ?? 0,
  losses: parseOptionalInt(row.losses) ?? 0,
  no_entries: parseOptionalInt(row.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row.sum_r) ?? 0,
  best_r: parseOptionalFloat(row.best_r) ?? 0,
  worst_r: parseOptionalFloat(row.worst_r) ?? 0,
});

const parsePatternAiStage1TradeCadence = (row = null) => {
  if (!row) return null;
  return {
    first_trade_at: row.first_trade_at ?? null,
    last_trade_at: row.last_trade_at ?? null,
    trades: parseOptionalInt(row.trades) ?? 0,
    trade_days: parseOptionalInt(row.trade_days) ?? 0,
    active_weeks: parseOptionalInt(row.active_weeks) ?? 0,
    active_months: parseOptionalInt(row.active_months) ?? 0,
    gap_count: parseOptionalInt(row.gap_count) ?? 0,
    avg_gap_minutes: parseOptionalFloat(row.avg_gap_minutes) ?? 0,
    median_gap_minutes: parseOptionalFloat(row.median_gap_minutes) ?? 0,
    min_gap_minutes: parseOptionalFloat(row.min_gap_minutes) ?? 0,
    max_gap_minutes: parseOptionalFloat(row.max_gap_minutes) ?? 0,
    max_trades_5m_window: parseOptionalInt(row.max_trades_5m_window) ?? 0,
    max_trades_15m_window: parseOptionalInt(row.max_trades_15m_window) ?? 0,
    gap_0_1m: parseOptionalInt(row.gap_0_1m) ?? 0,
    gap_1_5m: parseOptionalInt(row.gap_1_5m) ?? 0,
    gap_5_15m: parseOptionalInt(row.gap_5_15m) ?? 0,
    gap_15_30m: parseOptionalInt(row.gap_15_30m) ?? 0,
    gap_30_60m: parseOptionalInt(row.gap_30_60m) ?? 0,
    gap_over_60m: parseOptionalInt(row.gap_over_60m) ?? 0,
  };
};

const parsePatternAiStage1TradeWorkload = (row = null) => {
  if (!row) return null;
  return {
    first_trade_at: row.first_trade_at ?? null,
    last_trade_at: row.last_trade_at ?? null,
    trades: parseOptionalInt(row.trades) ?? 0,
    active_hours: parseOptionalInt(row.active_hours) ?? 0,
    active_days: parseOptionalInt(row.active_days) ?? 0,
    active_weeks: parseOptionalInt(row.active_weeks) ?? 0,
    active_months: parseOptionalInt(row.active_months) ?? 0,
    avg_trades_per_hour: parseOptionalFloat(row.avg_trades_per_hour) ?? 0,
    avg_trades_per_day: parseOptionalFloat(row.avg_trades_per_day) ?? 0,
    avg_trades_per_week: parseOptionalFloat(row.avg_trades_per_week) ?? 0,
    avg_trades_per_month: parseOptionalFloat(row.avg_trades_per_month) ?? 0,
    min_trades_per_day: parseOptionalInt(row.min_trades_per_day) ?? 0,
    max_trades_per_hour: parseOptionalInt(row.max_trades_per_hour) ?? 0,
    max_trades_per_day: parseOptionalInt(row.max_trades_per_day) ?? 0,
    max_trades_per_week: parseOptionalInt(row.max_trades_per_week) ?? 0,
    max_trades_per_month: parseOptionalInt(row.max_trades_per_month) ?? 0,
    hours_over_5_trades: parseOptionalInt(row.hours_over_5_trades) ?? 0,
    days_over_20_trades: parseOptionalInt(row.days_over_20_trades) ?? 0,
  };
};

const parsePatternAiStage1SymbolContributionRow = (row = {}) => ({
  root_symbol: row.root_symbol ?? '',
  trades: parseOptionalInt(row.trades) ?? 0,
  wins: parseOptionalInt(row.wins) ?? 0,
  losses: parseOptionalInt(row.losses) ?? 0,
  no_entries: parseOptionalInt(row.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row.sum_r) ?? 0,
  best_r: parseOptionalFloat(row.best_r) ?? 0,
  worst_r: parseOptionalFloat(row.worst_r) ?? 0,
  family_count: parseOptionalInt(row.family_count) ?? 0,
  template_count: parseOptionalInt(row.template_count) ?? 0,
});

const parsePatternAiStage1FamilyContributionRow = (row = {}) => ({
  family_key: row.family_key ?? '',
  harmonic_type: row.harmonic_type ?? '',
  family_bin: row.family_bin ?? '',
  family_size_bucket: row.family_size_bucket ?? '',
  family_time_bin: row.family_time_bin ?? '',
  family_x_strictness: row.family_x_strictness ?? '',
  trades: parseOptionalInt(row.trades) ?? 0,
  wins: parseOptionalInt(row.wins) ?? 0,
  losses: parseOptionalInt(row.losses) ?? 0,
  no_entries: parseOptionalInt(row.no_entries) ?? 0,
  win_rate: parseOptionalFloat(row.win_rate) ?? 0,
  avg_r: parseOptionalFloat(row.avg_r) ?? 0,
  sum_r: parseOptionalFloat(row.sum_r) ?? 0,
  best_r: parseOptionalFloat(row.best_r) ?? 0,
  worst_r: parseOptionalFloat(row.worst_r) ?? 0,
  symbol_count: parseOptionalInt(row.symbol_count) ?? 0,
  template_count: parseOptionalInt(row.template_count) ?? 0,
});

const parsePatternAiStage1LossWindowRow = (row = {}) => ({
  trade_date: row.trade_date ?? '',
  entry_hour: parseOptionalInt(row.entry_hour) ?? 0,
  trades: parseOptionalInt(row.trades) ?? 0,
  wins: parseOptionalInt(row.wins) ?? 0,
  losses: parseOptionalInt(row.losses) ?? 0,
  no_entries: parseOptionalInt(row.no_entries) ?? 0,
  loss_rate: parseOptionalFloat(row.loss_rate) ?? 0,
  total_r: parseOptionalFloat(row.total_r) ?? 0,
  symbol_count: parseOptionalInt(row.symbol_count) ?? 0,
  family_count: parseOptionalInt(row.family_count) ?? 0,
  template_count: parseOptionalInt(row.template_count) ?? 0,
});

const parsePatternAiStage1TradeRow = (row = {}) => {
  const templateUid = row.template_uid ?? '';
  const harmonicType = row.harmonic_type ?? '';
  const familyTimeBin = row.family_time_bin ?? '';
  const sourceTimeframe =
    row.source_timeframe && row.source_timeframe !== 'unknown'
      ? row.source_timeframe
      : familyTimeBin || (String(templateUid).startsWith('candle_wave_') ? '2m' : row.source_timeframe ?? '');

  return {
    multi_valid_eval_run_id: row.multi_valid_eval_run_id ?? '',
    valid_sample_slot: parseOptionalInt(row.valid_sample_slot) ?? 0,
    setup_id: row.setup_id ?? '',
    pattern_id: row.pattern_id ?? '',
    pattern_group_id: row.pattern_group_id ?? '',
    symbol: row.symbol ?? '',
    root_symbol: row.root_symbol ?? '',
    source_timeframe: sourceTimeframe,
    market: row.market ?? '',
    pattern_family_key: row.pattern_family_key ?? '',
    d_confirm_date: row.d_confirm_date ?? null,
    template_uid: templateUid,
    template_name: row.template_name ?? '',
    predicted_expected_r: parseOptionalFloat(row.predicted_expected_r),
    score_margin_top2: parseOptionalFloat(row.score_margin_top2),
    result_r: parseOptionalFloat(row.result_r) ?? 0,
    outcome: row.outcome ?? '',
    oracle_template_uid: row.oracle_template_uid ?? '',
    oracle_result_r: parseOptionalFloat(row.oracle_result_r),
    oracle_rank: parseOptionalInt(row.oracle_rank),
    entry_date: row.entry_date ?? null,
    exit_date: row.exit_date ?? null,
    entry_price: parseOptionalFloat(row.entry_price),
    stop_price: parseOptionalFloat(row.stop_price),
    target_price: parseOptionalFloat(row.target_price),
    exit_price: parseOptionalFloat(row.exit_price),
    risk_points: parseOptionalFloat(row.risk_points),
    exit_reason: row.exit_reason ?? '',
    trade_direction: row.trade_direction ?? '',
    harmonic_type: harmonicType,
    family_bin: row.family_bin ?? '',
    family_size_bucket: row.family_size_bucket ?? '',
    family_time_bin: familyTimeBin,
    family_x_strictness: row.family_x_strictness ?? '',
  };
};

export const fetchPatternAiStage1Trades = async ({
  aiRunId = null,
  validYear = 2026,
  takenOnly = false,
  limit = 300,
  offset = 0,
} = {}) => {
  try {
    const data = await postJson('/patterns/ai-stage1-trades', {
      ai_run_id: aiRunId,
      valid_year: parseOptionalInt(validYear),
      taken_only: Boolean(takenOnly),
      limit: parseOptionalInt(limit),
      offset: parseOptionalInt(offset),
    });

    return {
      run: parsePatternAiStage1TradeRun(data?.run ?? null),
      summary: parsePatternAiStage1TradeSummary(data?.summary ?? null),
      template_performance: Array.isArray(data?.template_performance)
        ? data.template_performance.map(parsePatternAiStage1TemplatePerformanceRow)
        : [],
      daily: Array.isArray(data?.daily) ? data.daily.map(parsePatternAiStage1DailyRow) : [],
      hourly: Array.isArray(data?.hourly) ? data.hourly.map(parsePatternAiStage1HourlyRow) : [],
      trade_cadence: parsePatternAiStage1TradeCadence(data?.trade_cadence ?? null),
      trade_workload: parsePatternAiStage1TradeWorkload(data?.trade_workload ?? null),
      symbol_contribution: Array.isArray(data?.symbol_contribution)
        ? data.symbol_contribution.map(parsePatternAiStage1SymbolContributionRow)
        : [],
      family_contribution: Array.isArray(data?.family_contribution)
        ? data.family_contribution.map(parsePatternAiStage1FamilyContributionRow)
        : [],
      loss_windows: Array.isArray(data?.loss_windows)
        ? data.loss_windows.map(parsePatternAiStage1LossWindowRow)
        : [],
      total_rows: parseOptionalInt(data?.total_rows) ?? 0,
      limit: parseOptionalInt(data?.limit) ?? limit,
      offset: parseOptionalInt(data?.offset) ?? offset,
      rows: Array.isArray(data?.rows) ? data.rows.map(parsePatternAiStage1TradeRow) : [],
    };
  } catch (error) {
    console.error(error);
    return {
      run: null,
      summary: null,
      template_performance: [],
      daily: [],
      hourly: [],
      trade_cadence: null,
      trade_workload: null,
      symbol_contribution: [],
      family_contribution: [],
      loss_windows: [],
      total_rows: 0,
      limit,
      offset,
      rows: [],
    };
  }
};

export const fetchEntryExitRouterRuns = async ({
  trainRunId = null,
  limit = 8,
} = {}) => {
  try {
    const data = await postJson('/entry-exit/router-runs', {
      train_run_id: trainRunId,
      limit: parseOptionalInt(limit),
    });

    return {
      current_run: parseEntryExitRouterRunRecord(data?.current_run ?? null),
      runs: Array.isArray(data?.runs)
        ? data.runs.map(parseEntryExitRouterRunRecord).filter(Boolean)
        : [],
      symbols: Array.isArray(data?.symbols)
        ? data.symbols.map(parseEntryExitRouterSymbolRecord)
        : [],
      family_routes: Array.isArray(data?.family_routes)
        ? data.family_routes.map(parseEntryExitRouterFamilyRouteRecord)
        : [],
      template_performance: Array.isArray(data?.template_performance)
        ? data.template_performance.map(parseEntryExitSimTemplatePerformanceRecord)
        : [],
      manual_family_bans: Array.isArray(data?.manual_family_bans)
        ? data.manual_family_bans.map(parseEntryExitManualFamilyBanRecord)
        : [],
      manual_symbol_bans: Array.isArray(data?.manual_symbol_bans)
        ? data.manual_symbol_bans.map(parseEntryExitManualSymbolBanRecord)
        : [],
    };
  } catch (error) {
    console.error(error);
    return {
      current_run: null,
      runs: [],
      symbols: [],
      family_routes: [],
      template_performance: [],
      manual_family_bans: [],
      manual_symbol_bans: [],
    };
  }
};

export const fetchEntryExitSimEquityCurve = async ({
  simRunId = null,
  pointLimit = 1200,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', points: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-equity-curve', {
      sim_run_id: simRunId,
      point_limit: parseOptionalInt(pointLimit),
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      points: Array.isArray(data?.points)
        ? data.points.map(parseEntryExitSimEquityPoint)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, points: [] };
  }
};

export const fetchEntryExitSimDailyR = async ({
  simRunId = null,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', days: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-daily-r', {
      sim_run_id: simRunId,
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      days: Array.isArray(data?.days)
        ? data.days.map(parseEntryExitSimDailyRRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, days: [] };
  }
};

export const fetchEntryExitSimDailyTrades = async ({
  simRunId = null,
  tradeDate = null,
} = {}) => {
  if (!simRunId || !tradeDate) {
    return { sim_run_id: '', trade_date: '', trades: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-daily-trades', {
      sim_run_id: simRunId,
      trade_date: tradeDate,
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      trade_date: data?.trade_date ?? tradeDate,
      trades: Array.isArray(data?.trades)
        ? data.trades.map(parseEntryExitSimDailyTradeRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, trade_date: tradeDate, trades: [] };
  }
};

export const fetchEntryExitSimRawTrades = async ({
  simRunId = null,
  limit = 500,
  offset = 0,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', total_rows: 0, limit, offset, trades: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-raw-trades', {
      sim_run_id: simRunId,
      limit: parseOptionalInt(limit),
      offset: parseOptionalInt(offset),
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      total_rows: parseOptionalInt(data?.total_rows) ?? 0,
      limit: parseOptionalInt(data?.limit) ?? limit,
      offset: parseOptionalInt(data?.offset) ?? offset,
      trades: Array.isArray(data?.trades)
        ? data.trades.map(parseEntryExitSimRawTradeRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, total_rows: 0, limit, offset, trades: [] };
  }
};

export const fetchEntryExitSimHourly = async ({
  simRunId = null,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', hours: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-hourly', {
      sim_run_id: simRunId,
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      hours: Array.isArray(data?.hours)
        ? data.hours.map(parseEntryExitSimHourlyRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, hours: [] };
  }
};

export const fetchEntryExitSimTradeCadence = async ({
  simRunId = null,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', cadence: null };
  }

  try {
    const data = await postJson('/entry-exit/sim-trade-cadence', {
      sim_run_id: simRunId,
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      cadence: parseEntryExitSimTradeCadence(data?.cadence ?? null),
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, cadence: null };
  }
};

export const fetchEntryExitSimTradeGaps = async ({
  simRunId = null,
  limit = 5000,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', gaps: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-trade-gaps', {
      sim_run_id: simRunId,
      limit,
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      gaps: Array.isArray(data?.gaps)
        ? data.gaps.map(parseEntryExitSimTradeGapRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, gaps: [] };
  }
};

export const fetchEntryExitSimTradeWorkload = async ({
  simRunId = null,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', workload: null };
  }

  try {
    const data = await postJson('/entry-exit/sim-trade-workload', {
      sim_run_id: simRunId,
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      workload: parseEntryExitSimTradeWorkload(data?.workload ?? null),
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, workload: null };
  }
};

export const fetchEntryExitSimTestFrequency = async ({
  simRunId = null,
  limit = 500,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', tests: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-test-frequency', {
      sim_run_id: simRunId,
      limit,
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      tests: Array.isArray(data?.tests)
        ? data.tests.map(parseEntryExitSimTestFrequencyRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, tests: [] };
  }
};

export const fetchEntryExitDayTradingSim = async ({
  simRunId = null,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', summary: null, daily: [], monthly: [] };
  }

  try {
    const data = await postJson('/entry-exit/day-trading-sim', {
      sim_run_id: simRunId,
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      summary: parseEntryExitDayTradingSummary(data?.summary ?? null),
      daily: Array.isArray(data?.daily)
        ? data.daily.map(parseEntryExitDayTradingDailyRow)
        : [],
      monthly: Array.isArray(data?.monthly)
        ? data.monthly.map(parseEntryExitDayTradingMonthlyRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, summary: null, daily: [], monthly: [] };
  }
};

export const fetchEntryExitSimMarketTrends = async ({
  simRunId = null,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', performance: [], alignment: [], direction_alignment: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-market-trends', {
      sim_run_id: simRunId,
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      performance: Array.isArray(data?.performance)
        ? data.performance.map(parseEntryExitSimMarketTrendPerformanceRow)
        : [],
      alignment: Array.isArray(data?.alignment)
        ? data.alignment.map(parseEntryExitSimMarketTrendAlignmentRow)
        : [],
      direction_alignment: Array.isArray(data?.direction_alignment)
        ? data.direction_alignment.map(parseEntryExitSimMarketTrendDirectionRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, performance: [], alignment: [], direction_alignment: [] };
  }
};

export const fetchEntryExitSimSymbolContribution = async ({
  simRunId = null,
  limit = 120,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', symbols: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-symbol-contribution', {
      sim_run_id: simRunId,
      limit: parseOptionalInt(limit),
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      symbols: Array.isArray(data?.symbols)
        ? data.symbols.map(parseEntryExitSimSymbolContributionRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, symbols: [] };
  }
};

export const fetchEntryExitSimFamilyContribution = async ({
  simRunId = null,
  limit = 120,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', families: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-family-contribution', {
      sim_run_id: simRunId,
      limit: parseOptionalInt(limit),
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      families: Array.isArray(data?.families)
        ? data.families.map(parseEntryExitSimFamilyContributionRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, families: [] };
  }
};

export const fetchEntryExitSimStreaks = async ({
  simRunId = null,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', streaks: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-streaks', {
      sim_run_id: simRunId,
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      streaks: Array.isArray(data?.streaks)
        ? data.streaks.map(parseEntryExitSimStreakRow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, streaks: [] };
  }
};

export const fetchEntryExitSimLossClustering = async ({
  simRunId = null,
  windowLimit = 40,
} = {}) => {
  if (!simRunId) {
    return { sim_run_id: '', summary: null, buckets: [], windows: [] };
  }

  try {
    const data = await postJson('/entry-exit/sim-loss-clustering', {
      sim_run_id: simRunId,
      window_limit: parseOptionalInt(windowLimit),
    });

    return {
      sim_run_id: data?.sim_run_id ?? simRunId,
      summary: parseEntryExitSimLossSummary(data?.summary ?? null),
      buckets: Array.isArray(data?.buckets)
        ? data.buckets.map(parseEntryExitSimLossGapBucket)
        : [],
      windows: Array.isArray(data?.windows)
        ? data.windows.map(parseEntryExitSimLossWindow)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { sim_run_id: simRunId, summary: null, buckets: [], windows: [] };
  }
};

export const fetchEntryExitTemplateBreakdown = async ({
  runId = null,
  templateUid = null,
} = {}) => {
  if (!runId || !templateUid) {
    return { market: [], harmonic_type: [], conditions: [], family_results: [], combos: [] };
  }

  try {
    const data = await postJson('/entry-exit/template-breakdown', {
      run_id: runId,
      template_uid: templateUid,
    });

    return {
      market: Array.isArray(data?.market)
        ? data.market.map(parseEntryExitTemplateMarketBreakdown)
        : [],
      harmonic_type: Array.isArray(data?.harmonic_type)
        ? data.harmonic_type.map(parseEntryExitTemplateMarketBreakdown)
        : [],
      conditions: Array.isArray(data?.conditions)
        ? data.conditions.map(parseEntryExitTemplateConditionBreakdown)
        : [],
      family_results: Array.isArray(data?.family_results)
        ? data.family_results.map(parseEntryExitTemplateFamilyBreakdown)
        : [],
      combos: Array.isArray(data?.combos)
        ? data.combos.map(parseEntryExitTemplateComboBreakdown)
        : [],
    };
  } catch (error) {
    console.error(error);
    return { market: [], harmonic_type: [], conditions: [], family_results: [], combos: [] };
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
