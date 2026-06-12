import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import CandleChartPanel from '../candle-chart/CandleChartPanel';
import {
  fetchCandleStorageSummary,
  fetchEntryExitBuilds,
  fetchEntryExitDayTradingSim,
  fetchEntryExitTemplateBreakdown,
  fetchEntryExitRouterRuns,
  fetchEntryExitSimDailyR,
  fetchEntryExitSimDailyTrades,
  fetchEntryExitSimEquityCurve,
  fetchEntryExitSimFamilyContribution,
  fetchEntryExitSimHourly,
  fetchEntryExitSimLossClustering,
  fetchEntryExitSimMarketTrends,
  fetchEntryExitSimRawTrades,
  fetchEntryExitSimStreaks,
  fetchEntryExitSimSymbolContribution,
  fetchEntryExitSimTradeCadence,
  fetchEntryExitSimTradeGaps,
  fetchEntryExitSimTestFrequency,
  fetchEntryExitSimTradeWorkload,
  fetchEntryExitTemplates,
  fetchPatternDetail,
  fetchPatternAiExitModelTrades,
  fetchPatternAiStage1Trades,
  fetchPatternFamilies,
  fetchPatternReversalAiScores,
  fetchPatternXaOutcomes,
  fetchNinjaTraderOracleTrends,
  fetchNinjaTraderLiveBarSnapshot,
  fetchNinjaTraderScannerActivity,
  fetchNinjaTraderSignalHistory,
  fetchNinjaTraderTrendEvents,
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

const PATTERN_AI_STAGE1_DEFAULT_RUN_ID = 'aicw-mtf-eg1-xtight-t014-rd3-en4-v1-2m-2026';
const PATTERN_AI_STAGE1_DEFAULT_YEAR = 2026;
const PATTERN_AI_STAGE1_TRADE_PAGE_SIZE = 1000;
const PATTERN_AI_EXIT_MAIN_RUN_ID = 'aicw-exit-dyn180s2-srcfb-v1-2m-2026';
const PATTERN_AI_EXIT_TRADE_PAGE_SIZE = 300;
const PATTERN_AI_EXIT_TAKEN_TRADE_PAGE_SIZE = 1000;
const LIVE_CANVAS_ROOT = 'HO';
const LIVE_CANVAS_SYMBOL = 'HO';
const LIVE_CANVAS_TIMEFRAME = '2m';
const LIVE_CANVAS_TREND_RUN_ID = 'ho-mho-audit-full-2m-v1-20210425-20260608';
const LIVE_CANVAS_SIGNAL_RUN_ID = 'nt-live-ho-5k-demo-20260610-v2';
const LIVE_CANVAS_ACCOUNT_NAME = 'DEMO5859105';
const LIVE_CANVAS_SIGNAL_INSTRUMENT = 'HO JUL26';
const LIVE_CANVAS_ORACLE_RUN_ID = 'oracle-long-trends-v3-10r-2m-HO-2024_2026';
const LIVE_CANVAS_DISPLAY_TIME_ZONE = 'America/Chicago';
const LIVE_CANVAS_START_DATE = null;
const LIVE_CANVAS_END_DATE = null;
const LIVE_CANVAS_CURRENT_VISIBLE_CANDLES = 56;
const LIVE_CANVAS_TREND_VISIBLE_CANDLES = 900;
const LIVE_CANVAS_CURRENT_VERTICAL_ZOOM = 3.25;
const LIVE_CANVAS_ALL_CONTRACTS = true;
const LIVE_CANVAS_INITIAL_CANDLE_PAGE_LIMIT = 1500;
const LIVE_CANVAS_CANDLE_PAGE_LIMIT = 3000;
const LIVE_CANVAS_TREND_CONTEXT_PRE_BARS = 900;
const LIVE_CANVAS_TREND_CONTEXT_POST_BARS = 420;
const LIVE_CANVAS_TREND_WINDOW_MIN_LIMIT = 2200;
const LIVE_CANVAS_TREND_WINDOW_MAX_LIMIT = 20000;
const LIVE_CANVAS_RECENT_REFRESH_LIMIT = 600;
const LIVE_CANVAS_POLL_MS = 5000;
const LIVE_CANVAS_SNAPSHOT_POLL_MS = 1000;
const LIVE_CANVAS_TREND_LIMIT = 80;
const LIVE_CANVAS_ORACLE_LIMIT = 10000;
const LIVE_CANVAS_SCANNER_LIMIT = 120;
const LIVE_CANVAS_FIXED_HISTORY_WINDOW = Boolean(LIVE_CANVAS_START_DATE || LIVE_CANVAS_END_DATE);
const NINJATRADER_MONTH_CODES = {
  JAN: 'F',
  FEB: 'G',
  MAR: 'H',
  APR: 'J',
  MAY: 'K',
  JUN: 'M',
  JUL: 'N',
  AUG: 'Q',
  SEP: 'U',
  OCT: 'V',
  NOV: 'X',
  DEC: 'Z',
};
const CANVAS_ONLY_MODE = true;

const formatNumber = (value) =>
  Number.isFinite(Number(value)) ? Number(value).toLocaleString() : '0';

const formatOptionalNumber = (value) =>
  Number.isFinite(Number(value)) ? Number(value).toLocaleString() : 'N/A';

const formatDate = (value) => {
  if (!value) return 'N/A';
  const text = String(value);
  return text.length > 10 ? text.slice(0, 10) : text;
};

const formatTime = (value) => {
  if (!value) return 'N/A';
  const text = String(value);
  const timePart = text.includes('T') ? text.split('T')[1] : text.split(' ')[1];
  return timePart ? timePart.slice(0, 5) : text.slice(11, 16) || 'N/A';
};

const parseServerUtcDate = (value) => {
  if (!value) return null;
  if (value instanceof Date) return Number.isNaN(value.getTime()) ? null : value;

  const text = String(value);
  const match = text.match(/^(\d{4})-(\d{2})-(\d{2})(?:[T\s](\d{2}):(\d{2})(?::(\d{2})(?:\.\d+)?)?)?/);
  if (match) {
    const parsed = new Date(Date.UTC(
      Number(match[1]),
      Number(match[2]) - 1,
      Number(match[3]),
      Number(match[4] ?? 0),
      Number(match[5] ?? 0),
      Number(match[6] ?? 0)
    ));
    return Number.isNaN(parsed.getTime()) ? null : parsed;
  }

  const parsed = new Date(text);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
};

const formatLiveCanvasNtTime = (value) => {
  const parsed = parseServerUtcDate(value);
  if (!parsed) return 'N/A';
  return parsed.toLocaleString('en-US', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    hour12: false,
    minute: '2-digit',
    timeZone: LIVE_CANVAS_DISPLAY_TIME_ZONE,
    timeZoneName: 'short',
  });
};

const formatShortDateTime = (value) => {
  if (!value) return 'N/A';
  return `${formatDate(value).slice(5)} ${formatTime(value)}`;
};

const formatTimelineTick = (value) => {
  if (!value) return 'N/A';
  return `${formatDate(value).slice(5)} ${formatTime(value)}`;
};

const formatHourLabel = (value) => `${String(Number(value || 0)).padStart(2, '0')}:00`;

const minDateValue = (left, right) => {
  if (!left) return right ?? null;
  if (!right) return left;
  return Date.parse(right) < Date.parse(left) ? right : left;
};

const maxDateValue = (left, right) => {
  if (!left) return right ?? null;
  if (!right) return left;
  return Date.parse(right) > Date.parse(left) ? right : left;
};

const compactText = (value = '', maxLength = 18) => {
  const text = String(value || 'N/A');
  if (text.length <= maxLength) return text;
  if (maxLength <= 6) return text.slice(0, maxLength);
  const left = Math.ceil((maxLength - 3) / 2);
  const right = Math.floor((maxLength - 3) / 2);
  return `${text.slice(0, left)}...${text.slice(text.length - right)}`;
};

const formatEntryExitFamilyRouteLabel = (route = null, fallbackKey = '') => {
  const parts = [
    route?.harmonic_type,
    route?.market,
    route?.family_bin,
    route?.family_size_bucket,
    route?.family_time_bin,
  ].filter(Boolean);

  return parts.length ? parts.join(' / ') : compactText(fallbackKey || 'Family', 18);
};

const optionalNumber = (value) => {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
};

const formatDecimal = (value, digits = 2) =>
  Number.isFinite(Number(value)) ? Number(value).toFixed(digits) : '0.00';

const formatSignedR = (value, digits = 2) => {
  if (!Number.isFinite(Number(value))) return 'N/A';
  const parsed = Number(value);
  return `${parsed > 0 ? '+' : ''}${parsed.toFixed(digits)}R`;
};

const formatNullableDecimal = (value, digits = 2, fallback = 'N/A') => {
  if (value === null || value === undefined || value === '') {
    return fallback;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed.toFixed(digits) : fallback;
};

const formatScorePercent = (value, digits = 1) => {
  if (!Number.isFinite(Number(value))) return 'N/A';
  return `${formatDecimal(Number(value) * 100, digits)}%`;
};

const formatSignedPercent = (value, digits = 1) => {
  if (!Number.isFinite(Number(value))) return '';
  const parsed = Number(value);
  return `${parsed > 0 ? '+' : ''}${parsed.toFixed(digits)}%`;
};

const formatGapDuration = (minutes) => {
  const parsed = Number(minutes);
  if (!Number.isFinite(parsed)) return '0.0m';
  if (Math.abs(parsed) >= 120) {
    return `${formatDecimal(parsed / 60, 1)}h`;
  }
  return `${formatDecimal(parsed, 1)}m`;
};

const formatTrendLabel = (value = '') =>
  String(value || 'Unlabeled')
    .split('_')
    .filter(Boolean)
    .map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
    .join(' ');

const formatTrendAlignmentLabel = (value = '') => {
  if (value === 'with_trend') return 'With Trend';
  if (value === 'against_trend') return 'Fighting Trend';
  if (value === 'neutral') return 'Neutral';
  return formatTrendLabel(value || 'Unknown');
};

const formatRatePercent = (value, digits = 1) =>
  Number.isFinite(Number(value)) ? (Number(value) * 100).toFixed(digits) : '0.0';

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

// Temporary performance isolation: keep the standalone Entry / Exit page focused
// on selected build data only while we diagnose the slow follow-on dashboard loads.
const ENTRY_EXIT_STANDALONE_BUILD_ONLY = true;

const parseTemplateRuleJson = (template = {}) => {
  if (!template.rule_json || typeof template.rule_json !== 'string') {
    return null;
  }

  try {
    return JSON.parse(template.rule_json);
  } catch (_error) {
    return null;
  }
};

const getTemplateEntryOffset = (template = {}) => {
  const parsedRule = parseTemplateRuleJson(template);
  const ruleOffset = Number(parsedRule?.entry?.offset_from_confirmation);
  if (Number.isFinite(ruleOffset)) {
    return ruleOffset;
  }

  return null;
};

const getEntryExitTemplateRiskLabel = (template = {}) => {
  const riskBasis = String(template.risk_basis || '').toLowerCase();
  const parsedRule = parseTemplateRuleJson(template);
  const parsedBasis = String(parsedRule?.stop?.basis || '').toLowerCase();
  const label = riskBasis || parsedBasis;
  return label === 'xa_price_length' ? 'XA' : 'CD';
};

const formatEntryExitTemplateRule = (template = {}) => {
  const direction = template.direction_mode === 'inverse_pattern' ? 'INV' : 'PAT';
  const entryOffset = getTemplateEntryOffset(template);
  const entry = entryOffset ? `C+${entryOffset}` : formatRouteMode(template.entry_kind);
  const risk = `${formatDecimal(template.risk_multiple, 3)}${getEntryExitTemplateRiskLabel(template)}`;
  const target = `${formatDecimal(template.target_r, 2).replace('.00', '')}R`;
  return `${direction} ${entry} ${risk} ${target}`;
};

const getEntryExitTemplateDirectionLabel = (template = {}) =>
  template.direction_mode === 'inverse_pattern' ? 'Inverse Pattern' : 'With Pattern';

const getEntryExitTemplateEntryLabel = (template = {}) => {
  const entryOffset = getTemplateEntryOffset(template);
  return entryOffset ? `Confirmation + ${entryOffset}` : formatRouteMode(template.entry_kind);
};

const getEntryExitTemplateRiskValue = (template = {}) =>
  `${formatDecimal(template.risk_multiple, 3)} ${getEntryExitTemplateRiskLabel(template)}`;

const ENTRY_EXIT_CONDITION_LABELS = {
  market: 'Pattern Market',
  symbol: 'Symbol',
  root_symbol: 'Root Symbol',
  source_timeframe: 'Timeframe',
  harmonic_type: 'Harmonic Type',
  family_bin: 'Family Bin',
  family_size_bucket: 'Family Size',
  family_time_bin: 'Family Time',
  family_x_strictness: 'Family X',
  trend_3m: '3M Trend',
  trend_6m: '6M Trend',
  trend_12m: '12M Trend',
  trade_direction: 'Trade Direction',
  exit_reason: 'Exit Reason',
  confirm_year: 'Year',
  confirm_quarter: 'Quarter',
  confirm_session: 'Session',
  pattern_length_bucket: 'Pattern Length',
};

const ENTRY_EXIT_CONDITION_ORDER = [
  'harmonic_type',
  'family_bin',
  'family_size_bucket',
  'family_time_bin',
  'family_x_strictness',
  'trend_3m',
  'trend_6m',
  'trend_12m',
  'symbol',
  'root_symbol',
  'source_timeframe',
  'market',
  'trade_direction',
  'exit_reason',
  'confirm_year',
  'confirm_quarter',
  'confirm_session',
  'pattern_length_bucket',
];

const ENTRY_EXIT_EDGE_FEATURE_ORDER = [
  'harmonic_type',
  'root_symbol',
  'market',
  'family_size_bucket',
  'family_time_bin',
  'family_bin',
  'confirm_session',
  'pattern_length_bucket',
  'trade_direction',
];

const classifyEntryExitEdge = ({ evalCount, avgRLift, wrLift }) => {
  if (evalCount < 100) return 'Thin Sample';
  if (avgRLift <= 0) return 'Avoid';
  if (wrLift >= 2) return 'Strong';
  if (wrLift <= 0) return 'Payoff Edge';
  return 'Modest Edge';
};

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

const formatInspectorHoverDate = (value) => {
  if (!value) return '--';

  const textValue = String(value);
  const textMatch = textValue.match(/^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2})/);
  if (textMatch) {
    return `${textMatch[1]} ${textMatch[2]}`;
  }

  const parsedDate = new Date(value);
  if (Number.isNaN(parsedDate.getTime())) {
    return textValue;
  }

  const year = parsedDate.getFullYear();
  const month = `${parsedDate.getMonth() + 1}`.padStart(2, '0');
  const day = `${parsedDate.getDate()}`.padStart(2, '0');
  const hour = `${parsedDate.getHours()}`.padStart(2, '0');
  const minute = `${parsedDate.getMinutes()}`.padStart(2, '0');
  return `${year}-${month}-${day} ${hour}:${minute}`;
};

const formatInspectorHoverPrice = (value) => {
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

const formatInspectorHoverVolume = (value) => {
  if (value === null || value === undefined || value === '') {
    return '--';
  }

  const numericValue = Number(value);
  return Number.isFinite(numericValue) ? numericValue.toLocaleString() : '--';
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

const TIMEFRAME_OPTIONS = [
  { value: 'All', label: 'All' },
  { value: '1m', label: '1m' },
  { value: '3m', label: '3m' },
  { value: '5m', label: '5m' },
  { value: '15m', label: '15m' },
  { value: '30m', label: '30m' },
  { value: '1h', label: '1h' },
  { value: '4h', label: '4h' },
  { value: '12h', label: '12h' },
  { value: '1d', label: '1d' },
  { value: 'daily', label: 'Daily' },
];

const FUTURES_EXCHANGE_BY_ROOT = {
  '6A': 'CME',
  '6B': 'CME',
  '6C': 'CME',
  '6E': 'CME',
  '6J': 'CME',
  '6M': 'CME',
  '6N': 'CME',
  '6S': 'CME',
  BTC: 'CME',
  CL: 'NYMEX',
  EMD: 'CME',
  ES: 'CME',
  GC: 'COMEX',
  GF: 'CME',
  HE: 'CME',
  HG: 'COMEX',
  HO: 'NYMEX',
  KE: 'CBOT',
  LE: 'CME',
  M2K: 'CME',
  MCL: 'NYMEX',
  MES: 'CME',
  MGC: 'COMEX',
  MNQ: 'CME',
  NG: 'NYMEX',
  NKD: 'CME',
  NQ: 'CME',
  PA: 'NYMEX',
  PL: 'NYMEX',
  QG: 'NYMEX',
  QM: 'NYMEX',
  RB: 'NYMEX',
  RTY: 'CME',
  SI: 'COMEX',
  UB: 'CBOT',
  YM: 'CBOT',
  ZB: 'CBOT',
  ZC: 'CBOT',
  ZF: 'CBOT',
  ZL: 'CBOT',
  ZM: 'CBOT',
  ZN: 'CBOT',
  ZS: 'CBOT',
  ZT: 'CBOT',
  ZW: 'CBOT',
};

// TODO: Move this display-only mapping into a DB-backed Symbol Master / market catalog table.
const normalizeFuturesRootSymbol = (rootSymbol = '') => {
  const value = String(rootSymbol || '').trim().toUpperCase();
  const treasuryContractRoot = value.match(/^(ZB|ZN)[FGHJKMNQUVXZ]\d{1,2}$/);
  return treasuryContractRoot ? treasuryContractRoot[1] : value;
};

const getFuturesExchange = (rootSymbol = '') =>
  FUTURES_EXCHANGE_BY_ROOT[normalizeFuturesRootSymbol(rootSymbol)] ?? null;

const getExchangeClassSuffix = (exchange = '') =>
  String(exchange || 'unknown').toLowerCase().replace(/[^a-z0-9]+/g, '-');

const DEFAULT_PATTERN_SCAN_FIT = {
  key: 'default-fit-on',
  label: 'Default Fit ON',
  detail: 'Default fit only',
};

const getPatternCatalogSource = (row = {}) => {
  const tableName = String(row.table_name || '').toLowerCase();
  if (tableName.includes('futures') || tableName.includes('contract')) return 'Futures';
  if (tableName.includes('stock') || tableName === 'candles' || tableName.includes('daily')) return 'Stocks';
  return 'Other';
};

const getPatternCatalogSourceKey = (source = '') =>
  String(source || 'other').toLowerCase().replace(/[^a-z0-9]+/g, '-');

const getPatternScanProfileKey = ({ source, timeframe, fitKey }) =>
  [source || 'Other', timeframe || 'unknown', fitKey || DEFAULT_PATTERN_SCAN_FIT.key]
    .map((part) => String(part).toLowerCase().replace(/[^a-z0-9]+/g, '-'))
    .join('__');

const getTimeframeSortIndex = (timeframe = '') => {
  const index = TIMEFRAME_OPTIONS.findIndex((option) => option.value === timeframe);
  return index === -1 ? TIMEFRAME_OPTIONS.length : index;
};

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

const ENTRY_EXIT_PROP_RULES = {
  profitTargetR: 30,
  dailyLossR: 10,
  maxDrawdownR: 20,
};

const getEntryExitCooldownLabel = (run = {}) => {
  const cooldownMinutes = Number(run.trade_cooldown_minutes || 0);
  if (cooldownMinutes > 0) {
    return `${formatNumber(cooldownMinutes)}m Cooldown`;
  }
  return run.one_trade_per_minute ? '1 Per Minute' : null;
};

const getEntryExitCooldownShortLabel = (run = {}) => {
  const cooldownMinutes = Number(run.trade_cooldown_minutes || 0);
  if (cooldownMinutes > 0) {
    return `${formatNumber(cooldownMinutes)}m gap`;
  }
  return run.one_trade_per_minute ? '1/min' : null;
};

const getEntryExitSimulationYearKey = (run = {}) => {
  const year = Number(run.test_year || 0);
  return year > 0 ? String(year) : 'all';
};

const getEntryExitSimulationYearLabel = (run = {}) => {
  const year = Number(run.test_year || 0);
  return year > 0 ? String(year) : 'All';
};

const getEntryExitPlaybookName = (run = {}, manualFamilyBansApplied = 0) => {
  const pieces = [];
  if (manualFamilyBansApplied) {
    pieces.push('Manual Ban');
  } else {
    pieces.push('Base');
  }

  const executionPieces = [];
  if (run.one_trade_at_a_time) {
    executionPieces.push('1 Trade Total');
  } else if (run.one_trade_per_root_symbol) {
    executionPieces.push('1 Per Root');
  }
  const cooldownLabel = getEntryExitCooldownLabel(run);
  if (cooldownLabel) {
    executionPieces.push(cooldownLabel);
  }
  if (run.daily_loss_lockout) {
    executionPieces.push('Daily Lockout');
  }
  if (run.near_pass_protection) {
    executionPieces.push('Near Pass Protect');
  }
  if (run.loss_cluster_day_lockout) {
    const lossCount = Number(run.loss_cluster_loss_count || 0) || 3;
    const windowMinutes = Number(run.loss_cluster_window_minutes || 0) || 60;
    executionPieces.push(`${formatNumber(lossCount)}L/${formatNumber(windowMinutes)}m Guard`);
  }
  if (executionPieces.length) {
    pieces.push(executionPieces.join(' + '));
  } else {
    pieces.push('Overlap Off');
  }

  if (run.symbol_filter_enabled) {
    pieces.push('Symbol Gate');
  }
  if (/CL (?:is marked SKIP|excluded)/i.test(String(run.playbook_description || ''))) {
    pieces.push('CL Skip');
  }

  return pieces.join(' | ');
};

const buildEntryExitPlaybookRuleSections = ({
  run = {},
  playbookLabel = '',
  playbookName = '',
  manualFamilyBansApplied = 0,
} = {}) => {
  if (!run) return [];

  const title = [playbookLabel, playbookName].filter(Boolean).join(' - ');
  const cooldownMinutes = Number(run.trade_cooldown_minutes || 0);
  const sisterWindow = Number(run.sister_window_minutes || 0);
  const lossClusterCount = Number(run.loss_cluster_loss_count || 0) || 3;
  const lossClusterWindow = Number(run.loss_cluster_window_minutes || 0) || 60;
  const sections = [
    {
      title: 'Template Assignment',
      detail: title || 'Selected playbook',
      items: [
        {
          label: 'Build',
          value: compactText(run.train_run_id, 24),
          detail: `${formatRouteMode(run.source_scope)} ${run.source_timeframe || 'all-timeframe'}`,
        },
        {
          label: 'Template Rules',
          value: 'Unchanged',
          detail: 'playbook selects templates, it does not rewrite entries/exits',
        },
        {
          label: 'Family Mapping',
          value: 'Best Match',
          detail: 'each trade family gets its selected template',
        },
        ...(manualFamilyBansApplied
          ? [
              {
                label: 'Manual Ban',
                value: formatNumber(manualFamilyBansApplied),
                detail: 'family ban applied when this playbook was built',
                tone: 'skipped',
              },
            ]
          : []),
      ],
    },
  ];

  const timingItems = [];
  if (sisterWindow > 0) {
    timingItems.push({
      label: 'Twin Window',
      value: `${formatNumber(sisterWindow)}m`,
      detail: 'one candidate is chosen inside the twin window',
    });
  } else {
    timingItems.push({
      label: 'Twin Window',
      value: 'Off',
      detail: 'this playbook row does not group twin candidates',
    });
  }
  if (run.one_trade_at_a_time) {
    timingItems.push({
      label: 'Overlap',
      value: '1 Account',
      detail: 'only one active trade across the whole account',
      tone: 'skipped',
    });
  } else if (run.one_trade_per_root_symbol) {
    timingItems.push({
      label: 'Overlap',
      value: '1 Root',
      detail: 'only one active trade per root symbol',
      tone: 'skipped',
    });
  } else {
    timingItems.push({
      label: 'Overlap',
      value: 'Off',
      detail: 'overlapping accepted trades are not limited here',
    });
  }
  if (run.one_trade_per_minute) {
    timingItems.push({
      label: 'Entry Spacing',
      value: '1/min',
      detail: 'only one accepted entry per minute',
      tone: 'skipped',
    });
  }
  if (cooldownMinutes > 0) {
    timingItems.push({
      label: 'Trade Gap',
      value: `${formatNumber(cooldownMinutes)}m`,
      detail: 'rolling gap after each accepted win/loss trade',
      tone: 'skipped',
    });
  } else if (!run.one_trade_per_minute) {
    timingItems.push({
      label: 'Trade Gap',
      value: 'Off',
      detail: 'no extra rolling time gap after accepted trades',
    });
  }
  sections.push({
    title: 'Trade Timing',
    detail: 'when a routed candidate is allowed to become a trade',
    items: timingItems,
  });

  const riskItems = [];
  if (run.daily_loss_lockout) {
    riskItems.push({
      label: 'Daily Limit',
      value: 'Lockout',
      detail: '-10R stops trading for the rest of the date instead of failing the cycle',
      tone: 'skipped',
    });
  } else {
    riskItems.push({
      label: 'Daily Limit',
      value: 'Fail',
      detail: '-10R counts as a prop daily-loss failure',
      tone: 'loss',
    });
  }
  if (run.near_pass_protection) {
    riskItems.push({
      label: 'Near Pass',
      value: `${formatDecimal(run.near_pass_within_r, 0)}R`,
      detail: `near target, daily lockout tightens to -${formatDecimal(run.near_pass_daily_loss_r, 0)}R`,
      tone: 'skipped',
    });
  } else {
    riskItems.push({
      label: 'Near Pass',
      value: 'Off',
      detail: 'no extra protection near the pass target',
    });
  }
  if (run.loss_cluster_day_lockout) {
    riskItems.push({
      label: 'Loss Cluster',
      value: `${formatNumber(lossClusterCount)}L/${formatNumber(lossClusterWindow)}m`,
      detail: 'locks the rest of the date after clustered losses',
      tone: 'skipped',
    });
  } else {
    riskItems.push({
      label: 'Loss Cluster',
      value: 'Off',
      detail: 'clustered losses do not trigger a day lockout',
    });
  }
  sections.push({
    title: 'Prop Risk',
    detail: 'how the replay handles prop-firm account risk',
    items: riskItems,
  });

  return sections;
};

const buildEntryExitPlaybookDescription = (options = {}) =>
  buildEntryExitPlaybookRuleSections(options)
    .map((section) =>
      `${section.title}: ${section.items
        .map((item) => `${item.label} ${item.value}${item.detail ? ` (${item.detail})` : ''}`)
        .join('; ')}`
    )
    .join('\n\n');

const getEntryExitPlaybookRank = (run = {}, manualFamilyBansApplied = 0) => {
  let rank = manualFamilyBansApplied ? 100 : 0;
  if (run.one_trade_at_a_time) {
    rank += 1;
  } else if (run.one_trade_per_root_symbol) {
    rank += 2;
  }
  if (run.one_trade_per_minute || Number(run.trade_cooldown_minutes || 0) > 0) {
    rank += 4;
  }
  if (run.daily_loss_lockout) {
    rank += 8;
  }
  if (run.near_pass_protection) {
    rank += 16;
  }
  if (run.loss_cluster_day_lockout) {
    rank += 32;
  }
  if (!run.symbol_filter_enabled) {
    rank += 10;
  }
  return rank;
};

const getRouteKey = (route) => `${route.run_id}-${route.route_id}`;
const getPatternFamilyKey = (pattern = {}) => pattern.prop_strategy_id ?? pattern.pattern_family_key ?? null;
const getFamilyPatternKey = (pattern = {}) =>
  [
    pattern.pattern_id ?? '',
    pattern.pattern_group_id ?? '',
    pattern.symbol ?? '',
    pattern.d_date ?? '',
  ].join('|');
const getPatternBrowseText = (pattern = {}) =>
  [
    pattern.pattern_id,
    pattern.pattern_group_id,
    getPatternFamilyKey(pattern),
    pattern.symbol,
    pattern.market,
    pattern.harmonic_type,
    pattern.d_date,
    pattern.d_confirm_date,
  ]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();
const getRouteTradeKey = (trade = {}) =>
  [
    trade.test_index ?? 1,
    trade.trade_index ?? 0,
    trade.pattern_id ?? trade.pattern_group_id ?? '',
    trade.entry_date ?? '',
  ].join('|');

const getSimulationRawTradeKey = (trade = {}, fallbackIndex = 0) =>
  [
    trade.id ?? '',
    trade.setup_id ?? '',
    trade.pattern_id ?? trade.pattern_group_id ?? '',
    trade.entry_date ?? trade.d_confirm_date ?? '',
    fallbackIndex,
  ].join('|');

const getAiStage1TradeUid = (trade = {}) =>
  [
    trade.multi_valid_eval_run_id ?? trade.sim_run_id ?? '',
    trade.setup_id ?? trade.pattern_id ?? trade.pattern_group_id ?? '',
    trade.template_uid ?? '',
  ]
    .filter(Boolean)
    .join('|');

const isAiCandleWaveTrade = (trade = {}) =>
  String(trade.template_uid ?? '').startsWith('candle_wave_') ||
  String(trade.harmonic_type ?? '').toLowerCase() === 'candle wave' ||
  String(trade.pattern_family_key ?? '').toLowerCase().includes('candle wave');

const getAiStage1TradeTimeframe = (trade = {}) => {
  const sourceTimeframe = String(trade.source_timeframe ?? '').trim();
  if (sourceTimeframe && sourceTimeframe.toLowerCase() !== 'unknown') {
    return sourceTimeframe;
  }

  const familyTimeframe = String(trade.family_time_bin ?? '').trim();
  if (familyTimeframe) {
    return familyTimeframe;
  }

  return isAiCandleWaveTrade(trade) ? '2m' : sourceTimeframe || null;
};

const buildAiStage1VirtualPattern = (trade = {}) => {
  const timeframe = getAiStage1TradeTimeframe(trade);
  const direction = String(trade.trade_direction ?? trade.market ?? '').toUpperCase();
  const isShort = direction === 'SHORT' || direction === 'SELL';
  const signalDate = trade.d_confirm_date ?? trade.signal_date ?? trade.entry_date ?? trade.exit_date ?? null;
  const entryDate = trade.entry_date ?? signalDate;
  const exitDate = trade.exit_date ?? trade.target_date ?? entryDate;
  const canvasEndDate = getAiExitAuditCanvasEndDate(trade, timeframe, entryDate, exitDate);

  return {
    ...trade,
    prop_outcome_mode: 'phase1-family',
    source_timeframe: timeframe,
    sourceTimeframe: timeframe,
    pattern_id: trade.pattern_id ?? trade.setup_id ?? '',
    pattern_group_id: trade.pattern_group_id ?? trade.multi_valid_eval_run_id ?? '',
    harmonic_type: trade.harmonic_type || 'Candle Wave',
    market: isShort ? 'Bearish' : 'Bullish',
    trade_direction: direction || trade.trade_direction,
    d_date: signalDate,
    d_confirm_date: signalDate,
    entry_date: entryDate,
    target_date: exitDate,
    canvas_end_date: canvasEndDate,
    trade_date: exitDate,
    trade_enter_price: trade.trade_enter_price ?? trade.entry_price,
    trade_risk_exit_price: trade.trade_risk_exit_price ?? trade.stop_price,
    trade_reward_exit_price: trade.trade_reward_exit_price ?? trade.target_price,
    target_close: trade.exit_price ?? trade.trade_exit_price,
    trade_current_price: trade.exit_price ?? trade.trade_exit_price,
    exit_price: trade.exit_price ?? trade.trade_exit_price,
    trade_result:
      trade.trade_result ??
      (Number(trade.result_r) < 0 || String(trade.outcome ?? '').toLowerCase() === 'fail' ? 2 : 1),
  };
};

const getPatternXaOutcomeRowKey = (row = {}) =>
  [
    row.setup_id ?? '',
    row.pattern_id ?? row.pattern_group_id ?? '',
    row.d_confirm_date ?? '',
  ].join('|');

const normalizeSimulationRawTradeForCanvas = (trade = {}) => {
  const resultR = Number(trade.result_r);
  const isLoss =
    String(trade.outcome || '').toLowerCase() === 'fail' ||
    String(trade.exit_reason || '').toLowerCase() === 'stop' ||
    (Number.isFinite(resultR) && resultR < 0);
  const exitPrice =
    trade.exit_price ??
    (isLoss ? trade.stop_price : trade.target_price);
  const tradeUid = trade.trade_uid ?? trade.trade_id ?? trade.id ?? getAiStage1TradeUid(trade);

  return {
    ...trade,
    trade_uid: tradeUid || undefined,
    prop_outcome_mode: 'phase1-family',
    entry_date: trade.entry_date ?? trade.d_confirm_date ?? trade.d_date,
    target_date: trade.exit_date ?? trade.target_date ?? trade.entry_date ?? trade.d_confirm_date,
    trade_enter_price: trade.entry_price,
    trade_risk_exit_price: trade.stop_price,
    trade_reward_exit_price: trade.target_price,
    trade_current_price: exitPrice,
    target_close: exitPrice,
    trade_result: isLoss ? 2 : 1,
    result_r: Number.isFinite(resultR) ? resultR : trade.result_r,
    risk_points: trade.risk_points,
  };
};

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

const getPatternEventKey = (pattern = {}, fallbackIndex = 0) =>
  pattern.event_id || `solo:${getFamilyPatternKey(pattern)}:${fallbackIndex}`;

const getPatternTimelineTime = (pattern = {}) => {
  const parsed = Date.parse(pattern.d_confirm_date ?? pattern.d_date ?? pattern.entry_date ?? '');
  return Number.isFinite(parsed) ? parsed : Number.MAX_SAFE_INTEGER;
};

const groupTwinPatternRows = (rows = []) => {
  const groups = new Map();

  rows.forEach((pattern, index) => {
    const key = getPatternEventKey(pattern, index);
    const time = getPatternTimelineTime(pattern);
    const existing = groups.get(key);
    if (!existing || time < existing.time || (time === existing.time && index < existing.index)) {
      groups.set(key, { index, time });
    }
  });

  return rows
    .map((pattern, index) => ({
      index,
      pattern,
      group: groups.get(getPatternEventKey(pattern, index)) ?? { index, time: getPatternTimelineTime(pattern) },
    }))
    .sort((left, right) => {
      if (left.group.time !== right.group.time) {
        return left.group.time - right.group.time;
      }
      if (left.group.index !== right.group.index) {
        return left.group.index - right.group.index;
      }
      const leftRank = Number.isFinite(Number(left.pattern.event_rank))
        ? Number(left.pattern.event_rank)
        : left.index + 1;
      const rightRank = Number.isFinite(Number(right.pattern.event_rank))
        ? Number(right.pattern.event_rank)
        : right.index + 1;
      if (leftRank !== rightRank) {
        return leftRank - rightRank;
      }
      const leftTime = getPatternTimelineTime(left.pattern);
      const rightTime = getPatternTimelineTime(right.pattern);
      if (leftTime !== rightTime) {
        return leftTime - rightTime;
      }
      return left.index - right.index;
    })
    .map((item) => item.pattern);
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

const getTimeframePaddingMs = (timeframe) => {
  const text = String(timeframe ?? '').trim().toLowerCase();
  const match = text.match(/^(\d+)\s*(m|min|minute|h|hr|hour|d|day)s?$/);
  if (!match) {
    return 30 * 60 * 1000;
  }

  const amount = Number(match[1]);
  const unit = match[2];
  if (!Number.isFinite(amount) || amount <= 0) {
    return 30 * 60 * 1000;
  }

  if (unit.startsWith('h')) {
    return Math.max(30 * 60 * 1000, amount * 2 * 60 * 60 * 1000);
  }

  if (unit.startsWith('d')) {
    return Math.max(30 * 60 * 1000, amount * 2 * 24 * 60 * 60 * 1000);
  }

  return Math.max(30 * 60 * 1000, amount * 3 * 60 * 1000);
};

const getTimeframeMs = (timeframe) => {
  const text = String(timeframe ?? '').trim().toLowerCase();
  const match = text.match(/^(\d+)\s*(m|min|minute|h|hr|hour|d|day)s?$/);
  if (!match) {
    return 60 * 1000;
  }

  const amount = Number(match[1]);
  const unit = match[2];
  if (!Number.isFinite(amount) || amount <= 0) {
    return 60 * 1000;
  }

  if (unit.startsWith('h')) {
    return amount * 60 * 60 * 1000;
  }

  if (unit.startsWith('d')) {
    return amount * 24 * 60 * 60 * 1000;
  }

  return amount * 60 * 1000;
};

const addTimeframeBars = (value, timeframe, bars) => {
  const parsed = new Date(value);
  const barCount = Number(bars);
  if (Number.isNaN(parsed.getTime()) || !Number.isFinite(barCount)) {
    return null;
  }

  return formatDateTimeForServer(new Date(parsed.getTime() + getTimeframeMs(timeframe) * barCount));
};

const getExitModelWindowBars = (trade = {}) => {
  const runId = String(trade.exit_model_run_id ?? '').toLowerCase();
  const match = runId.match(/dyn(\d+)s\d+/);
  return match ? Number(match[1]) : 120;
};

const getAiExitAuditCanvasEndDate = (trade = {}, timeframe, entryDate, exitDate) => {
  if (!trade.exit_model_run_id) {
    return exitDate;
  }

  const modelWindowBars = getExitModelWindowBars(trade);
  const modelWindowEnd = addTimeframeBars(entryDate, timeframe, modelWindowBars);
  const postExitEnd = addTimeframeBars(exitDate, timeframe, Math.max(30, Math.round(modelWindowBars / 2)));
  const candidates = [modelWindowEnd, postExitEnd, exitDate]
    .map((value) => {
      const parsed = new Date(value);
      return Number.isNaN(parsed.getTime()) ? null : parsed;
    })
    .filter(Boolean);

  if (!candidates.length) {
    return exitDate;
  }

  return formatDateTimeForServer(new Date(Math.max(...candidates.map((date) => date.getTime()))));
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
    pattern.canvas_end_date,
    pattern.xa_outcome_hit_date,
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
  const exitDate =
    pattern.canvas_end_date ??
    pattern.xa_outcome_hit_date ??
    pattern.target_date ??
    pattern.trade_date ??
    null;
  const parsedExitDate = exitDate ? new Date(exitDate) : null;
  const exitTime =
    parsedExitDate && !Number.isNaN(parsedExitDate.getTime())
      ? parsedExitDate.getTime()
      : null;

  return {
    startDate: formatDateTimeForServer(new Date(Math.min(...dateValues) - paddingMs)),
    endDate:
      exitTime !== null
        ? formatDateTimeForServer(
            new Date(exitTime + getTimeframePaddingMs(pattern.source_timeframe ?? pattern.sourceTimeframe))
          )
        : formatDateTimeForServer(new Date(Math.max(...dateValues) + paddingMs)),
    sourceTimeframe: pattern.source_timeframe ?? pattern.sourceTimeframe ?? null,
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

const addLiveCanvasDisplayDates = (candles = []) =>
  candles.map((candle) => ({
    ...candle,
    // The model table stores the 2m bucket/open timestamp. NinjaTrader labels the same bar at close time.
    candle_display_date:
      addTimeframeBars(candle.candle_date, LIVE_CANVAS_TIMEFRAME, 1) ?? candle.candle_date,
    candle_display_timezone: LIVE_CANVAS_DISPLAY_TIME_ZONE,
  }));

const getLiveCanvasDisplayDate = (candle) =>
  candle?.candle_display_date ?? candle?.candle_date ?? null;

const deriveNinjaTraderModelSymbol = (instrument = '', rootSymbol = LIVE_CANVAS_ROOT) => {
  const root = String(rootSymbol || LIVE_CANVAS_ROOT).toUpperCase();
  const text = String(instrument || '').toUpperCase().trim();
  const monthNameMatch = text.match(/\b(JAN|FEB|MAR|APR|MAY|JUN|JUL|AUG|SEP|OCT|NOV|DEC)(\d{2,4})\b/);
  if (monthNameMatch) {
    return `${root}${NINJATRADER_MONTH_CODES[monthNameMatch[1]]}${monthNameMatch[2].slice(-1)}`;
  }
  const compactMatch = text.match(/\b([FGHJKMNQUVXZ])(\d{1,2})\b/);
  if (compactMatch) {
    return `${root}${compactMatch[1]}${compactMatch[2].slice(-1)}`;
  }
  return root;
};

const mapLiveBarSnapshotToCanvasCandle = (snapshot = null) => {
  if (!snapshot) {
    return null;
  }

  const bucketTime =
    snapshot.bucket_time_utc ??
    addTimeframeBars(snapshot.candle_time_utc ?? snapshot.candle_time, LIVE_CANVAS_TIMEFRAME, -1);
  const candleDate = formatCandleDateForChart(bucketTime);
  const open = Number(snapshot.open);
  const high = Number(snapshot.high);
  const low = Number(snapshot.low);
  const close = Number(snapshot.close ?? snapshot.last_price);

  if (!candleDate || !Number.isFinite(open) || !Number.isFinite(high) || !Number.isFinite(low) || !Number.isFinite(close)) {
    return null;
  }

  const displayDate =
    formatCandleDateForChart(snapshot.candle_time_utc) ??
    addTimeframeBars(candleDate, LIVE_CANVAS_TIMEFRAME, 1) ??
    candleDate;
  const rootSymbol = String(snapshot.root_symbol || LIVE_CANVAS_ROOT).toUpperCase();
  const symbol = deriveNinjaTraderModelSymbol(snapshot.instrument, rootSymbol);

  return {
    symbol,
    root_symbol: rootSymbol,
    candle_date: candleDate,
    candle_display_date: displayDate,
    candle_display_timezone: LIVE_CANVAS_DISPLAY_TIME_ZONE,
    open,
    high,
    low,
    close,
    volume: Number.isFinite(Number(snapshot.volume)) ? Number(snapshot.volume) : null,
    candle_open: open,
    candle_high: high,
    candle_low: low,
    candle_close: close,
    candle_volume: Number.isFinite(Number(snapshot.volume)) ? Number(snapshot.volume) : null,
    candle_is_live_snapshot: true,
    candle_snapshot_time: snapshot.snapshot_time_utc ?? snapshot.updated_at ?? snapshot.received_at ?? null,
    candle_last_price: Number.isFinite(Number(snapshot.last_price)) ? Number(snapshot.last_price) : close,
  };
};

const getTrendEventKey = (trend) => String(trend?.event_uid ?? trend?.id ?? '');
const getOracleTrendKey = (trend) => `oracle:${trend?.oracle_trade_id ?? trend?.symbol ?? 'trend'}:${trend?.entry_date ?? ''}`;

const mapOracleTrendToLiveCanvasTrend = (trend = {}) => ({
  raw_source: 'oracle',
  event_uid: getOracleTrendKey(trend),
  event_type: 'oracle_trend',
  model_symbol: trend.symbol ?? null,
  root_symbol: trend.root_symbol ?? LIVE_CANVAS_ROOT,
  timeframe: trend.timeframe ?? LIVE_CANVAS_TIMEFRAME,
  candle_time: trend.entry_date ?? null,
  ts_utc: trend.entry_date ?? null,
  direction: trend.direction ?? null,
  level2_score: null,
  stage2_score: trend.quality_score ?? null,
  status: 'oracle',
  paper_entry_status: 'accepted',
  paper_exit_status: 'closed',
  paper_entry_candle_time: trend.entry_date ?? null,
  paper_entry_ts_utc: trend.entry_date ?? null,
  paper_exit_candle_time: trend.exit_date ?? null,
  paper_exit_ts_utc: trend.exit_date ?? null,
  entry_price: trend.entry_price ?? null,
  stop_price: trend.stop_price ?? null,
  risk_ticks: trend.risk_ticks ?? null,
  paper_entry_price: trend.entry_price ?? null,
  paper_stop_price: trend.stop_price ?? null,
  paper_risk_ticks: trend.risk_ticks ?? null,
  paper_exit_price: trend.exit_price ?? null,
  paper_exit_reason: trend.outcome ?? 'oracle',
  paper_result_r: trend.result_r ?? null,
  oracle_trade_id: trend.oracle_trade_id ?? null,
  oracle_quality_score: trend.quality_score ?? null,
});

const getLiveTradeSignalKey = (trade) => `signal:${trade?.signal_uid ?? trade?.id ?? ''}`;

const getLiveTradeSignalResultLabel = (trade = {}) => {
  const explicitLabel = String(trade.result_label || '').trim();
  if (explicitLabel) {
    return explicitLabel;
  }
  const pnl = Number(trade.realized_accounting_dollars ?? trade.realized_execution_dollars ?? trade.realized_ticks);
  if (String(trade.status || '').toLowerCase() === 'completed' && Number.isFinite(pnl)) {
    if (pnl > 0) return 'Win';
    if (pnl < 0) return 'Loss';
    return 'Flat';
  }
  if (String(trade.status || '').toLowerCase() === 'triggered') {
    return 'Open';
  }
  return trade.status || 'N/A';
};

const getLiveTradeSignalResultTone = (trade = {}) => {
  const label = getLiveTradeSignalResultLabel(trade).toLowerCase();
  if (label === 'win') return 'win';
  if (label === 'loss') return 'loss';
  if (label === 'open') return 'open';
  return '';
};

const formatLiveMoney = (value) =>
  Number.isFinite(Number(value))
    ? Number(value).toLocaleString('en-US', {
        style: 'currency',
        currency: 'USD',
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
      })
    : 'N/A';

const formatSignedLiveMoney = (value) => {
  if (!Number.isFinite(Number(value))) {
    return 'N/A';
  }
  const parsed = Number(value);
  const formatted = formatLiveMoney(Math.abs(parsed));
  return `${parsed > 0 ? '+' : parsed < 0 ? '-' : ''}${formatted}`;
};

const parseLiveTradeNotes = (trade = {}) => {
  const notes = trade.notes;
  if (!notes || typeof notes !== 'string') {
    return {};
  }

  try {
    const parsed = JSON.parse(notes);
    return parsed && typeof parsed === 'object' ? parsed : {};
  } catch (_error) {
    return {};
  }
};

const getLiveTradeStage2Score = (trade = {}) => {
  const notes = parseLiveTradeNotes(trade);
  return trade.stage2_score ?? notes.stage2_score ?? trade.level2_score ?? notes.level2_score ?? null;
};

const mapLiveTradeSignalToCanvasTrend = (trade = {}) => {
  const notes = parseLiveTradeNotes(trade);
  const direction = String(trade.side || '').toUpperCase();
  const entryPrice = trade.actual_trigger_price ?? trade.expected_price ?? null;
  const stopPrice = trade.stop_price ?? null;
  const tickSize = Number(trade.tick_size);
  const riskTicks =
    Number.isFinite(Number(entryPrice)) && Number.isFinite(Number(stopPrice)) && Number.isFinite(tickSize) && tickSize > 0
      ? Math.abs(Number(entryPrice) - Number(stopPrice)) / tickSize
      : null;
  const resultR =
    Number.isFinite(Number(trade.realized_ticks)) && Number.isFinite(Number(riskTicks)) && Number(riskTicks) > 0
      ? Number(trade.realized_ticks) / Number(riskTicks)
      : null;

  return {
    raw_source: 'live_signal',
    event_uid: getLiveTradeSignalKey(trade),
    event_type: 'live_trade_signal',
    model_symbol: trade.instrument ?? LIVE_CANVAS_SIGNAL_INSTRUMENT,
    root_symbol: trade.root_symbol ?? LIVE_CANVAS_ROOT,
    timeframe: LIVE_CANVAS_TIMEFRAME,
    candle_time: trade.triggered_at ?? trade.expected_time ?? trade.created_at ?? null,
    ts_utc: trade.triggered_at ?? trade.expected_time ?? trade.created_at ?? null,
    direction,
    level2_score: trade.level2_score ?? notes.level2_score ?? null,
    stage2_score: trade.stage2_score ?? notes.stage2_score ?? null,
    status: trade.status ?? null,
    paper_entry_status: String(trade.status || '').toLowerCase() === 'completed' ? 'accepted' : trade.status ?? null,
    paper_exit_status: String(trade.status || '').toLowerCase() === 'completed' ? 'closed' : trade.status ?? null,
    paper_entry_candle_time: trade.triggered_at ?? trade.expected_time ?? trade.created_at ?? null,
    paper_entry_ts_utc: trade.triggered_at ?? trade.expected_time ?? trade.created_at ?? null,
    paper_exit_candle_time: trade.exit_received_at ?? null,
    paper_exit_ts_utc: trade.exit_received_at ?? null,
    entry_price: entryPrice,
    stop_price: stopPrice,
    risk_ticks: riskTicks,
    paper_entry_price: entryPrice,
    paper_stop_price: stopPrice,
    paper_risk_ticks: riskTicks,
    paper_exit_price: trade.exit_price ?? null,
    paper_exit_reason: getLiveTradeSignalResultLabel(trade),
    paper_result_r: resultR,
    signal_uid: trade.signal_uid ?? null,
  };
};

const getDateMinuteKey = (value) => {
  if (!value) {
    return '';
  }

  return String(value).replace('T', ' ').slice(0, 16);
};

const getNearestCandleIndexByTime = (candles = [], value, timeframe = LIVE_CANVAS_TIMEFRAME) => {
  const targetTime = getDateTimeForCompare(value);
  const timeframeMs = getTimeframeMs(timeframe);
  const maxDistanceMs = Math.max(timeframeMs, 60 * 1000);

  if (targetTime === null) {
    return null;
  }

  let bestIndex = null;
  let bestDistance = Number.POSITIVE_INFINITY;

  candles.forEach((candle, index) => {
    const candleTime = getDateTimeForCompare(candle?.candle_date);
    if (candleTime === null) {
      return;
    }

    const distance = Math.abs(candleTime - targetTime);
    if (distance < bestDistance) {
      bestDistance = distance;
      bestIndex = index + 1;
    }
  });

  return bestDistance <= maxDistanceMs ? bestIndex : null;
};

const findCandleIndexByTime = (candles = [], value) => {
  const targetKey = getDateMinuteKey(value);

  if (!targetKey) {
    return null;
  }

  const matchIndex = candles.findIndex((candle) => getDateMinuteKey(candle?.candle_date) === targetKey);
  return matchIndex >= 0
    ? matchIndex + 1
    : getNearestCandleIndexByTime(candles, value, LIVE_CANVAS_TIMEFRAME);
};

const findCandleIndexByTimes = (candles = [], values = []) => {
  for (const value of values) {
    const matchIndex = findCandleIndexByTime(candles, value);

    if (matchIndex !== null) {
      return matchIndex;
    }
  }

  return null;
};

const buildLiveCanvasPattern = (candles = [], selectedTrend = null, canvasSymbol = LIVE_CANVAS_SYMBOL) => {
  const latestCandle = candles[0] ?? null;
  const confirmIndex = selectedTrend
    ? findCandleIndexByTimes(candles, [selectedTrend.ts_utc, selectedTrend.candle_time])
    : null;
  const entryIndex = selectedTrend
    ? findCandleIndexByTimes(candles, [
        selectedTrend.paper_entry_ts_utc,
        selectedTrend.paper_entry_candle_time,
        selectedTrend.ts_utc,
        selectedTrend.candle_time,
      ])
    : null;
  const exitIndex = selectedTrend
    ? findCandleIndexByTimes(candles, [
        selectedTrend.paper_exit_ts_utc,
        selectedTrend.paper_exit_candle_time,
      ])
    : null;
  const centerLatestCandle = !selectedTrend && Boolean(candles.length);
  const focusIndex = entryIndex ?? confirmIndex ?? exitIndex ?? (centerLatestCandle ? 1 : null);
  const direction = String(selectedTrend?.direction ?? '').toUpperCase();
  const trendKey = getTrendEventKey(selectedTrend);

  return {
    raw_candle_view: true,
    raw_visible_candles: centerLatestCandle
      ? LIVE_CANVAS_CURRENT_VISIBLE_CANDLES
      : LIVE_CANVAS_TREND_VISIBLE_CANDLES,
    raw_focus_index: focusIndex,
    raw_center_focus_at_midpoint: centerLatestCandle,
    raw_follow_latest: centerLatestCandle,
    raw_vertical_zoom: centerLatestCandle ? LIVE_CANVAS_CURRENT_VERTICAL_ZOOM : 1,
    raw_viewport_key: trendKey || `${LIVE_CANVAS_ROOT}-${LIVE_CANVAS_TIMEFRAME}-live`,
    raw_selected_trend: Boolean(selectedTrend),
    raw_trend_source: selectedTrend?.raw_source ?? 'model',
    trend_confirm_index: confirmIndex,
    trend_event_uid: trendKey || null,
    trend_stage2_score: selectedTrend?.stage2_score ?? null,
    trend_level2_score: selectedTrend?.level2_score ?? null,
    trend_entry_status: selectedTrend?.paper_entry_status ?? selectedTrend?.status ?? null,
    pattern_group_id: `${LIVE_CANVAS_ROOT}-${canvasSymbol}-${LIVE_CANVAS_TIMEFRAME}-live`,
    pattern_id: `${LIVE_CANVAS_ROOT}-${canvasSymbol}-${LIVE_CANVAS_TIMEFRAME}-live-candles`,
    symbol: canvasSymbol,
    root_symbol: LIVE_CANVAS_ROOT,
    source_timeframe: LIVE_CANVAS_TIMEFRAME,
    harmonic_type: 'Live Candles',
    market: direction === 'SHORT' ? 'Bearish' : 'Bullish',
    trade_direction: direction || null,
    prop_outcome_mode: 'live-candles',
    d_date: latestCandle?.candle_date ?? null,
    d_confirm_date: latestCandle?.candle_date ?? null,
    entry: entryIndex,
    exit_date: exitIndex,
    trade_enter_price: selectedTrend?.paper_entry_price ?? selectedTrend?.entry_price ?? null,
    trade_risk_exit_price: selectedTrend?.paper_stop_price ?? selectedTrend?.stop_price ?? null,
    trade_exit_price: selectedTrend?.paper_exit_price ?? null,
    exit_price: selectedTrend?.paper_exit_price ?? null,
    result_r: selectedTrend?.paper_result_r ?? null,
    exit_reason: selectedTrend?.paper_exit_reason ?? null,
    pattern_ABCD_bar_length: candles.length,
  };
};

const getLiveCanvasSignature = (candles = []) => {
  const latestCandle = candles[0] ?? {};
  return [
    candles.length,
    latestCandle.candle_date ?? '',
    latestCandle.candle_open ?? '',
    latestCandle.candle_high ?? '',
    latestCandle.candle_low ?? '',
    latestCandle.candle_close ?? '',
    latestCandle.candle_volume ?? '',
  ].join('|');
};

const getLiveCanvasCandleKey = (candle = {}) =>
  `${String(candle.symbol ?? candle.root_symbol ?? '').toUpperCase()}|${getDateMinuteKey(candle.candle_date)}`;

const formatScannerCount = (value) =>
  Number.isFinite(Number(value)) ? formatNumber(Number(value)) : '-';

const formatScannerScore = (value) =>
  Number.isFinite(Number(value)) ? formatScorePercent(value, 0) : '-';

const getScannerActivityLabel = (row = {}) => {
  const eventType = String(row.event_type || '').toLowerCase();
  if (eventType === 'cycle_scored') return 'Candle Check';
  if (eventType === 'level2_pick') return 'L2 Pick';
  if (eventType === 'trend_confirmed') return 'Stage 2';
  if (eventType === 'stage2_expired') return 'Expired';
  if (eventType === 'order_signal') return 'Order';
  if (eventType === 'heartbeat') return 'Heartbeat';
  if (eventType === 'feed_stale') return 'Feed Paused';
  if (eventType === 'feed_restored') return 'Feed Restored';
  if (eventType === 'feed_gap_detected') return 'Feed Gap';
  if (eventType === 'feed_backlog_reset') return 'Backlog Reset';
  return formatRouteMode(row.event_type || 'Event');
};

const getScannerActivityState = (row = {}) => {
  const details = row.details ?? {};
  const eventType = String(row.event_type || '').toLowerCase();
  if (eventType === 'cycle_scored') {
    const confirms = Number(details.trend_confirms ?? 0);
    const picks = Number(details.level2_picks ?? 0);
    if (confirms > 0) return `${formatNumber(confirms)} confirm`;
    if (picks > 0) return `${formatNumber(picks)} watch`;
    return 'Scored';
  }
  if (eventType.startsWith('feed_')) {
    const outageCount = Number(details.feed_outage_count ?? 0);
    const status = formatRouteMode(row.status || row.event_type || 'Seen');
    return outageCount > 0 ? `${status} #${formatNumber(outageCount)}` : status;
  }
  return formatRouteMode(row.status || row.event_type || 'Seen');
};

const getScannerActivityTone = (row = {}) => {
  const eventType = String(row.event_type || '').toLowerCase();
  const status = String(row.status || '').toLowerCase();
  if (eventType === 'cycle_scored') return 'open';
  if (eventType === 'heartbeat') return 'open';
  if (eventType === 'feed_restored') return 'win';
  if (eventType === 'feed_stale' || eventType === 'feed_gap_detected' || eventType === 'feed_backlog_reset') return 'loss';
  if (status.includes('confirmed') || status.includes('queued')) return 'win';
  if (status.includes('rejected') || status.includes('expired') || status.includes('blocked') || status.includes('failed')) {
    return 'loss';
  }
  return '';
};

const getScannerCandidateUid = (row = {}) =>
  String(row?.details?.candidate_uid ?? row?.candidate_uid ?? '').trim();

const getWatchingTrendMarkerSignature = (markers = []) =>
  markers
    .map((marker) => [
      marker.candidate_uid,
      marker.index,
      marker.direction,
      marker.level2_score,
      marker.status,
    ].join(':'))
    .join('|');

const buildWatchingTrendMarkers = (candles = [], rows = []) =>
  rows
    .map((row) => {
      const details = row.details ?? {};
      const index = findCandleIndexByTimes(candles, [
        details.signal_date,
        row.ts_utc,
        row.candle_time,
      ]);

      if (!index) {
        return null;
      }

      return {
        event_uid: row.event_uid ?? null,
        candidate_uid: getScannerCandidateUid(row),
        index,
        direction: row.direction ?? null,
        level2_score: row.level2_score ?? null,
        stage2_score: row.stage2_score ?? null,
        status: row.status ?? 'watching_stage2',
        candle_time: row.candle_time ?? row.ts_utc ?? null,
      };
    })
    .filter(Boolean);

const mergeLiveCanvasCandles = (...candleSets) => {
  const merged = new Map();

  candleSets.flat().forEach((candle) => {
    const key = getLiveCanvasCandleKey(candle);
    if (key && key !== '|') {
      merged.set(key, candle);
    }
  });

  return [...merged.values()].sort((left, right) => {
    const leftTime = new Date(left.candle_date).getTime();
    const rightTime = new Date(right.candle_date).getTime();
    if (leftTime !== rightTime) {
      return rightTime - leftTime;
    }
    return String(right.symbol ?? '').localeCompare(String(left.symbol ?? ''));
  });
};

const getDateTimeForCompare = (value) => {
  const formattedValue = formatDateTimeForServer(value);
  if (!formattedValue) {
    return null;
  }

  const parsed = new Date(formattedValue.replace(' ', 'T'));
  return Number.isNaN(parsed.getTime()) ? null : parsed.getTime();
};

const clampServerDateText = (value, minValue = null, maxValue = null) => {
  const formatted = formatDateTimeForServer(value);
  if (!formatted) {
    return null;
  }

  const valueTime = getDateTimeForCompare(formatted);
  const minTime = minValue ? getDateTimeForCompare(minValue) : null;
  const maxTime = maxValue ? getDateTimeForCompare(maxValue) : null;

  if (minValue && valueTime !== null && minTime !== null && valueTime < minTime) {
    return minValue;
  }

  if (maxValue && valueTime !== null && maxTime !== null && valueTime > maxTime) {
    return maxValue;
  }

  return formatted;
};

const getTrendEntryDate = (trend = {}) =>
  trend.paper_entry_ts_utc ??
  trend.paper_entry_candle_time ??
  trend.ts_utc ??
  trend.candle_time ??
  null;

const getTrendExitDate = (trend = {}) =>
  trend.paper_exit_ts_utc ??
  trend.paper_exit_candle_time ??
  trend.target_date ??
  trend.exit_date ??
  null;

const buildLiveCanvasTrendCandleWindow = (trend = {}) => {
  const entryDate = getTrendEntryDate(trend);
  const exitDate = getTrendExitDate(trend) ?? entryDate;

  if (!entryDate && !exitDate) {
    return {
      startDate: LIVE_CANVAS_START_DATE,
      endDate: LIVE_CANVAS_END_DATE,
      limit: LIVE_CANVAS_INITIAL_CANDLE_PAGE_LIMIT,
    };
  }

  const startDate = clampServerDateText(
    addTimeframeBars(entryDate ?? exitDate, LIVE_CANVAS_TIMEFRAME, -LIVE_CANVAS_TREND_CONTEXT_PRE_BARS),
    LIVE_CANVAS_START_DATE,
    LIVE_CANVAS_END_DATE
  );
  const endDate = clampServerDateText(
    addTimeframeBars(exitDate ?? entryDate, LIVE_CANVAS_TIMEFRAME, LIVE_CANVAS_TREND_CONTEXT_POST_BARS),
    LIVE_CANVAS_START_DATE,
    LIVE_CANVAS_END_DATE
  );
  const startMs = getDateTimeForCompare(startDate);
  const endMs = getDateTimeForCompare(endDate);
  const estimatedBars =
    startMs !== null && endMs !== null
      ? Math.max(1, Math.ceil((endMs - startMs) / getTimeframeMs(LIVE_CANVAS_TIMEFRAME)))
      : LIVE_CANVAS_TREND_WINDOW_MIN_LIMIT;
  const limit = Math.min(
    LIVE_CANVAS_TREND_WINDOW_MAX_LIMIT,
    Math.max(LIVE_CANVAS_TREND_WINDOW_MIN_LIMIT, estimatedBars * 3 + 240)
  );

  return { startDate, endDate, limit };
};

const clipCandlesAfterCanvasEnd = (candles = [], pattern = {}) => {
  const exitTime = getDateTimeForCompare(
    pattern.canvas_end_date ?? pattern.xa_outcome_hit_date ?? pattern.target_date ?? pattern.trade_date
  );
  if (exitTime === null) {
    return candles;
  }

  const candleTimes = candles
    .map((candle) => getDateTimeForCompare(candle.candle_date ?? candle.date))
    .filter((value) => value !== null);
  const hasExactExitCandle = candleTimes.some((value) => value === exitTime);
  const nextAfterExitTime = hasExactExitCandle
    ? null
    : candleTimes.reduce(
        (nextTime, candleTime) =>
          candleTime > exitTime && candleTime < nextTime ? candleTime : nextTime,
        Number.POSITIVE_INFINITY
      );

  return candles.filter((candle) => {
    const candleTime = getDateTimeForCompare(candle.candle_date ?? candle.date);
    return (
      candleTime === null ||
      candleTime <= exitTime ||
      (!hasExactExitCandle && candleTime === nextAfterExitTime)
    );
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
    source_table: trade.source_table ?? pattern.source_table,
    source_timeframe: trade.source_timeframe ?? pattern.source_timeframe,
    entry_date: trade.entry_date ?? pattern.entry_date,
    target_date: trade.target_date ?? pattern.target_date,
    canvas_end_date: trade.canvas_end_date ?? pattern.canvas_end_date,
    xa_canvas_mode: trade.xa_canvas_mode ?? pattern.xa_canvas_mode,
    xa_start_price: trade.xa_start_price ?? pattern.xa_start_price,
    xa_reversal_limit_price: trade.xa_reversal_limit_price ?? pattern.xa_reversal_limit_price,
    xa_continuation_limit_price: trade.xa_continuation_limit_price ?? pattern.xa_continuation_limit_price,
    xa_outcome_hit_date: trade.xa_outcome_hit_date ?? pattern.xa_outcome_hit_date,
    xa_outcome: trade.xa_outcome ?? pattern.xa_outcome,
    xa_outcome_price: trade.xa_outcome_price ?? pattern.xa_outcome_price,
    trade_enter_price: trade.trade_enter_price ?? pattern.trade_enter_price,
    trade_risk_exit_price: trade.trade_risk_exit_price ?? pattern.trade_risk_exit_price,
    trade_reward_exit_price: trade.trade_reward_exit_price ?? pattern.trade_reward_exit_price,
    trade_current_price: exitPrice ?? pattern.trade_current_price,
    target_close: exitPrice ?? pattern.target_close,
    trade_result: trade.trade_result ?? pattern.trade_result,
    trade_direction: trade.trade_direction ?? pattern.trade_direction,
    outcome: trade.outcome ?? pattern.outcome,
    exit_reason: trade.exit_reason ?? pattern.exit_reason,
    result_r: trade.result_r ?? pattern.result_r,
    risk_points: trade.risk_points ?? pattern.risk_points,
  };
};

const copyTextToClipboard = async (value) => {
  const text = String(value ?? '').trim();
  if (!text) {
    return false;
  }

  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return true;
  }

  const textarea = document.createElement('textarea');
  textarea.value = text;
  textarea.setAttribute('readonly', '');
  textarea.style.position = 'fixed';
  textarea.style.opacity = '0';
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand('copy');
  document.body.removeChild(textarea);
  return copied;
};

const SelectedSummaryRow = ({
  actionDisabled = false,
  actionLabel,
  className = '',
  emptyText,
  index,
  isLoading = false,
  label,
  loadingText,
  metrics = [],
  onAction,
  onMouseEnter,
  onMouseLeave,
  status,
  tone = 'neutral',
}) => {
  return (
    <section
      className={[
        'pattern-family-selected-card',
        `pattern-family-selected-card--${tone}`,
        className,
      ].filter(Boolean).join(' ')}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
    >
      <header className="pattern-family-selected-head">
        <span className="pattern-family-selected-index">{String(index).padStart(2, '0')}</span>
        <div className="pattern-family-selected-head-copy">
          <strong>{label}</strong>
        </div>
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
};

const PatternFamilyUniversePage = ({ initialFamilyKey = null, entryExitOnly = false } = {}) => {
  const isEntryExitStandalone = Boolean(entryExitOnly);
  const [families, setFamilies] = useState([]);
  const [yearOptions, setYearOptions] = useState([]);
  const [isLoading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [search, setSearch] = useState('');
  const [harmonicType, setHarmonicType] = useState('All');
  const [sourceScope, setSourceScope] = useState('futures');
  const [timeframeFilter, setTimeframeFilter] = useState('All');
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
  const [patternBrowseScope, setPatternBrowseScope] = useState('selected');
  const [patternBrowseSymbol, setPatternBrowseSymbol] = useState('All');
  const [patternBrowseSearch, setPatternBrowseSearch] = useState('');
  const [patternBrowseRows, setPatternBrowseRows] = useState([]);
  const [patternBrowseMeta, setPatternBrowseMeta] = useState({ totalCount: 0, hasMore: false });
  const [isPatternBrowseLoading, setPatternBrowseLoading] = useState(false);
  const [patternBrowseError, setPatternBrowseError] = useState('');
  const [patternXaOutcomeData, setPatternXaOutcomeData] = useState({
    run: null,
    totalRows: 0,
    limit: 300,
    offset: 0,
    familyRows: [],
    rows: [],
  });
  const [isPatternXaOutcomeLoading, setPatternXaOutcomeLoading] = useState(false);
  const [patternXaOutcomeError, setPatternXaOutcomeError] = useState('');
  const [patternReversalAiData, setPatternReversalAiData] = useState({
    run: null,
    totalRows: 0,
    limit: 300,
    offset: 0,
    thresholds: [],
    buckets: [],
    rows: [],
  });
  const [isPatternReversalAiLoading, setPatternReversalAiLoading] = useState(false);
  const [patternReversalAiError, setPatternReversalAiError] = useState('');
  const [patternAiStage1TradeData, setPatternAiStage1TradeData] = useState({
    run: null,
    summary: null,
    templatePerformance: [],
    daily: [],
    hourly: [],
    tradeCadence: null,
    tradeWorkload: null,
    symbolContribution: [],
    familyContribution: [],
    lossWindows: [],
    totalRows: 0,
    limit: PATTERN_AI_STAGE1_TRADE_PAGE_SIZE,
    offset: 0,
    rows: [],
  });
  const [isPatternAiStage1TradeLoading, setPatternAiStage1TradeLoading] = useState(false);
  const [patternAiStage1TradeError, setPatternAiStage1TradeError] = useState('');
  const [patternAiExitModelTradeData, setPatternAiExitModelTradeData] = useState({
    run: null,
    summary: null,
    totalRows: 0,
    limit: PATTERN_AI_EXIT_TRADE_PAGE_SIZE,
    offset: 0,
    rows: [],
  });
  const [isPatternAiExitModelTradeLoading, setPatternAiExitModelTradeLoading] = useState(false);
  const [patternAiExitModelTradeError, setPatternAiExitModelTradeError] = useState('');
  const [patternAiExitModelTakenTradeData, setPatternAiExitModelTakenTradeData] = useState({
    run: null,
    summary: null,
    totalRows: 0,
    limit: PATTERN_AI_EXIT_TAKEN_TRADE_PAGE_SIZE,
    offset: 0,
    rows: [],
  });
  const [isPatternAiExitModelTakenTradeLoading, setPatternAiExitModelTakenTradeLoading] = useState(false);
  const [patternAiExitModelTakenTradeError, setPatternAiExitModelTakenTradeError] = useState('');
  const [dataCenterCollapsedSections, setDataCenterCollapsedSections] = useState({});
  const [selectedPatternXaOutcomeRowKey, setSelectedPatternXaOutcomeRowKey] = useState('');
  const patternXaOutcomeTableWrapRef = useRef(null);
  const patternXaOutcomeRowRefs = useRef(new Map());
  const [appliedPatternNavigation, setAppliedPatternNavigation] = useState(null);
  const [routeTrades, setRouteTrades] = useState([]);
  const [isRouteTradesLoading, setRouteTradesLoading] = useState(false);
  const [routeTradesError, setRouteTradesError] = useState('');
  const [routeFamilyRows, setRouteFamilyRows] = useState([]);
  const [isRouteFamiliesLoading, setRouteFamiliesLoading] = useState(false);
  const [routeFamiliesError, setRouteFamiliesError] = useState('');
  const [supplyData, setSupplyData] = useState({ symbols: [], families: [] });
  const [isSupplyLoading, setSupplyLoading] = useState(false);
  const [supplyError, setSupplyError] = useState('');
  const [patternStorageData, setPatternStorageData] = useState(null);
  const [isPatternStorageLoading, setPatternStorageLoading] = useState(false);
  const [patternStorageError, setPatternStorageError] = useState('');
  const [patternCatalogSource, setPatternCatalogSource] = useState('All');
  const [patternCatalogTimeframe, setPatternCatalogTimeframe] = useState('All');
  const [patternCatalogFit, setPatternCatalogFit] = useState('All');
  const [selectedPatternCatalogProfileId, setSelectedPatternCatalogProfileId] = useState(null);
  const [entryExitData, setEntryExitData] = useState({ run: null, build_summary: null, templates: [], coverage: [] });
  const [entryExitBuildSummaries, setEntryExitBuildSummaries] = useState([]);
  const [isEntryExitBuildListLoading, setEntryExitBuildListLoading] = useState(false);
  const [entryExitBuildListError, setEntryExitBuildListError] = useState('');
  const [isEntryExitLoading, setEntryExitLoading] = useState(false);
  const [entryExitError, setEntryExitError] = useState('');
  const [selectedBuildView, setSelectedBuildView] = useState('dashboard');
  const [entryExitRouterData, setEntryExitRouterData] = useState({
    current_run: null,
    runs: [],
    symbols: [],
    family_routes: [],
    template_performance: [],
    manual_family_bans: [],
    manual_symbol_bans: [],
  });
  const [entryExitRouterRefreshKey] = useState(0);
  const [isEntryExitRouterLoading, setEntryExitRouterLoading] = useState(false);
  const [entryExitRouterError, setEntryExitRouterError] = useState('');
  const [entryExitSimEquityCurve, setEntryExitSimEquityCurve] = useState({ sim_run_id: '', points: [] });
  const [isEntryExitSimEquityLoading, setEntryExitSimEquityLoading] = useState(false);
  const [entryExitSimEquityError, setEntryExitSimEquityError] = useState('');
  const [entryExitSimDailyRData, setEntryExitSimDailyRData] = useState({ sim_run_id: '', days: [] });
  const [isEntryExitSimDailyRLoading, setEntryExitSimDailyRLoading] = useState(false);
  const [entryExitSimDailyRError, setEntryExitSimDailyRError] = useState('');
  const [selectedSimulationDailyDate, setSelectedSimulationDailyDate] = useState('');
  const [entryExitSimDailyTradesData, setEntryExitSimDailyTradesData] = useState({
    sim_run_id: '',
    trade_date: '',
    trades: [],
  });
  const [isEntryExitSimDailyTradesLoading, setEntryExitSimDailyTradesLoading] = useState(false);
  const [entryExitSimDailyTradesError, setEntryExitSimDailyTradesError] = useState('');
  const [entryExitSimRawTradesData, setEntryExitSimRawTradesData] = useState({
    sim_run_id: '',
    total_rows: 0,
    limit: 500,
    offset: 0,
    trades: [],
  });
  const [isEntryExitSimRawTradesLoading, setEntryExitSimRawTradesLoading] = useState(false);
  const [entryExitSimRawTradesError, setEntryExitSimRawTradesError] = useState('');
  const [entryExitSimHourlyData, setEntryExitSimHourlyData] = useState({ sim_run_id: '', hours: [] });
  const [isEntryExitSimHourlyLoading, setEntryExitSimHourlyLoading] = useState(false);
  const [entryExitSimHourlyError, setEntryExitSimHourlyError] = useState('');
  const [entryExitSimTradeCadenceData, setEntryExitSimTradeCadenceData] = useState({
    sim_run_id: '',
    cadence: null,
  });
  const [isEntryExitSimTradeCadenceLoading, setEntryExitSimTradeCadenceLoading] = useState(false);
  const [entryExitSimTradeCadenceError, setEntryExitSimTradeCadenceError] = useState('');
  const [entryExitSimTradeGapData, setEntryExitSimTradeGapData] = useState({
    sim_run_id: '',
    gaps: [],
  });
  const [isEntryExitSimTradeGapLoading, setEntryExitSimTradeGapLoading] = useState(false);
  const [entryExitSimTradeGapError, setEntryExitSimTradeGapError] = useState('');
  const [entryExitSimTradeWorkloadData, setEntryExitSimTradeWorkloadData] = useState({
    sim_run_id: '',
    workload: null,
  });
  const [isEntryExitSimTradeWorkloadLoading, setEntryExitSimTradeWorkloadLoading] = useState(false);
  const [entryExitSimTradeWorkloadError, setEntryExitSimTradeWorkloadError] = useState('');
  const [entryExitSimTestFrequencyData, setEntryExitSimTestFrequencyData] = useState({
    sim_run_id: '',
    tests: [],
  });
  const [isEntryExitSimTestFrequencyLoading, setEntryExitSimTestFrequencyLoading] = useState(false);
  const [entryExitSimTestFrequencyError, setEntryExitSimTestFrequencyError] = useState('');
  const [entryExitDayTradingSimData, setEntryExitDayTradingSimData] = useState({
    sim_run_id: '',
    summary: null,
    daily: [],
    monthly: [],
  });
  const [isEntryExitDayTradingSimLoading, setEntryExitDayTradingSimLoading] = useState(false);
  const [entryExitDayTradingSimError, setEntryExitDayTradingSimError] = useState('');
  const [entryExitSimMarketTrendData, setEntryExitSimMarketTrendData] = useState({
    sim_run_id: '',
    performance: [],
    alignment: [],
    direction_alignment: [],
  });
  const [isEntryExitSimMarketTrendLoading, setEntryExitSimMarketTrendLoading] = useState(false);
  const [entryExitSimMarketTrendError, setEntryExitSimMarketTrendError] = useState('');
  const [entryExitSimSymbolContributionData, setEntryExitSimSymbolContributionData] = useState({
    sim_run_id: '',
    symbols: [],
  });
  const [isEntryExitSimSymbolContributionLoading, setEntryExitSimSymbolContributionLoading] = useState(false);
  const [entryExitSimSymbolContributionError, setEntryExitSimSymbolContributionError] = useState('');
  const [entryExitSimFamilyContributionData, setEntryExitSimFamilyContributionData] = useState({
    sim_run_id: '',
    families: [],
  });
  const [isEntryExitSimFamilyContributionLoading, setEntryExitSimFamilyContributionLoading] = useState(false);
  const [entryExitSimFamilyContributionError, setEntryExitSimFamilyContributionError] = useState('');
  const [entryExitSimStreakData, setEntryExitSimStreakData] = useState({ sim_run_id: '', streaks: [] });
  const [isEntryExitSimStreakLoading, setEntryExitSimStreakLoading] = useState(false);
  const [entryExitSimStreakError, setEntryExitSimStreakError] = useState('');
  const [entryExitSimLossClusterData, setEntryExitSimLossClusterData] = useState({
    sim_run_id: '',
    summary: null,
    buckets: [],
    windows: [],
  });
  const [isEntryExitSimLossClusterLoading, setEntryExitSimLossClusterLoading] = useState(false);
  const [entryExitSimLossClusterError, setEntryExitSimLossClusterError] = useState('');
  const [entryExitSimulationTab, setEntryExitSimulationTab] = useState('overview');
  const [entryExitSimulationMode, setEntryExitSimulationMode] = useState('propFirm');
  const [selectedEntryExitSimulationYearKey, setSelectedEntryExitSimulationYearKey] = useState('');
  const [entryExitTemplateBreakdown, setEntryExitTemplateBreakdown] = useState({
    market: [],
    harmonic_type: [],
    conditions: [],
    family_results: [],
    combos: [],
  });
  const [isEntryExitTemplateBreakdownLoading, setEntryExitTemplateBreakdownLoading] = useState(false);
  const [entryExitTemplateBreakdownError, setEntryExitTemplateBreakdownError] = useState('');
  const [selectedEntryExitRouterRunId, setSelectedEntryExitRouterRunId] = useState(null);
  const [selectedRouteTradeKey, setSelectedRouteTradeKey] = useState(null);
  const [selectedSimulationRawTradeKey, setSelectedSimulationRawTradeKey] = useState(null);
  const [selectedSimulationRawTrade, setSelectedSimulationRawTrade] = useState(null);
  const [selectedPatternRouteTrade, setSelectedPatternRouteTrade] = useState(null);
  const [isPatternRouteTradeLoading, setPatternRouteTradeLoading] = useState(false);
  const [patternRouteTradeError, setPatternRouteTradeError] = useState('');
  const [canvasChartData, setCanvasChartData] = useState({ candles: [], rust_patterns: null });
  const [canvasPattern, setCanvasPattern] = useState(null);
  const [inspectorHoveredCandle, setInspectorHoveredCandle] = useState(null);
  const [isCanvasLoading, setCanvasLoading] = useState(false);
  const [canvasError, setCanvasError] = useState('');
  const [liveCanvasRefreshMeta, setLiveCanvasRefreshMeta] = useState({
    checkedAt: null,
    snapshotCheckedAt: null,
    latestSnapshotAt: null,
    latestSnapshotPrice: null,
    latestCandleAt: null,
    candleCount: 0,
    isRefreshing: false,
    changed: false,
    error: '',
  });
  const liveCanvasCandlePageMetaRef = useRef({
    isLoadingOlder: false,
    hasOlder: true,
    oldestCandleAt: null,
    newestCandleAt: null,
    loadedCount: 0,
  });
  const updateLiveCanvasCandlePageMetaRef = useRef(null);
  updateLiveCanvasCandlePageMetaRef.current = (nextValueOrUpdater) => {
    liveCanvasCandlePageMetaRef.current =
      typeof nextValueOrUpdater === 'function'
        ? nextValueOrUpdater(liveCanvasCandlePageMetaRef.current)
        : nextValueOrUpdater;
  };
  const [liveTrendEvents, setLiveTrendEvents] = useState({
    runId: null,
    totalRows: 0,
    checkedAt: null,
    isLoading: false,
    error: '',
    rows: [],
  });
  const [liveOracleTrendEvents, setLiveOracleTrendEvents] = useState({
    runId: null,
    totalRows: 0,
    checkedAt: null,
    isLoading: false,
    error: '',
    rows: [],
  });
  const [liveTradeSignals, setLiveTradeSignals] = useState({
    runId: LIVE_CANVAS_SIGNAL_RUN_ID,
    totalRows: 0,
    summary: null,
    checkedAt: null,
    isLoading: false,
    error: '',
    rows: [],
  });
  const [liveScannerActivity, setLiveScannerActivity] = useState({
    runId: LIVE_CANVAS_TREND_RUN_ID,
    totalRows: 0,
    checkedAt: null,
    isLoading: false,
    error: '',
    rows: [],
  });
  const [liveCanvasSymbol, setLiveCanvasSymbol] = useState(LIVE_CANVAS_SYMBOL);
  const [liveTrendPanelMode, setLiveTrendPanelMode] = useState('trades');
  const [showSkippedLiveTrends, setShowSkippedLiveTrends] = useState(false);
  const [selectedLiveTrendKey, setSelectedLiveTrendKey] = useState('');
  const selectedLiveTrendRef = useRef(null);
  const liveTrendRefreshInFlightRef = useRef(false);
  const liveOracleTrendRefreshInFlightRef = useRef(false);
  const liveCanvasSignatureRef = useRef('');
  const liveCanvasSnapshotSignatureRef = useRef('');
  const liveCanvasCandlesRef = useRef([]);
  const liveCanvasSymbolRef = useRef(LIVE_CANVAS_SYMBOL);
  const liveCanvasRefreshInFlightRef = useRef(false);
  const liveCanvasOlderCandlesInFlightRef = useRef(false);
  const liveCanvasLoadedAllOlderRef = useRef(false);
  const selectedFamilyKeyRef = useRef(selectedFamilyKey);

  useEffect(() => {
    if (!CANVAS_ONLY_MODE || typeof window === 'undefined') {
      return undefined;
    }

    const setLiveMobileViewportHeight = () => {
      const height = window.visualViewport?.height || window.innerHeight;
      if (Number.isFinite(height) && height > 0) {
        document.documentElement.style.setProperty('--live-mobile-vh', `${height}px`);
      }
    };

    setLiveMobileViewportHeight();
    window.addEventListener('resize', setLiveMobileViewportHeight);
    window.visualViewport?.addEventListener('resize', setLiveMobileViewportHeight);
    window.visualViewport?.addEventListener('scroll', setLiveMobileViewportHeight);

    return () => {
      window.removeEventListener('resize', setLiveMobileViewportHeight);
      window.visualViewport?.removeEventListener('resize', setLiveMobileViewportHeight);
      window.visualViewport?.removeEventListener('scroll', setLiveMobileViewportHeight);
      document.documentElement.style.removeProperty('--live-mobile-vh');
    };
  }, []);

  useEffect(() => {
    if (CANVAS_ONLY_MODE && liveTrendPanelMode !== 'trades') {
      setLiveTrendPanelMode('trades');
    }
  }, [liveTrendPanelMode]);

  const visibleLiveTrendRows = useMemo(
    () => {
      const rows = Array.isArray(liveTrendEvents.rows) ? liveTrendEvents.rows : [];
      if (showSkippedLiveTrends) {
        return rows;
      }
      return rows.filter((trend) => String(trend?.paper_entry_status || '').toLowerCase() === 'accepted');
    },
    [liveTrendEvents.rows, showSkippedLiveTrends]
  );
  const hiddenSkippedLiveTrendCount = useMemo(
    () => {
      const rows = Array.isArray(liveTrendEvents.rows) ? liveTrendEvents.rows : [];
      return rows.filter((trend) => String(trend?.paper_entry_status || '').toLowerCase() !== 'accepted').length;
    },
    [liveTrendEvents.rows]
  );
  const visibleLiveOracleTrendRows = useMemo(
    () => (Array.isArray(liveOracleTrendEvents.rows) ? liveOracleTrendEvents.rows : []),
    [liveOracleTrendEvents.rows]
  );
  const liveWatchingTrendRows = useMemo(
    () => {
      const rows = Array.isArray(liveScannerActivity.rows) ? liveScannerActivity.rows : [];
      const activeByUid = new Map();

      rows.forEach((row) => {
        const uid = getScannerCandidateUid(row);
        if (!uid) {
          return;
        }

        const eventType = String(row.event_type || '').toLowerCase();
        if (eventType === 'level2_pick') {
          activeByUid.set(uid, row);
          return;
        }

        if (eventType === 'trend_confirmed' || eventType === 'stage2_expired' || eventType === 'order_signal') {
          activeByUid.delete(uid);
        }
      });

      return Array.from(activeByUid.values());
    },
    [liveScannerActivity.rows]
  );
  const [isCanvasExpanded, setCanvasExpanded] = useState(false);
  const [showCanvasCandles, setShowCanvasCandles] = useState(true);
  const [inspectorDetailMode, setInspectorDetailMode] = useState('pattern');
  const [copiedTradeId, setCopiedTradeId] = useState('');
  const [copiedPatternId, setCopiedPatternId] = useState('');
  const [routeLogicHover, setRouteLogicHover] = useState(null);
  const [isRouteLogicCollapsed, setRouteLogicCollapsed] = useState(false);
  const simAccountSize = '50K';
  const simContracts = '1';
  const simTestsToChain = '1';
  const simDrawdownModel = 'intraday';
  const simOneTradeAtATime = false;
  const [browsePanel, setBrowsePanel] = useState(null);
  const [testOverviewTab, setTestOverviewTab] = useState('patterns');
  const [selectedDataCollapseLevel, setSelectedDataCollapseLevel] = useState(isEntryExitStandalone ? 2 : 0);
  const [isInspectorCollapsed, setInspectorCollapsed] = useState(isEntryExitStandalone);
  const [isPatternCardHovered, setPatternCardHovered] = useState(false);
  const [selectedEntryExitTemplateUid, setSelectedEntryExitTemplateUid] = useState(null);
  const [entryExitProfileTab, setEntryExitProfileTab] = useState('all');
  const [selectedPlaybookView, setSelectedPlaybookView] = useState('dashboard');
  const [selectedBuildCoverageExchangeKey, setSelectedBuildCoverageExchangeKey] = useState('');
  const [selectedEntryExitModelDatasetId, setSelectedEntryExitModelDatasetId] = useState(null);
  const [expandedSimulationTable, setExpandedSimulationTable] = useState(null);
  const isSelectedDataCollapsed = selectedDataCollapseLevel >= 1;
  const isSelectedDataFullyCollapsed = selectedDataCollapseLevel >= 2;
  const selectionDeckToggleLabel =
    selectedDataCollapseLevel === 0
      ? 'Show compact Selection Deck'
      : selectedDataCollapseLevel === 1
        ? 'Show Selection Deck header only'
        : 'Expand Selection Deck';

  useEffect(() => {
    selectedFamilyKeyRef.current = selectedFamilyKey;
  }, [selectedFamilyKey]);

  const entryExitModelDatasets = useMemo(
    () =>
      entryExitBuildSummaries.map((summary, index) => ({
        id: summary.run_id,
        buildLabel: summary.build_label || `B${index + 1}`,
        label: summary.scan_year_label || compactText(summary.run_id, 20),
        detail: `${formatNumber(summary.templates_created)} tests | ${formatNumber(
          summary.patterns_scanned
        )} patterns`,
        summary,
      })),
    [entryExitBuildSummaries]
  );
  const selectedEntryExitModelDataset =
    entryExitModelDatasets.find((dataset) => dataset.id === selectedEntryExitModelDatasetId) ??
    entryExitModelDatasets[0] ??
    null;
  const selectedBuildRunId = selectedEntryExitModelDataset?.id ?? null;
  const selectedBuildLabel = selectedEntryExitModelDataset?.buildLabel ?? '';
  const selectedBuildRunIsLoaded =
    Boolean(selectedBuildRunId) && entryExitData.run?.run_id === selectedBuildRunId;
  const selectedBuildSummary =
    selectedBuildRunIsLoaded && entryExitData.build_summary?.run_id === selectedBuildRunId
      ? entryExitData.build_summary
      : selectedEntryExitModelDataset?.summary ?? null;
  const selectedBuildHasStoredSummary = Boolean(selectedBuildSummary);
  const selectedBuildYearLabel = selectedBuildSummary?.scan_year_label ?? '';
  const selectedBuildPatternsScanned = selectedBuildSummary
    ? formatNumber(selectedBuildSummary?.patterns_scanned)
    : isEntryExitLoading
      ? 'Loading'
      : '';
  const selectedBuildTestsBuilt = selectedBuildSummary
    ? formatNumber(selectedBuildSummary?.templates_created)
    : isEntryExitLoading
      ? 'Loading'
      : '';
  const selectedBuildSourceLabel =
    selectedBuildSummary?.source_scope
      ? formatRouteMode(selectedBuildSummary.source_scope)
      : '';
  const selectedBuildTimeframeLabel =
    selectedBuildSummary?.source_timeframe
      ? selectedBuildSummary.source_timeframe
      : '';
  useEffect(() => {
    if (isEntryExitStandalone) {
      setTestOverviewTab('patterns');
      setSelectedDataCollapseLevel(2);
      setInspectorCollapsed(true);
      setEntryExitProfileTab('model');
    }
  }, [isEntryExitStandalone]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitBuilds = async () => {
      try {
        setEntryExitBuildListLoading(true);
        setEntryExitBuildListError('');
        const builds = await fetchEntryExitBuilds({ limit: 25 });

        if (!isCancelled) {
          setEntryExitBuildSummaries(builds);
          setSelectedEntryExitModelDatasetId((currentBuildId) =>
            builds.some((build) => build.run_id === currentBuildId)
              ? currentBuildId
              : builds[0]?.run_id ?? null
          );
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitBuildListError('Could not load stored Entry / Exit builds.');
          setEntryExitBuildSummaries([]);
          setSelectedEntryExitModelDatasetId(null);
        }
      } finally {
        if (!isCancelled) {
          setEntryExitBuildListLoading(false);
        }
      }
    };

    void loadEntryExitBuilds();

    return () => {
      isCancelled = true;
    };
  }, []);

  useEffect(() => {
    if (inspectorDetailMode !== 'logic') {
      setRouteLogicHover(null);
    }
  }, [inspectorDetailMode]);

  useEffect(() => {
    if (ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) {
      setPatternStorageLoading(false);
      setPatternStorageError('');
      return undefined;
    }

    const shouldLoadPatternStorage =
      testOverviewTab === 'patterns' || testOverviewTab === 'entryExit' || isEntryExitStandalone;
    if (!shouldLoadPatternStorage || patternStorageData) {
      return undefined;
    }

    let isCancelled = false;

    const loadPatternStorage = async () => {
      try {
        setPatternStorageLoading(true);
        setPatternStorageError('');
        const result = await fetchCandleStorageSummary({ includeStockSymbols: false });
        if (!isCancelled) {
          setPatternStorageData(result);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setPatternStorageError('Could not load pattern root symbols.');
        }
      } finally {
        if (!isCancelled) {
          setPatternStorageLoading(false);
        }
      }
    };

    void loadPatternStorage();

    return () => {
      isCancelled = true;
    };
  }, [isEntryExitStandalone, patternStorageData, testOverviewTab]);

  useEffect(() => {
    if (ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) {
      setFamilies([]);
      setLoading(false);
      setError('');
      return undefined;
    }

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
          sourceTimeframe: timeframeFilter === 'All' ? null : timeframeFilter,
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
  }, [isEntryExitStandalone, sourceScope, timeframeFilter, yearFilter]);

  const harmonicOptions = useMemo(
    () => ['All', ...uniqueValues(families, 'harmonic_type')],
    [families]
  );

  const patternCatalogProfiles = useMemo(() => {
    const profileMap = (patternStorageData?.setup_contracts ?? []).reduce((map, row) => {
      const source = getPatternCatalogSource(row);
      const timeframe = row.source_timeframe || 'unknown';
      const key = getPatternScanProfileKey({
        source,
        timeframe,
        fitKey: DEFAULT_PATTERN_SCAN_FIT.key,
      });
      const current = map.get(key) ?? {
        id: key,
        source,
        sourceKey: getPatternCatalogSourceKey(source),
        timeframe,
        fitKey: DEFAULT_PATTERN_SCAN_FIT.key,
        fitLabel: DEFAULT_PATTERN_SCAN_FIT.label,
        fitDetail: DEFAULT_PATTERN_SCAN_FIT.detail,
        setup_count: 0,
        rootSymbols: new Set(),
        contractSymbols: new Set(),
        exchangeNames: new Set(),
        sourceTables: new Set(),
        first_d_date: null,
        last_d_date: null,
      };
      const rootSymbol = normalizeFuturesRootSymbol(row.root_symbol || row.contract_symbol || 'Unknown');
      current.setup_count += Number(row.setup_count || 0);
      if (rootSymbol) {
        current.rootSymbols.add(rootSymbol);
        current.exchangeNames.add(getFuturesExchange(rootSymbol) || (source === 'Stocks' ? 'Stocks' : 'Unknown'));
      }
      if (row.contract_symbol) {
        current.contractSymbols.add(row.contract_symbol);
      }
      if (row.table_name) {
        current.sourceTables.add(row.table_name);
      }
      current.first_d_date = minDateValue(current.first_d_date, row.first_d_date);
      current.last_d_date = maxDateValue(current.last_d_date, row.last_d_date);
      map.set(key, current);
      return map;
    }, new Map());

    return [...profileMap.values()]
      .sort((left, right) => {
        const sourceOrder = { Futures: 0, Stocks: 1, Other: 2 };
        return (
          (sourceOrder[left.source] ?? 9) - (sourceOrder[right.source] ?? 9) ||
          getTimeframeSortIndex(left.timeframe) - getTimeframeSortIndex(right.timeframe) ||
          String(left.fitLabel).localeCompare(String(right.fitLabel))
        );
      })
      .map((profile, index) => ({
        ...profile,
        displayId: `S${index + 1}`,
        root_count: profile.rootSymbols.size,
        contract_count: profile.contractSymbols.size,
        exchange_count: profile.exchangeNames.size,
        source_tables: [...profile.sourceTables].sort(),
      }));
  }, [patternStorageData]);

  const patternCatalogSourceOptions = useMemo(() => {
    const preferred = ['Futures', 'Stocks'];
    const discovered = patternCatalogProfiles.map((profile) => profile.source);
    return ['All', ...[...new Set([...preferred, ...discovered])].filter((source) => source !== 'Other'), ...(discovered.includes('Other') ? ['Other'] : [])];
  }, [patternCatalogProfiles]);

  const patternCatalogTimeframeOptions = useMemo(() => {
    const discovered = [...new Set(patternCatalogProfiles.map((profile) => profile.timeframe).filter(Boolean))].sort(
      (left, right) => getTimeframeSortIndex(left) - getTimeframeSortIndex(right) || String(left).localeCompare(String(right))
    );
    return ['All', ...discovered];
  }, [patternCatalogProfiles]);

  const patternCatalogFitOptions = useMemo(() => {
    const discovered = [...new Map(patternCatalogProfiles.map((profile) => [profile.fitKey, profile.fitLabel])).entries()];
    return [{ key: 'All', label: 'All Fits' }, ...discovered.map(([key, label]) => ({ key, label }))];
  }, [patternCatalogProfiles]);

  const filteredPatternCatalogProfiles = useMemo(
    () =>
      patternCatalogProfiles.filter((profile) => {
        if (patternCatalogSource !== 'All' && profile.source !== patternCatalogSource) return false;
        if (patternCatalogTimeframe !== 'All' && profile.timeframe !== patternCatalogTimeframe) return false;
        if (patternCatalogFit !== 'All' && profile.fitKey !== patternCatalogFit) return false;
        return true;
      }),
    [patternCatalogFit, patternCatalogProfiles, patternCatalogSource, patternCatalogTimeframe]
  );

  const selectedPatternCatalogProfile = useMemo(
    () =>
      filteredPatternCatalogProfiles.find((profile) => profile.id === selectedPatternCatalogProfileId) ??
      filteredPatternCatalogProfiles[0] ??
      null,
    [filteredPatternCatalogProfiles, selectedPatternCatalogProfileId]
  );

  const selectedPatternCatalogRows = useMemo(() => {
    if (!selectedPatternCatalogProfile) {
      return [];
    }

    return (patternStorageData?.setup_contracts ?? []).filter(
      (row) =>
        getPatternCatalogSource(row) === selectedPatternCatalogProfile.source &&
        (row.source_timeframe || 'unknown') === selectedPatternCatalogProfile.timeframe
    );
  }, [patternStorageData, selectedPatternCatalogProfile]);

  const selectedPatternCatalogPatternRows = useMemo(() => {
    if (!selectedPatternCatalogProfile) {
      return [];
    }

    return (patternStorageData?.setup_patterns ?? []).filter(
      (row) =>
        getPatternCatalogSource(row) === selectedPatternCatalogProfile.source &&
        (row.source_timeframe || 'unknown') === selectedPatternCatalogProfile.timeframe
    );
  }, [patternStorageData, selectedPatternCatalogProfile]);

  const selectedPatternCatalogRootRows = useMemo(() => {
    const rootMap = selectedPatternCatalogRows.reduce((map, row) => {
      const rootSymbol = normalizeFuturesRootSymbol(row.root_symbol || row.contract_symbol || 'Unknown');
      const current = map.get(rootSymbol) ?? {
        root_symbol: rootSymbol,
        exchange: getFuturesExchange(rootSymbol) || (selectedPatternCatalogProfile?.source === 'Stocks' ? 'Stocks' : 'Unknown'),
        setup_count: 0,
        contractSymbols: new Set(),
        first_d_date: null,
        last_d_date: null,
      };
      current.setup_count += Number(row.setup_count || 0);
      if (row.contract_symbol) {
        current.contractSymbols.add(row.contract_symbol);
      }
      current.first_d_date = minDateValue(current.first_d_date, row.first_d_date);
      current.last_d_date = maxDateValue(current.last_d_date, row.last_d_date);
      map.set(rootSymbol, current);
      return map;
    }, new Map());

    return [...rootMap.values()]
      .map((row) => ({
        ...row,
        contract_count: row.contractSymbols.size,
      }))
      .sort((left, right) =>
        Number(right.setup_count || 0) - Number(left.setup_count || 0) ||
        String(left.root_symbol).localeCompare(String(right.root_symbol))
      );
  }, [selectedPatternCatalogProfile?.source, selectedPatternCatalogRows]);

  const selectedPatternCatalogExchangeSections = useMemo(() => {
    const exchangeOrder = ['CME', 'CBOT', 'NYMEX', 'COMEX', 'Stocks', 'Unknown'];
    const groups = selectedPatternCatalogRootRows.reduce((map, row) => {
      const exchange = row.exchange || 'Unknown';
      const current = map.get(exchange) ?? [];
      current.push(row);
      map.set(exchange, current);
      return map;
    }, new Map());

    return [...groups.entries()]
      .map(([exchange, rows]) => ({
        exchange,
        setupCount: rows.reduce((sum, row) => sum + Number(row.setup_count || 0), 0),
        rows,
      }))
      .sort((left, right) => {
        const leftIndex = exchangeOrder.indexOf(left.exchange);
        const rightIndex = exchangeOrder.indexOf(right.exchange);
        const normalizedLeftIndex = leftIndex === -1 ? exchangeOrder.length : leftIndex;
        const normalizedRightIndex = rightIndex === -1 ? exchangeOrder.length : rightIndex;
        return normalizedLeftIndex - normalizedRightIndex || left.exchange.localeCompare(right.exchange);
      });
  }, [selectedPatternCatalogRootRows]);

  const selectedPatternCatalogHarmonicRows = useMemo(() => {
    const harmonicMap = selectedPatternCatalogPatternRows.reduce((map, row) => {
      const harmonicType = row.harmonic_type || 'Unknown';
      const current = map.get(harmonicType) ?? { harmonic_type: harmonicType, setup_count: 0 };
      current.setup_count += Number(row.setup_count || 0);
      map.set(harmonicType, current);
      return map;
    }, new Map());
    return [...harmonicMap.values()].sort(
      (left, right) =>
        Number(right.setup_count || 0) - Number(left.setup_count || 0) ||
        String(left.harmonic_type).localeCompare(String(right.harmonic_type))
    );
  }, [selectedPatternCatalogPatternRows]);

  const selectedPatternCatalogMarketRows = useMemo(() => {
    const marketMap = selectedPatternCatalogPatternRows.reduce((map, row) => {
      const market = row.market || 'Unknown';
      const current = map.get(market) ?? { market, setup_count: 0 };
      current.setup_count += Number(row.setup_count || 0);
      map.set(market, current);
      return map;
    }, new Map());
    return [...marketMap.values()].sort(
      (left, right) =>
        Number(right.setup_count || 0) - Number(left.setup_count || 0) ||
        String(left.market).localeCompare(String(right.market))
    );
  }, [selectedPatternCatalogPatternRows]);

  const patternCatalogTotalPatterns = patternCatalogProfiles.reduce(
    (sum, profile) => sum + Number(profile.setup_count || 0),
    0
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

  const selectedFamily = useMemo(
    () => visibleFamilies.find((family) => family.family_key === selectedFamilyKey) ?? null,
    [selectedFamilyKey, visibleFamilies]
  );
  const selectedRoute = useMemo(
    () => phase1Results.find((result) => getRouteKey(result) === selectedRouteKey) ?? null,
    [phase1Results, selectedRouteKey]
  );
  const patternBrowseSymbolOptions = useMemo(() => {
    const symbols = uniqueValues(patternBrowseRows.length ? patternBrowseRows : familyPatterns, 'symbol');
    if (patternBrowseSymbol !== 'All' && !symbols.includes(patternBrowseSymbol)) {
      symbols.unshift(patternBrowseSymbol);
    }
    return ['All', ...symbols];
  }, [familyPatterns, patternBrowseRows, patternBrowseSymbol]);
  const visiblePatternBrowseRows = useMemo(() => {
    const query = patternBrowseSearch.trim().toLowerCase();
    if (!query) {
      return groupTwinPatternRows(patternBrowseRows);
    }

    return groupTwinPatternRows(patternBrowseRows.filter((pattern) => getPatternBrowseText(pattern).includes(query)));
  }, [patternBrowseRows, patternBrowseSearch]);
  const selectedFamilyPatternRows = useMemo(
    () =>
      groupTwinPatternRows(
        familyPatterns.filter((pattern) => {
          const patternFamilyKey = getPatternFamilyKey(pattern);
          return !selectedFamilyKey || !patternFamilyKey || patternFamilyKey === selectedFamilyKey;
        })
      ),
    [familyPatterns, selectedFamilyKey]
  );
  const patternBrowseFilterLabel = useMemo(() => {
    const scopeLabel =
      patternBrowseScope === 'all'
        ? 'All families'
        : `Family ${selectedFamily?.family_key ?? selectedFamilyKey ?? 'N/A'}`;
    const timeframeLabel = timeframeFilter === 'All' ? 'All timeframes' : timeframeFilter;
    const symbolLabel = patternBrowseSymbol === 'All' ? 'All symbols' : patternBrowseSymbol;
    const searchLabel = patternBrowseSearch.trim()
      ? `Search ${compactText(patternBrowseSearch.trim(), 18)}`
      : null;

    return [scopeLabel, timeframeLabel, symbolLabel, searchLabel].filter(Boolean).join(' | ');
  }, [
    patternBrowseScope,
    patternBrowseSearch,
    patternBrowseSymbol,
    selectedFamily?.family_key,
    selectedFamilyKey,
    timeframeFilter,
  ]);
  const canApplyPatternNavigation = !isPatternBrowseLoading && visiblePatternBrowseRows.length > 0;
  const patternXaOutcomeRun = patternXaOutcomeData.run;
  const patternXaOutcomeRows = patternXaOutcomeData.rows;
  const patternXaOutcomeFamilyRows = patternXaOutcomeData.familyRows;
  const patternReversalAiRun = patternReversalAiData.run;
  const patternReversalAiRows = patternReversalAiData.rows;
  const patternReversalAiThresholdRows = patternReversalAiData.thresholds;
  const patternReversalAiBucketRows = patternReversalAiData.buckets;
  const patternAiStage1TradeRows = useMemo(
    () => {
      const rawPatternAiStage1TradeRows = patternAiStage1TradeData.rows ?? [];
      return rawPatternAiStage1TradeRows.filter(
        (row) => row.multi_valid_eval_run_id === PATTERN_AI_STAGE1_DEFAULT_RUN_ID
      );
    },
    [patternAiStage1TradeData.rows]
  );
  const isPatternAiStage1LatestRunLoaded =
    patternAiStage1TradeData.run?.multi_valid_eval_run_id === PATTERN_AI_STAGE1_DEFAULT_RUN_ID;
  const patternAiExitModelTradeRows = patternAiExitModelTradeData.rows ?? [];
  const isPatternAiExitModelMainLoaded =
    patternAiExitModelTradeData.run?.exit_model_run_id === PATTERN_AI_EXIT_MAIN_RUN_ID;
  const patternAiExitModelTakenTradeRows = patternAiExitModelTakenTradeData.rows ?? [];
  const isPatternAiExitModelTakenMainLoaded =
    patternAiExitModelTakenTradeData.run?.exit_model_run_id === PATTERN_AI_EXIT_MAIN_RUN_ID;
  const patternAiExitModelTakenTradeHasMore =
    patternAiExitModelTakenTradeRows.length < Number(patternAiExitModelTakenTradeData.totalRows || 0);
  const patternAiExitModelOverviewItems = [
    {
      label: 'Exit Model',
      value: compactText(patternAiExitModelTradeData.run?.exit_model_run_id || PATTERN_AI_EXIT_MAIN_RUN_ID, 34),
      title: patternAiExitModelTradeData.run?.exit_model_run_id || PATTERN_AI_EXIT_MAIN_RUN_ID,
      wide: true,
    },
    {
      label: 'Source',
      value: compactText(patternAiExitModelTradeData.run?.source_model_run_id || 'N/A', 30),
      title: patternAiExitModelTradeData.run?.source_model_run_id || 'N/A',
      wide: true,
    },
    { label: 'Changed Trades', value: formatNumber(patternAiExitModelTradeData.summary?.changed_trades) },
    { label: 'Held Longer', value: formatNumber(patternAiExitModelTradeData.summary?.held_longer_trades), tone: 'win' },
    { label: 'Early Exits', value: formatNumber(patternAiExitModelTradeData.summary?.early_exit_trades) },
    {
      label: 'Source R',
      value: `${formatDecimal(patternAiExitModelTradeData.run?.baseline_sum_r, 1)}R`,
    },
    {
      label: 'AI Exit R',
      value: `${formatDecimal(patternAiExitModelTradeData.run?.sum_r, 1)}R`,
      tone: 'win',
    },
    {
      label: 'Delta',
      value: `${formatDecimal(patternAiExitModelTradeData.summary?.delta_sum_r, 1)}R`,
      tone: Number(patternAiExitModelTradeData.summary?.delta_sum_r || 0) >= 0 ? 'win' : 'loss',
    },
    {
      label: 'Source DD',
      value: `${formatDecimal(patternAiExitModelTradeData.run?.baseline_max_drawdown_r, 2)}R`,
    },
    {
      label: 'AI DD',
      value: `${formatDecimal(patternAiExitModelTradeData.run?.max_drawdown_r, 2)}R`,
    },
  ];
  const patternAiStage1TemplatePerformanceRows = patternAiStage1TradeData.templatePerformance ?? [];
  const patternAiStage1DailyRows = patternAiStage1TradeData.daily ?? [];
  const patternAiStage1HourlyRows = patternAiStage1TradeData.hourly ?? [];
  const patternAiStage1SymbolContributionRows = patternAiStage1TradeData.symbolContribution ?? [];
  const patternAiStage1FamilyContributionRows = patternAiStage1TradeData.familyContribution ?? [];
  const patternAiStage1LossWindowRows = patternAiStage1TradeData.lossWindows ?? [];
  const patternAiStage1TradeCadence = patternAiStage1TradeData.tradeCadence;
  const patternAiStage1TradeWorkload = patternAiStage1TradeData.tradeWorkload;
  const patternAiStage1TradeCadenceGapCount = Number(patternAiStage1TradeCadence?.gap_count ?? 0);
  const patternAiStage1TradeCadenceBuckets = patternAiStage1TradeCadence
    ? [
        { label: '0-1m', count: patternAiStage1TradeCadence.gap_0_1m },
        { label: '>1-5m', count: patternAiStage1TradeCadence.gap_1_5m },
        { label: '>5-15m', count: patternAiStage1TradeCadence.gap_5_15m },
        { label: '>15-30m', count: patternAiStage1TradeCadence.gap_15_30m },
        { label: '>30-60m', count: patternAiStage1TradeCadence.gap_30_60m },
        { label: '>60m', count: patternAiStage1TradeCadence.gap_over_60m },
      ].map((bucket) => ({
        ...bucket,
        count: Number(bucket.count || 0),
        percent: patternAiStage1TradeCadenceGapCount
          ? (Number(bucket.count || 0) / patternAiStage1TradeCadenceGapCount) * 100
          : 0,
      }))
    : [];
  const patternAiStage1TradeWorkloadCards = patternAiStage1TradeWorkload
    ? [
        {
          key: 'daily',
          title: 'Daily',
          value: formatDecimal(patternAiStage1TradeWorkload.avg_trades_per_day, 1),
          unit: 'avg trades / day',
          stats: [
            { label: 'Min', value: formatNumber(patternAiStage1TradeWorkload.min_trades_per_day) },
            { label: 'Max', value: formatNumber(patternAiStage1TradeWorkload.max_trades_per_day) },
            { label: 'Trading Days', value: formatNumber(patternAiStage1TradeWorkload.active_days) },
            { label: 'Days > 20', value: formatNumber(patternAiStage1TradeWorkload.days_over_20_trades) },
          ],
        },
        {
          key: 'hourly',
          title: 'Hourly',
          value: formatDecimal(patternAiStage1TradeWorkload.avg_trades_per_hour, 1),
          unit: 'avg trades / active hour',
          stats: [
            { label: 'Max', value: formatNumber(patternAiStage1TradeWorkload.max_trades_per_hour) },
            { label: 'Hours > 5', value: formatNumber(patternAiStage1TradeWorkload.hours_over_5_trades) },
          ],
        },
        {
          key: 'weekly',
          title: 'Weekly',
          value: formatDecimal(patternAiStage1TradeWorkload.avg_trades_per_week, 1),
          unit: 'avg trades / active week',
          stats: [
            { label: 'Max', value: formatNumber(patternAiStage1TradeWorkload.max_trades_per_week) },
            { label: 'Active Weeks', value: formatNumber(patternAiStage1TradeWorkload.active_weeks) },
          ],
        },
        {
          key: 'monthly',
          title: 'Monthly',
          value: formatDecimal(patternAiStage1TradeWorkload.avg_trades_per_month, 1),
          unit: 'avg trades / active month',
          stats: [
            { label: 'Max', value: formatNumber(patternAiStage1TradeWorkload.max_trades_per_month) },
            { label: 'Active Months', value: formatNumber(patternAiStage1TradeWorkload.active_months) },
          ],
        },
      ]
    : [];
  const patternAiStage1TopSymbol = patternAiStage1SymbolContributionRows[0] ?? null;
  const patternAiStage1WorstSymbol = patternAiStage1SymbolContributionRows.reduce(
    (worst, row) => (Number(row.sum_r || 0) < Number(worst?.sum_r ?? Number.POSITIVE_INFINITY) ? row : worst),
    null
  );
  const patternAiStage1TopFamily = patternAiStage1FamilyContributionRows[0] ?? null;
  const patternAiStage1WorstFamily = patternAiStage1FamilyContributionRows.reduce(
    (worst, row) => (Number(row.sum_r || 0) < Number(worst?.sum_r ?? Number.POSITIVE_INFINITY) ? row : worst),
    null
  );
  const patternAiStage1PrimaryTemplate = patternAiStage1TemplatePerformanceRows[0] ?? null;
  const patternAiStage1Timeframes = [
    ...new Set(
      patternAiStage1TradeRows
        .map((row) => row.source_timeframe || row.family_time_bin)
        .filter(Boolean)
    ),
  ];
  const patternAiStage1Directions = [
    ...new Set(
      patternAiStage1TradeRows
        .map((row) => row.trade_direction || row.market)
        .filter(Boolean)
        .map((value) => String(value).toUpperCase())
    ),
  ];
  const patternAiStage1ExitReasons = [
    ...new Set(patternAiStage1TradeRows.map((row) => row.exit_reason).filter(Boolean)),
  ];
  const patternAiStage1HasFixedTargets = patternAiStage1TradeRows.some(
    (row) => row.target_price !== null && row.target_price !== undefined
  );
  const patternAiStage1RuleSummary = [
    patternAiStage1PrimaryTemplate?.template_name || patternAiStage1TradeRows[0]?.template_name || 'Candle Wave',
    patternAiStage1Timeframes.length ? patternAiStage1Timeframes.join(', ') : '2m',
    patternAiStage1Directions.length ? patternAiStage1Directions.join(' / ') : 'LONG / SHORT',
    'Market entry',
    'Fixed SL',
    patternAiStage1HasFixedTargets ? 'Fixed TP' : 'Dynamic exit / no fixed TP',
  ].filter(Boolean).join(' | ');
  const patternAiStage1OverviewItems = [
    {
      label: 'Model Run',
      value: compactText(patternAiStage1TradeData.run?.multi_valid_eval_run_id || 'N/A', 30),
      title: patternAiStage1TradeData.run?.multi_valid_eval_run_id || 'N/A',
      wide: true,
    },
    {
      label: 'Rules',
      value: patternAiStage1RuleSummary,
      title: patternAiStage1RuleSummary,
      wide: true,
    },
    {
      label: 'Train Years',
      value: patternAiStage1TradeData.run
        ? `${patternAiStage1TradeData.run.train_start_year}-${patternAiStage1TradeData.run.train_end_year}`
        : 'N/A',
    },
    { label: 'Test Year', value: patternAiStage1TradeData.run?.valid_year || 'N/A' },
    { label: 'Trades Taken', value: formatNumber(patternAiStage1TradeData.summary?.total_trades) },
    { label: 'Wins', value: formatNumber(patternAiStage1TradeData.summary?.wins), tone: 'win' },
    { label: 'Losses', value: formatNumber(patternAiStage1TradeData.summary?.losses), tone: 'loss' },
    {
      label: 'Win Rate',
      value: `${formatDecimal(patternAiStage1TradeData.summary?.win_rate, 2)}%`,
      tone: Number(patternAiStage1TradeData.summary?.win_rate || 0) >= 25 ? 'win' : 'loss',
    },
    {
      label: 'Net R',
      value: `${formatDecimal(patternAiStage1TradeData.summary?.sum_r, 1)}R`,
      tone: Number(patternAiStage1TradeData.summary?.sum_r || 0) >= 0 ? 'win' : 'loss',
    },
    {
      label: 'Avg R',
      value: `${formatDecimal(patternAiStage1TradeData.summary?.avg_r, 3)}R`,
      tone: Number(patternAiStage1TradeData.summary?.avg_r || 0) >= 0 ? 'win' : 'loss',
    },
    { label: 'Best R', value: `${formatDecimal(patternAiStage1TradeData.summary?.best_r, 2)}R`, tone: 'win' },
    { label: 'Worst R', value: `${formatDecimal(patternAiStage1TradeData.summary?.worst_r, 2)}R`, tone: 'loss' },
    { label: 'Symbols Taken', value: formatNumber(patternAiStage1TradeData.summary?.symbol_count) },
    { label: 'Families', value: formatNumber(patternAiStage1TradeData.summary?.family_count) },
    { label: 'Templates', value: formatNumber(patternAiStage1TradeData.summary?.template_count) },
    {
      label: 'Exit Rules Seen',
      value: patternAiStage1ExitReasons.length ? patternAiStage1ExitReasons.map(formatRouteMode).join(' / ') : 'N/A',
      title: patternAiStage1ExitReasons.join(' / ') || 'N/A',
      wide: true,
    },
  ];
  const patternAiStage1TradeHasMore =
    patternAiStage1TradeRows.length < Number(patternAiStage1TradeData.totalRows || 0);
  const patternReversalAiHasMore =
    patternReversalAiRows.length < Number(patternReversalAiData.totalRows || 0);
  const patternXaOutcomeHasMore =
    patternXaOutcomeRows.length < Number(patternXaOutcomeData.totalRows || 0);
  const selectedPatternXaOutcomeRowIndex = selectedPatternXaOutcomeRowKey
    ? patternXaOutcomeRows.findIndex(
        (row) => getPatternXaOutcomeRowKey(row) === selectedPatternXaOutcomeRowKey
      )
    : -1;
  const patternXaOutcomeTotal = Number(patternXaOutcomeRun?.patterns_scanned || patternXaOutcomeData.totalRows || 0);
  const patternXaOutcomeRate = (count) =>
    patternXaOutcomeTotal ? (Number(count || 0) / patternXaOutcomeTotal) * 100 : 0;
  const loadPatternXaOutcomes = useCallback(
    async ({ offset = 0, append = false } = {}) => {
      try {
        setPatternXaOutcomeLoading(true);
        setPatternXaOutcomeError('');
        const result = await fetchPatternXaOutcomes({ limit: 300, offset });
        setPatternXaOutcomeData((current) => ({
          run: result.run,
          totalRows: result.total_rows,
          limit: result.limit,
          offset: result.offset,
          familyRows: result.family_rows ?? [],
          rows: append ? [...current.rows, ...(result.rows ?? [])] : result.rows ?? [],
        }));
      } catch (error) {
        console.error(error);
        setPatternXaOutcomeError('Could not load reversal pattern rows.');
        if (!append) {
          setPatternXaOutcomeData({ run: null, totalRows: 0, limit: 300, offset: 0, familyRows: [], rows: [] });
        }
      } finally {
        setPatternXaOutcomeLoading(false);
      }
    },
    []
  );
  const loadPatternReversalAiScores = useCallback(
    async ({ offset = 0, append = false } = {}) => {
      try {
        setPatternReversalAiLoading(true);
        setPatternReversalAiError('');
        const result = await fetchPatternReversalAiScores({ limit: 300, offset });
        setPatternReversalAiData((current) => ({
          run: result.run,
          totalRows: result.total_rows,
          limit: result.limit,
          offset: result.offset,
          thresholds: result.thresholds ?? [],
          buckets: result.buckets ?? [],
          rows: append ? [...current.rows, ...(result.rows ?? [])] : result.rows ?? [],
        }));
      } catch (error) {
        console.error(error);
        setPatternReversalAiError('Could not load AI reversal score rows.');
        if (!append) {
          setPatternReversalAiData({ run: null, totalRows: 0, limit: 300, offset: 0, thresholds: [], buckets: [], rows: [] });
        }
      } finally {
        setPatternReversalAiLoading(false);
      }
    },
    []
  );
  const loadPatternAiStage1Trades = useCallback(
    async ({ offset = 0, append = false } = {}) => {
      try {
        setPatternAiStage1TradeLoading(true);
        setPatternAiStage1TradeError('');
        const result = await fetchPatternAiStage1Trades({
          aiRunId: PATTERN_AI_STAGE1_DEFAULT_RUN_ID,
          validYear: PATTERN_AI_STAGE1_DEFAULT_YEAR,
          takenOnly: true,
          limit: PATTERN_AI_STAGE1_TRADE_PAGE_SIZE,
          offset,
        });
        setPatternAiStage1TradeData((current) => ({
          run: result.run,
          summary: result.summary,
          templatePerformance: result.template_performance ?? [],
          daily: result.daily ?? [],
          hourly: result.hourly ?? [],
          tradeCadence: result.trade_cadence ?? null,
          tradeWorkload: result.trade_workload ?? null,
          symbolContribution: result.symbol_contribution ?? [],
          familyContribution: result.family_contribution ?? [],
          lossWindows: result.loss_windows ?? [],
          totalRows: result.total_rows,
          limit: result.limit,
          offset: result.offset,
          rows: append ? [...current.rows, ...(result.rows ?? [])] : result.rows ?? [],
        }));
      } catch (error) {
        console.error(error);
        setPatternAiStage1TradeError('Could not load AI Stage 1 trade rows.');
        if (!append) {
          setPatternAiStage1TradeData({
            run: null,
            summary: null,
            templatePerformance: [],
            daily: [],
            hourly: [],
            tradeCadence: null,
            tradeWorkload: null,
            symbolContribution: [],
            familyContribution: [],
            lossWindows: [],
            totalRows: 0,
            limit: PATTERN_AI_STAGE1_TRADE_PAGE_SIZE,
            offset: 0,
            rows: [],
          });
        }
      } finally {
        setPatternAiStage1TradeLoading(false);
      }
    },
    []
  );
  const loadPatternAiExitModelTrades = useCallback(
    async ({ offset = 0, append = false } = {}) => {
      try {
        setPatternAiExitModelTradeLoading(true);
        setPatternAiExitModelTradeError('');
        const result = await fetchPatternAiExitModelTrades({
          exitModelRunId: PATTERN_AI_EXIT_MAIN_RUN_ID,
          validYear: PATTERN_AI_STAGE1_DEFAULT_YEAR,
          changedOnly: true,
          heldLongerOnly: false,
          limit: PATTERN_AI_EXIT_TRADE_PAGE_SIZE,
          offset,
        });
        setPatternAiExitModelTradeData((current) => ({
          run: result.run,
          summary: result.summary,
          totalRows: result.total_rows,
          limit: result.limit,
          offset: result.offset,
          rows: append ? [...current.rows, ...(result.rows ?? [])] : result.rows ?? [],
        }));
      } catch (error) {
        console.error(error);
        setPatternAiExitModelTradeError('Could not load AI exit model rows.');
        if (!append) {
          setPatternAiExitModelTradeData({
            run: null,
            summary: null,
            totalRows: 0,
            limit: PATTERN_AI_EXIT_TRADE_PAGE_SIZE,
            offset: 0,
            rows: [],
          });
        }
      } finally {
        setPatternAiExitModelTradeLoading(false);
      }
    },
    []
  );
  const loadPatternAiExitModelTakenTrades = useCallback(
    async ({ offset = 0, append = false } = {}) => {
      try {
        setPatternAiExitModelTakenTradeLoading(true);
        setPatternAiExitModelTakenTradeError('');
        const result = await fetchPatternAiExitModelTrades({
          exitModelRunId: PATTERN_AI_EXIT_MAIN_RUN_ID,
          validYear: PATTERN_AI_STAGE1_DEFAULT_YEAR,
          changedOnly: false,
          heldLongerOnly: false,
          limit: PATTERN_AI_EXIT_TAKEN_TRADE_PAGE_SIZE,
          offset,
        });
        setPatternAiExitModelTakenTradeData((current) => ({
          run: result.run,
          summary: result.summary,
          totalRows: result.total_rows,
          limit: result.limit,
          offset: result.offset,
          rows: append ? [...current.rows, ...(result.rows ?? [])] : result.rows ?? [],
        }));
      } catch (error) {
        console.error(error);
        setPatternAiExitModelTakenTradeError('Could not load 180-bar AI exit taken trades.');
        if (!append) {
          setPatternAiExitModelTakenTradeData({
            run: null,
            summary: null,
            totalRows: 0,
            limit: PATTERN_AI_EXIT_TAKEN_TRADE_PAGE_SIZE,
            offset: 0,
            rows: [],
          });
        }
      } finally {
        setPatternAiExitModelTakenTradeLoading(false);
      }
    },
    []
  );
  const toggleDataCenterSection = useCallback((sectionKey) => {
    setDataCenterCollapsedSections((current) => ({
      ...current,
      [sectionKey]: current[sectionKey] === false,
    }));
  }, []);
  const applyPatternNavigationSet = useCallback(() => {
    if (!visiblePatternBrowseRows.length) {
      return;
    }

    const firstPattern = visiblePatternBrowseRows[0];
    const firstPatternFamilyKey = getPatternFamilyKey(firstPattern);
    const matchingTrade = routeTrades.find((trade) => patternMatchesTrade(firstPattern, trade));

    setAppliedPatternNavigation({
      familyKey: patternBrowseScope === 'selected' ? selectedFamilyKey : null,
      label: patternBrowseFilterLabel,
      rows: visiblePatternBrowseRows,
      scope: patternBrowseScope,
      search: patternBrowseSearch.trim(),
      symbol: patternBrowseSymbol,
      timeframe: timeframeFilter,
    });
    if (firstPatternFamilyKey && firstPatternFamilyKey !== selectedFamilyKey) {
      setSelectedFamilyKey(firstPatternFamilyKey);
    }
    setSelectedFamilyPatternKey(getFamilyPatternKey(firstPattern));
    setSelectedRouteTradeKey(matchingTrade ? getRouteTradeKey(matchingTrade) : null);
  }, [
    patternBrowseFilterLabel,
    patternBrowseScope,
    patternBrowseSearch,
    patternBrowseSymbol,
    routeTrades,
    selectedFamilyKey,
    timeframeFilter,
    visiblePatternBrowseRows,
  ]);
  const clearPatternNavigationSet = useCallback(() => {
    setAppliedPatternNavigation(null);
    setPatternBrowseSymbol('All');
    setPatternBrowseSearch('');
  }, []);
  const patternNavigationRows = appliedPatternNavigation?.rows?.length
    ? appliedPatternNavigation.rows
    : selectedFamilyPatternRows;
  const activePatternNavigationLabel = appliedPatternNavigation
    ? appliedPatternNavigation.label
    : `Selected family | ${timeframeFilter === 'All' ? 'All timeframes' : timeframeFilter} | All symbols`;
  const selectedFamilyPattern = useMemo(
    () =>
      [...familyPatterns, ...patternBrowseRows].find((pattern) => {
        const patternFamilyKey = getPatternFamilyKey(pattern);
        return (
          getFamilyPatternKey(pattern) === selectedFamilyPatternKey &&
          (!selectedFamilyKey || !patternFamilyKey || patternFamilyKey === selectedFamilyKey)
        );
      }) ?? null,
    [familyPatterns, patternBrowseRows, selectedFamilyKey, selectedFamilyPatternKey]
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
  const moveSelectedPattern = useCallback(
    (direction) => {
      if (!patternNavigationRows.length) {
        return;
      }

      const currentIndex = patternNavigationRows.findIndex(
        (pattern) => getFamilyPatternKey(pattern) === selectedFamilyPatternKey
      );
      if (currentIndex < 0) {
        return;
      }

      const nextIndex = currentIndex + direction;
      if (nextIndex < 0 || nextIndex >= patternNavigationRows.length) {
        return;
      }

      const nextPattern = patternNavigationRows[nextIndex];
      if (!nextPattern) {
        return;
      }

      const nextFamilyKey = getPatternFamilyKey(nextPattern);
      const matchingTrade = routeTrades.find((trade) => patternMatchesTrade(nextPattern, trade));
      if (nextFamilyKey && nextFamilyKey !== selectedFamilyKey) {
        setSelectedFamilyKey(nextFamilyKey);
      }
      setSelectedFamilyPatternKey(getFamilyPatternKey(nextPattern));
      setSelectedRouteTradeKey(matchingTrade ? getRouteTradeKey(matchingTrade) : null);
    },
    [patternNavigationRows, routeTrades, selectedFamilyKey, selectedFamilyPatternKey]
  );
  const selectedSourceLabel =
    SOURCE_OPTIONS.find((option) => option.value === sourceScope)?.label ?? 'All';
  const selectedTimeframeLabel =
    TIMEFRAME_OPTIONS.find((option) => option.value === timeframeFilter)?.label ?? 'All';
  const selectedSimAccountRules = SIM_ACCOUNT_RULES[simAccountSize] ?? SIM_ACCOUNT_RULES['50K'];
  const simContractsCount = Math.max(1, Number.parseInt(String(simContracts), 10) || 1);
  const simTestsCount = Math.max(1, Math.min(250, Number.parseInt(String(simTestsToChain), 10) || 1));
  const simDailyLossLimit =
    simDrawdownModel === 'eod' ? selectedSimAccountRules.eodDailyLossLimit : null;
  const selectedTradeSource = selectedSimulationRawTrade ?? selectedRouteTrade;
  const isSelectedSimulationRawTrade = Boolean(selectedSimulationRawTrade);
  const selectedTradeResultR = Number(selectedTradeSource?.result_r);
  const selectedTradePnl = Number(selectedTradeSource?.pnl);
  const selectedTradeIsSkipped = Boolean(selectedTradeSource?.skipped_for_overlap);
  const selectedTradeIsLoss =
    Number(selectedTradeSource?.trade_result) === 2 ||
    String(selectedTradeSource?.outcome || '').toLowerCase() === 'fail' ||
    String(selectedTradeSource?.exit_reason || '').toLowerCase() === 'stop' ||
    (Number.isFinite(selectedTradeResultR) && selectedTradeResultR < 0) ||
    (Number.isFinite(selectedTradePnl) && selectedTradePnl < 0);
  const selectedTradeHasTrade = Boolean(selectedTradeSource);
  const selectedTradeOutcomeLabel = selectedTradeSource
    ? selectedTradeIsSkipped
      ? 'Skipped'
      : isSelectedSimulationRawTrade && selectedTradeSource.outcome
        ? formatRouteMode(selectedTradeSource.outcome)
        : selectedTradeIsLoss
          ? 'Loss'
          : 'Win'
    : 'N/A';
  const selectedTradeTone = selectedTradeHasTrade
    ? selectedTradeIsSkipped
      ? 'skipped'
      : selectedTradeIsLoss
        ? 'loss'
        : 'win'
    : '';
  const selectedTradeSideLabel = getTradeSide(selectedTradeSource);
  const selectedTradeSummaryOutcome = selectedTradeHasTrade
    ? selectedTradeIsSkipped
      ? 'SKIPPED'
      : selectedTradeIsLoss
        ? 'LOSS'
        : 'PROFIT'
    : 'NO TRADE';
  const selectedTradeSummaryLabel = selectedTradeHasTrade
    ? [selectedTradeSideLabel, selectedTradeSummaryOutcome].filter(Boolean).join(' ')
    : selectedTradeSummaryOutcome;
  const selectedTradeSummaryR = Number.isFinite(selectedTradeResultR)
    ? `${formatDecimal(selectedTradeResultR, 2)}R`
    : 'R N/A';
  const inspectorHoveredCandleStats = [
    { label: 'Candle', value: inspectorHoveredCandle?.dateLabel || formatInspectorHoverDate(inspectorHoveredCandle?.date), wide: true },
    { label: 'O', value: formatInspectorHoverPrice(inspectorHoveredCandle?.open) },
    { label: 'H', value: formatInspectorHoverPrice(inspectorHoveredCandle?.high) },
    { label: 'L', value: formatInspectorHoverPrice(inspectorHoveredCandle?.low) },
    { label: 'C', value: formatInspectorHoverPrice(inspectorHoveredCandle?.close) },
    { label: 'V', value: formatInspectorHoverVolume(inspectorHoveredCandle?.volume) },
  ];
  const selectedTradeEntryPrice =
    selectedTradeSource?.trade_enter_price ??
    selectedTradeSource?.entry_price ??
    canvasPattern?.trade_enter_price;
  const selectedTradeStopPrice =
    selectedTradeSource?.trade_risk_exit_price ??
    selectedTradeSource?.stop_price ??
    canvasPattern?.trade_risk_exit_price;
  const selectedTradeTargetPrice =
    selectedTradeSource?.trade_reward_exit_price ??
    selectedTradeSource?.target_price ??
    canvasPattern?.trade_reward_exit_price;
  const selectedTradeExitPrice =
    selectedTradeSource?.exit_price ??
    selectedTradeSource?.trade_exit_price ??
    canvasPattern?.target_close ??
    canvasPattern?.trade_current_price;
  const selectedTradeEntryDate =
    selectedTradeSource?.entry_date ??
    selectedTradeSource?.d_confirm_date ??
    canvasPattern?.entry_date;
  const selectedTradeExitDate =
    selectedTradeSource?.target_date ??
    selectedTradeSource?.exit_date ??
    canvasPattern?.target_date;
  const selectedTradeTimeframe =
    selectedTradeSource?.source_timeframe ??
    selectedTradeSource?.sourceTimeframe ??
    canvasPattern?.source_timeframe ??
    canvasPattern?.sourceTimeframe ??
    'N/A';
  const selectedTradeIdValue =
    selectedTradeSource?.trade_uid ??
    selectedTradeSource?.trade_id ??
    selectedTradeSource?.id ??
    null;
  const selectedTradeIdDisplay = selectedTradeIdValue
    ? String(selectedTradeIdValue).startsWith('#')
      ? String(selectedTradeIdValue)
      : selectedTradeSource?.trade_uid
        ? String(selectedTradeIdValue)
        : `#${selectedTradeIdValue}`
    : 'Replay only';
  const selectedTradeIdCopyValue =
    selectedTradeIdValue === null || selectedTradeIdValue === undefined ? '' : String(selectedTradeIdValue);
  const handleCopyTradeId = useCallback(async () => {
    if (!selectedTradeIdCopyValue) {
      return;
    }

    try {
      const copied = await copyTextToClipboard(selectedTradeIdCopyValue);
      if (copied) {
        setCopiedTradeId(selectedTradeIdCopyValue);
      }
    } catch (copyError) {
      console.error(copyError);
    }
  }, [selectedTradeIdCopyValue]);
  useEffect(() => {
    if (!copiedTradeId) {
      return undefined;
    }

    const timeoutId = window.setTimeout(() => setCopiedTradeId(''), 1400);
    return () => window.clearTimeout(timeoutId);
  }, [copiedTradeId]);
  const handleCopyPatternId = useCallback(async (value, event) => {
    event?.stopPropagation();
    const text = String(value ?? '').trim();
    if (!text) {
      return;
    }

    try {
      const copied = await copyTextToClipboard(text);
      if (copied) {
        setCopiedPatternId(text);
      }
    } catch (copyError) {
      console.error(copyError);
    }
  }, []);
  useEffect(() => {
    if (!copiedPatternId) {
      return undefined;
    }

    const timeoutId = window.setTimeout(() => setCopiedPatternId(''), 1400);
    return () => window.clearTimeout(timeoutId);
  }, [copiedPatternId]);
  const selectedTradeTemplateUid =
    selectedTradeSource?.template_uid ??
    selectedTradeSource?.template_id ??
    selectedRoute?.template_uid ??
    null;
  const selectedTradeTemplateCandidates = [
    selectedTradeSource,
    ...(entryExitData.templates ?? []),
    ...(entryExitRouterData.family_routes ?? []),
    ...(entryExitRouterData.template_performance ?? []),
  ].filter(Boolean);
  const selectedTradeTemplateHasRule = (template) =>
    Boolean(
      template?.rule_json ||
        template?.entry_kind ||
        template?.direction_mode ||
        template?.risk_basis ||
        Number(template?.risk_multiple) ||
        Number(template?.target_r) ||
        Number(template?.max_hold_multiple)
    );
  const selectedTradeTemplateLookup =
    selectedTradeTemplateUid
      ? selectedTradeTemplateCandidates.find(
          (template) => template?.template_uid === selectedTradeTemplateUid && selectedTradeTemplateHasRule(template)
        ) ??
        selectedTradeTemplateCandidates.find((template) => template?.template_uid === selectedTradeTemplateUid) ??
        null
      : null;
  const selectedTradeTemplate = selectedTradeTemplateUid
    ? {
        ...(selectedTradeTemplateLookup ?? {}),
        template_uid: selectedTradeTemplateUid,
        template_label:
          selectedTradeSource?.template_label ||
          selectedTradeTemplateLookup?.template_label ||
          compactText(selectedTradeTemplateUid, 10),
        template_name: selectedTradeSource?.template_name || selectedTradeTemplateLookup?.template_name || '',
        entry_kind: selectedTradeSource?.entry_kind || selectedTradeTemplateLookup?.entry_kind || '',
        direction_mode: selectedTradeSource?.direction_mode || selectedTradeTemplateLookup?.direction_mode || '',
        risk_basis: selectedTradeSource?.risk_basis || selectedTradeTemplateLookup?.risk_basis || '',
        risk_multiple: Number(selectedTradeSource?.risk_multiple) || Number(selectedTradeTemplateLookup?.risk_multiple) || 0,
        target_r: Number(selectedTradeSource?.target_r) || Number(selectedTradeTemplateLookup?.target_r) || 0,
        max_hold_multiple:
          Number(selectedTradeSource?.max_hold_multiple) || Number(selectedTradeTemplateLookup?.max_hold_multiple) || 0,
        rule_json: selectedTradeSource?.rule_json || selectedTradeTemplateLookup?.rule_json || '',
      }
    : null;
  const selectedTradeTemplateRule =
    selectedTradeTemplate && selectedTradeTemplateHasRule(selectedTradeTemplate)
      ? formatEntryExitTemplateRule(selectedTradeTemplate)
      : null;
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
  const selectedTwinSource =
    [selectedTradeSource, canvasPattern, selectedFamilyPattern, selectedRouteTradeFamilyPattern].find(
      (item) => {
        const hasTwinRank = item?.event_rank !== null && item?.event_rank !== undefined;
        const hasTwinCount = item?.event_sister_count !== null && item?.event_sister_count !== undefined;
        const hasTwinScore = item?.event_similarity_score !== null && item?.event_similarity_score !== undefined;
        return Boolean(item?.event_id || hasTwinRank || hasTwinCount || hasTwinScore);
      }
    ) ??
    canvasPattern ??
    selectedFamilyPattern ??
    selectedRouteTradeFamilyPattern ??
    selectedTradeSource ??
    null;
  const selectedTwinId = selectedTwinSource?.event_id ?? null;
  const selectedTwinRank = optionalNumber(selectedTwinSource?.event_rank);
  const selectedTwinCount = optionalNumber(selectedTwinSource?.event_sister_count);
  const selectedTwinScore = optionalNumber(selectedTwinSource?.event_similarity_score);
  const selectedTwinPrimary =
    selectedTwinSource?.is_event_primary === null || selectedTwinSource?.is_event_primary === undefined
      ? null
      : Boolean(selectedTwinSource.is_event_primary);
  const selectedTwinStats = [
    {
      label: 'Twin Cluster',
      value: selectedTwinId ?? 'Unassigned',
      title: selectedTwinId ?? 'No twin cluster has been assigned to this pattern yet.',
      wide: true,
      tone: selectedTwinId ? 'twin' : 'skipped',
    },
    {
      label: 'Twins',
      value: selectedTwinCount ? formatNumber(selectedTwinCount) : 'N/A',
      tone: selectedTwinCount && selectedTwinCount > 1 ? 'twin' : '',
    },
    {
      label: 'Twin Rank',
      value: selectedTwinRank
        ? selectedTwinCount
          ? `#${formatNumber(selectedTwinRank)} of ${formatNumber(selectedTwinCount)}`
          : `#${formatNumber(selectedTwinRank)}`
        : 'N/A',
    },
    {
      label: 'Primary',
      value: selectedTwinPrimary === null ? 'N/A' : selectedTwinPrimary ? 'Yes' : 'No',
      tone: selectedTwinPrimary ? 'twin' : '',
    },
    {
      label: 'Similarity',
      value: selectedTwinScore === null ? 'N/A' : formatDecimal(selectedTwinScore, 2),
    },
  ];
  const selectedTradeDetailStats = [
    {
      label: 'Trade ID',
      value: selectedTradeIdDisplay,
      wide: true,
      code: true,
      onClick: selectedTradeIdCopyValue ? handleCopyTradeId : null,
      copyLabel: copiedTradeId === selectedTradeIdCopyValue ? 'Copied' : 'Copy',
      title: selectedTradeIdCopyValue || selectedTradeIdDisplay,
    },
    {
      label: 'Test Run ID',
      value:
        selectedSimulationRawTrade?.sim_run_id ??
        selectedRoute?.run_id ??
        selectedTradeSource?.run_id ??
        'N/A',
      wide: true,
      code: true,
    },
    ...(isSelectedSimulationRawTrade
      ? [
          {
            label: 'Setup ID',
            value: selectedSimulationRawTrade?.setup_id ?? 'N/A',
            title: selectedSimulationRawTrade?.setup_id ?? 'N/A',
            wide: true,
            code: true,
          },
          {
            label: 'Template',
            value:
              selectedSimulationRawTrade?.template_label ||
              compactText(selectedSimulationRawTrade?.template_uid || 'N/A', 12),
            title: selectedSimulationRawTrade?.template_uid || selectedSimulationRawTrade?.template_label || 'N/A',
          },
          {
            label: 'Template Logic',
            value: selectedTradeTemplateRule ?? 'N/A',
            title: selectedTradeTemplate?.template_name || selectedTradeTemplate?.template_uid || selectedTradeTemplateRule || 'N/A',
            wide: true,
            code: true,
          },
          {
            label: 'Entry Rule',
            value: selectedTradeTemplate ? getEntryExitTemplateEntryLabel(selectedTradeTemplate) : 'N/A',
          },
          {
            label: 'Risk Rule',
            value: selectedTradeTemplate ? getEntryExitTemplateRiskValue(selectedTradeTemplate) : 'N/A',
            tone: 'loss',
          },
          {
            label: 'Target Rule',
            value: selectedTradeTemplate?.target_r ? `${formatDecimal(selectedTradeTemplate.target_r, 2)}R` : 'N/A',
            tone: 'win',
          },
          {
            label: 'Time Rule',
            value: selectedTradeTemplate?.max_hold_multiple
              ? `${formatNumber(selectedTradeTemplate.max_hold_multiple)}x hold`
              : 'N/A',
          },
          {
            label: 'Cycle',
            value: selectedSimulationRawTrade?.cycle_number
              ? `#${formatNumber(selectedSimulationRawTrade.cycle_number)}`
              : 'N/A',
          },
          {
            label: 'Raw Row',
            value: selectedSimulationRawTrade?.raw_row_number
              ? `#${formatNumber(selectedSimulationRawTrade.raw_row_number)}`
              : 'N/A',
          },
        ]
      : []),
    {
      label: 'Pattern ID',
      value:
        selectedTradeSource?.pattern_id ??
        selectedTradeSource?.pattern_group_id ??
        canvasPattern?.pattern_id ??
        'N/A',
      wide: true,
    },
    ...selectedTwinStats,
    { label: 'Symbol', value: selectedTradeSource?.symbol ?? canvasPattern?.symbol ?? 'N/A' },
    { label: 'Timeframe', value: selectedTradeTimeframe || 'N/A' },
    {
      label: 'Direction',
      value: formatTradeDirection(selectedTradeSource),
      tone: getTradeSide(selectedTradeSource) === 'SHORT' ? 'loss' : getTradeSide(selectedTradeSource) === 'LONG' ? 'win' : '',
    },
    {
      label: 'Result',
      value: selectedTradeOutcomeLabel,
      tone: selectedTradeTone,
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
      value: formatDecimal(selectedTradeEntryPrice, 2),
    },
    {
      label: 'Stop Price',
      value: formatDecimal(selectedTradeStopPrice, 2),
      tone: 'loss',
    },
    {
      label: 'Target Price',
      value: formatDecimal(selectedTradeTargetPrice, 2),
      tone: 'win',
    },
    {
      label: 'Exit Price',
      value: formatDecimal(selectedTradeExitPrice, 2),
    },
    { label: 'Entry Date', value: formatDate(selectedTradeEntryDate) },
    { label: 'Exit Date', value: formatDate(selectedTradeExitDate) },
    {
      label: 'Exit Reason',
      value: selectedTradeSource?.exit_reason ? formatRouteMode(selectedTradeSource.exit_reason) : 'N/A',
    },
    {
      label: 'Risk Points',
      value: Number.isFinite(Number(selectedTradeSource?.risk_points ?? canvasPattern?.risk_points))
        ? formatDecimal(selectedTradeSource?.risk_points ?? canvasPattern?.risk_points, 2)
      : 'N/A',
    },
    ...(isSelectedSimulationRawTrade
      ? [
          {
            label: 'Duration',
            value: formatGapDuration(selectedSimulationRawTrade?.duration_minutes),
          },
          {
            label: 'TP Progress',
            value:
              selectedSimulationRawTrade?.tp_progress_pct_after === null ||
              selectedSimulationRawTrade?.tp_progress_pct_after === undefined
                ? 'N/A'
                : `${formatDecimal(selectedSimulationRawTrade.tp_progress_pct_after, 1)}%`,
            tone: 'win',
          },
          {
            label: 'DD Progress',
            value:
              selectedSimulationRawTrade?.drawdown_progress_pct_after === null ||
              selectedSimulationRawTrade?.drawdown_progress_pct_after === undefined
                ? 'N/A'
                : `${formatDecimal(selectedSimulationRawTrade.drawdown_progress_pct_after, 1)}%`,
            tone: 'loss',
          },
        ]
      : []),
  ];
  const selectedFamilyDetailStats = [...selectedFamilyStats, ...selectedRouteStats];
  const selectedRouteMarket =
    selectedTradeSource?.market ?? canvasPattern?.market ?? selectedFamily?.market ?? '';
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
  const selectedTradeTemplateOverlayDetails = selectedTradeTemplate
    ? [
        {
          label: 'Entry',
          action: getEntryExitTemplateEntryLabel(selectedTradeTemplate),
          value: `${getEntryExitTemplateDirectionLabel(selectedTradeTemplate)} direction`,
          meta: selectedTradeSource
            ? `${formatDate(selectedTradeEntryDate)} @ ${formatDecimal(selectedTradeEntryPrice, 2)}`
            : null,
        },
        {
          label: 'Stop',
          action: `${getEntryExitTemplateRiskValue(selectedTradeTemplate)} STOP`,
          value: `${getEntryExitTemplateRiskLabel(selectedTradeTemplate)} risk distance from entry`,
          tone: 'loss',
        },
        {
          label: 'Target',
          action: `${formatDecimal(selectedTradeTemplate.target_r, 2)}R TARGET`,
          value: `takes profit at ${formatDecimal(selectedTradeTemplate.target_r, 2)}R from entry risk`,
          tone: 'win',
        },
        {
          label: 'Time Exit',
          action: `${selectedTradeTemplate.max_hold_multiple || '-'}X HOLD`,
          value: `closes after ${selectedTradeTemplate.max_hold_multiple || '-'}x pattern hold if stop/target has not hit`,
        },
      ]
    : [];
  const selectedTradeTemplateLogicStats = [
    {
      label: 'Template Logic',
      value: selectedTradeTemplateRule ?? 'N/A',
      title: selectedTradeTemplate?.template_name || selectedTradeTemplate?.template_uid || selectedTradeTemplateRule || 'N/A',
      wide: true,
      code: true,
    },
    {
      label: 'Template ID',
      value: selectedTradeTemplate?.template_uid ? compactText(selectedTradeTemplate.template_uid, 28) : 'N/A',
      title: selectedTradeTemplate?.template_uid ?? 'N/A',
      wide: true,
      code: true,
    },
    { label: 'Label', value: selectedTradeTemplate?.template_label ?? 'N/A' },
    { label: 'Direction', value: selectedTradeTemplate ? getEntryExitTemplateDirectionLabel(selectedTradeTemplate) : 'N/A' },
    { label: 'Entry', value: selectedTradeTemplate ? getEntryExitTemplateEntryLabel(selectedTradeTemplate) : 'N/A' },
    {
      label: 'Risk',
      value: selectedTradeTemplate ? getEntryExitTemplateRiskValue(selectedTradeTemplate) : 'N/A',
      tone: 'loss',
    },
    {
      label: 'Target',
      value: selectedTradeTemplate?.target_r ? `${formatDecimal(selectedTradeTemplate.target_r, 2)}R` : 'N/A',
      tone: 'win',
    },
    {
      label: 'Max Hold',
      value: selectedTradeTemplate?.max_hold_multiple
        ? `${formatNumber(selectedTradeTemplate.max_hold_multiple)}x pattern hold`
        : 'N/A',
    },
    {
      label: 'Template Name',
      value: selectedTradeTemplate?.template_name || 'N/A',
      wide: true,
    },
  ];
  const selectedLogicStats = selectedTradeTemplate ? selectedTradeTemplateLogicStats : selectedRouteLogicStats;
  const selectedLogicOverlayDetails = selectedTradeTemplate
    ? selectedTradeTemplateOverlayDetails
    : selectedRouteOverlayDetails;
  const selectedPatternMarketTone =
    String(selectedFamilyPattern?.market ?? selectedFamily?.market ?? '').toLowerCase() === 'bearish'
      ? 'loss'
      : String(selectedFamilyPattern?.market ?? selectedFamily?.market ?? '').toLowerCase() === 'bullish'
        ? 'win'
        : '';
  const selectedFamilyRowMetrics = [
    { label: 'Family', value: selectedFamily?.family_key ?? 'N/A', wide: true },
    { label: 'Type', value: selectedFamilyLabel, wide: true },
    { label: 'TF', value: selectedTimeframeLabel },
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
    {
      label: 'Nav Set',
      value: activePatternNavigationLabel,
      tone: appliedPatternNavigation ? 'win' : 'skipped',
      wide: true,
    },
    { label: 'Nav Rows', value: formatNumber(patternNavigationRows.length), tone: appliedPatternNavigation ? 'win' : '' },
    { label: 'TF', value: selectedTimeframeLabel },
    { label: 'Symbol', value: selectedFamilyPattern?.symbol ?? 'N/A' },
    { label: 'Market', value: selectedFamilyPattern?.market ?? selectedFamily?.market ?? 'N/A', tone: selectedPatternMarketTone },
    { label: 'Harmonic', value: selectedFamilyPattern?.harmonic_type ?? selectedFamily?.harmonic_type ?? 'N/A' },
    { label: 'D Date', value: formatDate(selectedFamilyPattern?.d_date) },
    { label: 'Confirm', value: formatDate(selectedFamilyPattern?.d_confirm_date) },
    { label: 'Trade', value: selectedPatternTrade ? `#${selectedPatternTrade.trade_index ?? '-'}` : 'No trade', tone: selectedPatternTrade ? 'win' : '' },
  ];
  const selectedPatternDetailSource =
    canvasPattern ?? selectedFamilyPattern ?? selectedRouteTradeFamilyPattern ?? null;
  const isRawCanvasView = Boolean(canvasChartData.rust_patterns?.raw_candle_view);
  const selectedPatternFamilyKey =
    selectedPatternDetailSource ? getPatternFamilyKey(selectedPatternDetailSource) : selectedFamily?.family_key ?? null;
  const selectedPatternDetailSymbol =
    selectedPatternDetailSource?.symbol ??
    selectedFamilyPattern?.symbol ??
    selectedRouteTrade?.symbol ??
    canvasChartData.rust_patterns?.symbol ??
    'N/A';
  const selectedPatternDetailFamily = [
    selectedPatternDetailSource?.harmonic_type ?? selectedFamily?.harmonic_type,
    selectedFamily?.bin,
    selectedFamily?.size_bucket,
  ].filter(Boolean).join(' / ') || 'N/A';
  const selectedPatternThreeMonthTrend =
    selectedPatternDetailSource?.three_month_trend ??
    (selectedPatternDetailSource?.three_month === true
      ? 'Bullish'
      : selectedPatternDetailSource?.three_month === false
        ? 'Bearish'
        : 'N/A');
  const selectedPatternSixMonthTrend =
    selectedPatternDetailSource?.six_month_trend ??
    (selectedPatternDetailSource?.six_month === true
      ? 'Bullish'
      : selectedPatternDetailSource?.six_month === false
        ? 'Bearish'
        : 'N/A');
  const selectedPatternTwelveMonthTrend =
    selectedPatternDetailSource?.twelve_month_trend ??
    (selectedPatternDetailSource?.twelve_month === true
      ? 'Bullish'
      : selectedPatternDetailSource?.twelve_month === false
        ? 'Bearish'
        : 'N/A');
  const selectedPatternPriceAccuracy = [
    selectedPatternDetailSource?.bat_accuracy,
    selectedPatternDetailSource?.butterfly_accuracy,
    selectedPatternDetailSource?.gartley_accuracy,
    selectedPatternDetailSource?.crab_accuracy,
    selectedPatternDetailSource?.shark_accuracy,
  ].find((value) => Number.isFinite(Number(value)));
  const loadCanvasPatternDirectly = useCallback(
    async (chartPattern, errorMessage = 'Could not load the selected trade on the canvas.') => {
      if (!chartPattern?.symbol) {
        setCanvasError('This trade does not have a symbol to load on the canvas.');
        return;
      }

      try {
        setCanvasLoading(true);
        setCanvasError('');
        setCanvasPattern(chartPattern);

        const [candles, snrLines] = await Promise.all([
          getCandles(chartPattern.symbol, buildPatternCandleWindow(chartPattern)).then(normalizeCandles),
          getSupportResistanceLines(chartPattern.symbol),
        ]);

        formatPattern(
          clipCandlesAfterCanvasEnd(candles, chartPattern),
          chartPattern,
          snrLines,
          setCanvasChartData
        );
      } catch (loadError) {
        console.error(loadError);
        setCanvasError(errorMessage);
      } finally {
        setCanvasLoading(false);
      }
    },
    []
  );
  const loadPatternDirectly = useCallback(
    async (patternSummary, matchingTrade = null) => {
      if (!patternSummary?.pattern_id && !patternSummary?.pattern_group_id) {
        return;
      }

      try {
        setCanvasLoading(true);
        setCanvasError('');

        const detail = await fetchPatternDetail({
          ...patternSummary,
          prop_outcome_mode: patternSummary.prop_outcome_mode ?? 'phase1-family',
        });
        if (!detail) {
          setCanvasError('Could not load the selected twin pattern.');
          return;
        }

        const chartPattern = mergeRouteTradeIntoPattern(detail, matchingTrade);
        setCanvasPattern(chartPattern);

        const [candles, snrLines] = await Promise.all([
          chartPattern.symbol
            ? getCandles(chartPattern.symbol, buildPatternCandleWindow(chartPattern)).then(normalizeCandles)
            : Promise.resolve([]),
          getSupportResistanceLines(chartPattern.symbol),
        ]);

        formatPattern(
          clipCandlesAfterCanvasEnd(candles, chartPattern),
          chartPattern,
          snrLines,
          setCanvasChartData
        );
      } catch (loadError) {
        console.error(loadError);
        setCanvasError('Could not load the selected twin pattern.');
      } finally {
        setCanvasLoading(false);
      }
    },
    []
  );
  const loadPatternXaOutcomeRow = useCallback(
    (row) => {
      if (!row) {
        return;
      }

      const xaOutcomeExitPrice =
        row.outcome === 'reversal_xa'
          ? row.reversal_target_price
          : row.outcome === 'continuation_xa'
            ? row.continuation_target_price
            : null;
      setSelectedPatternXaOutcomeRowKey(getPatternXaOutcomeRowKey(row));
      setSelectedSimulationRawTradeKey(null);
      setSelectedSimulationRawTrade(null);
      setInspectorDetailMode('pattern');
      void loadPatternDirectly(
        {
          pattern_id: row.pattern_id || undefined,
          pattern_group_id: row.pattern_group_id || undefined,
          prop_outcome_mode: 'phase1-family',
        },
        row.hit_date
          ? {
              canvas_end_date: row.hit_date,
              xa_canvas_mode: true,
              xa_start_price: row.d_price,
              xa_reversal_limit_price: row.reversal_target_price,
              xa_continuation_limit_price: row.continuation_target_price,
              xa_outcome_hit_date: row.hit_date,
              xa_outcome: row.outcome,
              xa_outcome_price: xaOutcomeExitPrice,
            }
          : {
              xa_canvas_mode: true,
              xa_start_price: row.d_price,
              xa_reversal_limit_price: row.reversal_target_price,
              xa_continuation_limit_price: row.continuation_target_price,
              xa_outcome: row.outcome,
              xa_outcome_price: xaOutcomeExitPrice,
            }
      );
    },
    [loadPatternDirectly]
  );
  const handlePatternXaOutcomeTableKeyDown = useCallback(
    (event) => {
      if (!['ArrowDown', 'ArrowUp'].includes(event.key) || !patternXaOutcomeRows.length) {
        return;
      }

      event.preventDefault();
      const currentIndex =
        selectedPatternXaOutcomeRowIndex >= 0 ? selectedPatternXaOutcomeRowIndex : -1;
      const nextIndex =
        event.key === 'ArrowDown'
          ? Math.min(currentIndex + 1, patternXaOutcomeRows.length - 1)
          : Math.max(currentIndex - 1, 0);
      loadPatternXaOutcomeRow(patternXaOutcomeRows[nextIndex]);
    },
    [loadPatternXaOutcomeRow, patternXaOutcomeRows, selectedPatternXaOutcomeRowIndex]
  );
  useEffect(() => {
    if (!selectedPatternXaOutcomeRowKey) {
      return;
    }

    patternXaOutcomeRowRefs.current
      .get(selectedPatternXaOutcomeRowKey)
      ?.scrollIntoView({ block: 'nearest' });
  }, [selectedPatternXaOutcomeRowKey]);
  const handleSimulationRawTradeClick = useCallback(
    (trade, index = 0) => {
      if (!trade?.pattern_id && !trade?.pattern_group_id) {
        setCanvasError('This raw trade does not have a pattern id to load on the canvas.');
        return;
      }

      const normalizedTrade = normalizeSimulationRawTradeForCanvas(trade);
      setSelectedSimulationRawTradeKey(getSimulationRawTradeKey(trade, index));
      setSelectedSimulationRawTrade({
        ...normalizedTrade,
        sim_run_id: entryExitSimRawTradesData.sim_run_id || trade.sim_run_id || '',
        raw_row_number: Number(entryExitSimRawTradesData.offset || 0) + index + 1,
      });
      setInspectorDetailMode('trade');

      void loadPatternDirectly(
        {
          pattern_id: trade.pattern_id || undefined,
          pattern_group_id: trade.pattern_group_id || undefined,
          prop_outcome_mode: 'phase1-family',
        },
        normalizedTrade
      );
    },
    [entryExitSimRawTradesData.offset, entryExitSimRawTradesData.sim_run_id, loadPatternDirectly]
  );
  const handlePatternAiStage1TradeClick = useCallback(
    (trade, index = 0) => {
      if (!trade?.pattern_id && !trade?.pattern_group_id) {
        setCanvasError('This AI trade row does not have a pattern id to load on the canvas.');
        return;
      }

      const sourceTimeframe = getAiStage1TradeTimeframe(trade);
      const normalizedTrade = {
        ...normalizeSimulationRawTradeForCanvas(trade),
        source_timeframe: sourceTimeframe,
        sourceTimeframe,
      };
      setSelectedPatternXaOutcomeRowKey('');
      setSelectedSimulationRawTradeKey(getSimulationRawTradeKey(trade, index));
      setSelectedSimulationRawTrade({
        ...normalizedTrade,
        sim_run_id: trade.multi_valid_eval_run_id || patternAiStage1TradeData.run?.multi_valid_eval_run_id || '',
        raw_row_number: Number(patternAiStage1TradeData.offset || 0) + index + 1,
      });
      setInspectorDetailMode('trade');

      if (isAiCandleWaveTrade(trade)) {
        void loadCanvasPatternDirectly(
          buildAiStage1VirtualPattern({ ...trade, ...normalizedTrade }),
          'Could not load the selected AI candle-wave trade.'
        );
        return;
      }

      void loadPatternDirectly(
        {
          pattern_id: trade.pattern_id || undefined,
          pattern_group_id: trade.pattern_group_id || undefined,
          prop_outcome_mode: 'phase1-family',
        },
        normalizedTrade
      );
    },
    [
      loadCanvasPatternDirectly,
      loadPatternDirectly,
      patternAiStage1TradeData.offset,
      patternAiStage1TradeData.run,
    ]
  );
  const handleTwinPatternClick = useCallback(
    (patternId) => {
      const nextPattern =
        [...familyPatterns, ...patternBrowseRows].find((pattern) => pattern.pattern_id === patternId) ?? null;
      const matchingTrade =
        routeTrades.find(
          (trade) => trade.pattern_id === patternId || trade.pattern_group_id === nextPattern?.pattern_group_id
        ) ?? null;

      setInspectorDetailMode('pattern');

      if (nextPattern) {
        const nextFamilyKey = getPatternFamilyKey(nextPattern);
        if (nextFamilyKey && nextFamilyKey !== selectedFamilyKey) {
          setSelectedFamilyKey(nextFamilyKey);
        }
        setSelectedFamilyPatternKey(getFamilyPatternKey(nextPattern));
        setSelectedRouteTradeKey(matchingTrade ? getRouteTradeKey(matchingTrade) : null);
        return;
      }

      setSelectedRouteTradeKey(matchingTrade ? getRouteTradeKey(matchingTrade) : null);
      void loadPatternDirectly({ pattern_id: patternId, prop_outcome_mode: 'phase1-family' }, matchingTrade);
    },
    [familyPatterns, loadPatternDirectly, patternBrowseRows, routeTrades, selectedFamilyKey]
  );
  const selectedTwinPatternIds = Array.isArray(selectedPatternDetailSource?.twin_pattern_ids)
    ? selectedPatternDetailSource.twin_pattern_ids
    : [];
  const selectedTwinPatternCards = selectedTwinPatternIds.map((patternId, index) => ({
    label: `Twin Pattern ${index + 1}`,
    value: patternId,
    title: 'Click to load this twin pattern',
    wide: true,
    code: true,
    tone: patternId === selectedPatternDetailSource?.pattern_id ? 'twin' : '',
    onClick: () => handleTwinPatternClick(patternId),
  }));
  const selectedPatternDetailStats = [
    { label: 'Symbol', value: selectedPatternDetailSymbol },
    { label: 'Family', value: selectedPatternDetailFamily, wide: true },
    {
      label: 'Pattern ID',
      value: selectedPatternDetailSource?.pattern_id ?? 'N/A',
      wide: true,
      code: true,
    },
    {
      label: 'Pattern Group',
      value: selectedPatternDetailSource?.pattern_group_id ?? 'N/A',
      wide: true,
      code: true,
    },
    ...selectedTwinStats,
    ...selectedTwinPatternCards,
    {
      label: 'Market',
      value: selectedPatternDetailSource?.market ?? selectedFamily?.market ?? 'N/A',
      tone: selectedPatternMarketTone,
    },
    { label: 'Harmonic', value: selectedPatternDetailSource?.harmonic_type ?? selectedFamily?.harmonic_type ?? 'N/A' },
    {
      label: 'Family Key',
      value: selectedPatternFamilyKey ?? 'N/A',
      wide: true,
      code: true,
    },
    { label: 'D Date', value: formatDate(selectedPatternDetailSource?.d_date) },
    { label: 'Confirm', value: formatDate(selectedPatternDetailSource?.d_confirm_date) },
    { label: 'X Len', value: formatOptionalNumber(selectedPatternDetailSource?.x_length) },
    { label: 'A Len', value: formatOptionalNumber(selectedPatternDetailSource?.a_length) },
    { label: 'B Len', value: formatOptionalNumber(selectedPatternDetailSource?.b_length) },
    { label: 'C Len', value: formatOptionalNumber(selectedPatternDetailSource?.c_length) },
    { label: 'D Len', value: formatOptionalNumber(selectedPatternDetailSource?.d_length) },
    { label: 'Full Len', value: formatOptionalNumber(selectedPatternDetailSource?.full_pattern_length) },
    {
      label: 'Price Acc',
      value: Number.isFinite(Number(selectedPatternPriceAccuracy))
        ? `${formatDecimal(selectedPatternPriceAccuracy, 1)}%`
        : 'N/A',
    },
    {
      label: 'Time Acc',
      value: Number.isFinite(Number(selectedPatternDetailSource?.time_accuracy))
        ? `${formatDecimal(selectedPatternDetailSource?.time_accuracy, 1)}%`
        : 'N/A',
    },
    { label: '3M Trend', value: selectedPatternThreeMonthTrend, tone: selectedPatternThreeMonthTrend === 'Bearish' ? 'loss' : selectedPatternThreeMonthTrend === 'Bullish' ? 'win' : '' },
    { label: '6M Trend', value: selectedPatternSixMonthTrend, tone: selectedPatternSixMonthTrend === 'Bearish' ? 'loss' : selectedPatternSixMonthTrend === 'Bullish' ? 'win' : '' },
    { label: '12M Trend', value: selectedPatternTwelveMonthTrend, tone: selectedPatternTwelveMonthTrend === 'Bearish' ? 'loss' : selectedPatternTwelveMonthTrend === 'Bullish' ? 'win' : '' },
    {
      label: 'Nav Set',
      value: activePatternNavigationLabel,
      tone: appliedPatternNavigation ? 'win' : 'skipped',
      wide: true,
    },
    { label: 'Nav Rows', value: formatNumber(patternNavigationRows.length), tone: appliedPatternNavigation ? 'win' : '' },
    { label: 'Trade Link', value: selectedPatternTrade ? `Trade #${selectedPatternTrade.trade_index ?? '-'}` : 'No trade', tone: selectedPatternTrade ? 'win' : '' },
  ];
  const selectedTradeRowMetrics = [
    {
      label: 'Trade ID',
      value: selectedTradeIdDisplay === 'Replay only' ? 'N/A' : selectedTradeIdDisplay,
      wide: true,
    },
    { label: 'Side', value: formatTradeDirection(selectedTradeSource), tone: getTradeSide(selectedTradeSource) === 'SHORT' ? 'loss' : getTradeSide(selectedTradeSource) === 'LONG' ? 'win' : '' },
    {
      label: 'Result',
      value: selectedTradeOutcomeLabel,
      tone: selectedTradeTone,
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
    { label: 'Entry', value: formatDate(selectedTradeEntryDate) },
    { label: 'Exit', value: formatDate(selectedTradeExitDate) },
    {
      label: 'Exit Why',
      value: selectedTradeSource?.exit_reason ? formatRouteMode(selectedTradeSource.exit_reason) : 'N/A',
      tone: selectedTradeSource?.exit_reason === 'stop' ? 'loss' : selectedTradeSource?.exit_reason === 'target' ? 'win' : '',
    },
    { label: 'Pattern', value: selectedTradeSource?.pattern_id ?? selectedTradeSource?.pattern_group_id ?? 'N/A', wide: true },
  ];
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
  const entryExitRun = entryExitData.run;
  const entryExitTemplates = entryExitData.templates ?? [];
  const entryExitBuildCoverageRows = useMemo(() => entryExitData.coverage ?? [], [entryExitData.coverage]);
  const entryExitBuildCoverageSymbolRows = useMemo(() => {
    return entryExitBuildCoverageRows
      .map((row) => {
        const rootSymbol = normalizeFuturesRootSymbol(row.root_symbol || row.contract_symbol || 'Unknown');
        const scannedCount = Number(row.scanned_pattern_count ?? row.pattern_count ?? 0);
        const status = row.status || (scannedCount > 0 ? 'Scanned' : 'Not scanned');
        return {
          root_symbol: rootSymbol,
          exchange: row.exchange_name || row.exchange || getFuturesExchange(rootSymbol) || 'Unknown',
          pattern_count: scannedCount,
          contract_count: Number(row.contract_count || 0),
          universe_pattern_count: Number(row.universe_pattern_count || 0),
          timeframe_label: row.source_timeframe || selectedBuildTimeframeLabel || 'unknown',
          is_scanned: status === 'Scanned',
          sort_order: Number.isFinite(Number(row.sort_order)) ? Number(row.sort_order) : null,
          status,
        };
      })
      .sort((left, right) =>
        (Number.isFinite(Number(left.sort_order)) ? Number(left.sort_order) : Number.MAX_SAFE_INTEGER) -
          (Number.isFinite(Number(right.sort_order)) ? Number(right.sort_order) : Number.MAX_SAFE_INTEGER) ||
          Number(right.pattern_count || 0) - Number(left.pattern_count || 0) ||
          String(left.root_symbol).localeCompare(String(right.root_symbol))
      );
  }, [entryExitBuildCoverageRows, selectedBuildTimeframeLabel]);
  const entryExitBuildExchangeSections = useMemo(() => {
    const exchangeOrder = ['CME', 'CBOT', 'NYMEX', 'COMEX', 'Unknown'];
    const groups = entryExitBuildCoverageSymbolRows.reduce((map, row) => {
      const exchange = row.exchange || 'Unknown';
      const current = map.get(exchange) ?? [];
      current.push(row);
      map.set(exchange, current);
      return map;
    }, new Map());

    return [...groups.entries()]
      .map(([exchange, rows]) => ({
        exchange,
        pattern_count: rows.reduce((sum, row) => sum + Number(row.pattern_count || 0), 0),
        scanned_count: rows.filter((row) => row.is_scanned).length,
        rows: rows.sort((left, right) =>
          (Number.isFinite(Number(left.sort_order)) ? Number(left.sort_order) : Number.MAX_SAFE_INTEGER) -
            (Number.isFinite(Number(right.sort_order)) ? Number(right.sort_order) : Number.MAX_SAFE_INTEGER) ||
          Number(right.is_scanned) - Number(left.is_scanned) ||
          Number(right.pattern_count || 0) - Number(left.pattern_count || 0) ||
          String(left.root_symbol).localeCompare(String(right.root_symbol))
        ),
      }))
      .sort((left, right) => {
        const leftIndex = exchangeOrder.indexOf(left.exchange);
        const rightIndex = exchangeOrder.indexOf(right.exchange);
        const normalizedLeftIndex = leftIndex === -1 ? exchangeOrder.length : leftIndex;
        const normalizedRightIndex = rightIndex === -1 ? exchangeOrder.length : rightIndex;
        return (
          normalizedLeftIndex - normalizedRightIndex ||
          Number(right.pattern_count || 0) - Number(left.pattern_count || 0) ||
          left.exchange.localeCompare(right.exchange)
        );
      });
  }, [entryExitBuildCoverageSymbolRows]);
  const selectedBuildCoverageExchangeSection =
    entryExitBuildExchangeSections.find(
      (section) => getExchangeClassSuffix(section.exchange) === selectedBuildCoverageExchangeKey
    ) ??
    entryExitBuildExchangeSections[0] ??
    null;
  const selectedBuildCoveragePatternCount = selectedBuildSummary
    ? Number(selectedBuildSummary.coverage_patterns || 0)
    : null;
  const selectedBuildRootCount = selectedBuildSummary
    ? Number(selectedBuildSummary.root_count || 0)
    : null;
  const selectedBuildUniverseRootCount = selectedBuildSummary
    ? entryExitBuildCoverageSymbolRows.length || selectedBuildRootCount || 0
    : null;
  const selectedBuildExchangeCount = selectedBuildSummary
    ? Number(selectedBuildSummary.exchange_count || 0)
    : null;
  const selectedBuildRootCardValue = selectedBuildSummary
    ? `${formatNumber(selectedBuildRootCount)} / ${formatNumber(selectedBuildUniverseRootCount)}`
    : '';

  useEffect(() => {
    if (!entryExitBuildExchangeSections.length) {
      if (selectedBuildCoverageExchangeKey) setSelectedBuildCoverageExchangeKey('');
      return;
    }
    const hasSelectedExchange = entryExitBuildExchangeSections.some(
      (section) => getExchangeClassSuffix(section.exchange) === selectedBuildCoverageExchangeKey
    );
    if (!hasSelectedExchange) {
      setSelectedBuildCoverageExchangeKey(getExchangeClassSuffix(entryExitBuildExchangeSections[0].exchange));
    }
  }, [entryExitBuildExchangeSections, selectedBuildCoverageExchangeKey]);

  const displayedEntryExitTemplates = Array.from(
    new Map(
      entryExitTemplates.map((template) => [
        template.template_uid || `${template.created_from_symbol}-${template.created_from_d_confirm_date}`,
        template,
      ])
    ).values()
  ).slice(0, entryExitRun?.templates_created || entryExitTemplates.length);
  const getEntryExitTemplateLabel = (template) => {
    const index = displayedEntryExitTemplates.findIndex(
      (item) => item.template_uid === template?.template_uid
    );
    return `T${String(index >= 0 ? index + 1 : 0).padStart(2, '0')}`;
  };
  const entryExitBestTemplate =
    displayedEntryExitTemplates
      .filter((template) => Number(template.eval_count) > 0)
      .sort((left, right) => {
        const leftPassRate = Number(left.pass_count || 0) / Math.max(1, Number(left.eval_count || 0));
        const rightPassRate = Number(right.pass_count || 0) / Math.max(1, Number(right.eval_count || 0));
        return (
          Number(right.avg_r || 0) - Number(left.avg_r || 0) ||
          rightPassRate - leftPassRate ||
          Number(right.pass_count || 0) - Number(left.pass_count || 0)
        );
      })[0] ?? null;
  const selectedEntryExitTemplate =
    displayedEntryExitTemplates.find((template) => template.template_uid === selectedEntryExitTemplateUid) ??
    displayedEntryExitTemplates[0] ??
    null;
  const selectedEntryExitTemplateLabel = selectedEntryExitTemplate
    ? getEntryExitTemplateLabel(selectedEntryExitTemplate)
    : 'T--';
  const selectedEntryExitEvalCount = Number(selectedEntryExitTemplate?.eval_count || 0);
  const selectedEntryExitPassRate = selectedEntryExitEvalCount
    ? (Number(selectedEntryExitTemplate?.pass_count || 0) / selectedEntryExitEvalCount) * 100
    : 0;
  const selectedEntryExitFailRate = selectedEntryExitEvalCount
    ? (Number(selectedEntryExitTemplate?.fail_count || 0) / selectedEntryExitEvalCount) * 100
    : 0;
  const selectedEntryExitNoEntryRate = selectedEntryExitEvalCount
    ? (Number(selectedEntryExitTemplate?.no_entry_count || 0) / selectedEntryExitEvalCount) * 100
    : 0;
  const selectedBuildPrimaryTemplate = selectedEntryExitTemplate ?? displayedEntryExitTemplates[0] ?? null;
  const selectedBuildPrimaryEvalCount = Number(selectedBuildPrimaryTemplate?.eval_count || 0);
  const selectedBuildPrimaryPassCount = Number(selectedBuildPrimaryTemplate?.pass_count || 0);
  const selectedBuildPrimaryFailCount = Number(selectedBuildPrimaryTemplate?.fail_count || 0);
  const selectedBuildPrimaryNoEntryCount = Number(selectedBuildPrimaryTemplate?.no_entry_count || 0);
  const selectedBuildPrimaryAvgR = Number(selectedBuildPrimaryTemplate?.avg_r || 0);
  const selectedBuildPrimaryNetR = selectedBuildPrimaryAvgR * selectedBuildPrimaryEvalCount;
  const selectedBuildInfoRows = [
    { label: 'Build ID', value: selectedBuildRunId || 'N/A', wide: true },
    { label: 'Label', value: selectedBuildLabel || 'N/A' },
    { label: 'Years', value: selectedBuildYearLabel || 'N/A' },
    { label: 'Source', value: selectedBuildSourceLabel || 'N/A' },
    { label: 'TF', value: selectedBuildTimeframeLabel || 'N/A' },
    {
      label: 'Scope',
      value: `${selectedBuildSourceLabel || 'N/A'} | ${selectedBuildTimeframeLabel || 'N/A'} | ${selectedBuildYearLabel || 'N/A'}`,
    },
    {
      label: 'Rule',
      value: selectedBuildPrimaryTemplate ? formatEntryExitTemplateRule(selectedBuildPrimaryTemplate) : 'N/A',
      wide: true,
    },
    {
      label: 'Direction',
      value: selectedBuildPrimaryTemplate ? getEntryExitTemplateDirectionLabel(selectedBuildPrimaryTemplate) : 'N/A',
    },
    {
      label: 'Entry',
      value: selectedBuildPrimaryTemplate ? getEntryExitTemplateEntryLabel(selectedBuildPrimaryTemplate) : 'N/A',
    },
    {
      label: 'Risk',
      value: selectedBuildPrimaryTemplate ? getEntryExitTemplateRiskValue(selectedBuildPrimaryTemplate) : 'N/A',
    },
    {
      label: 'Target',
      value: selectedBuildPrimaryTemplate ? `${formatDecimal(selectedBuildPrimaryTemplate.target_r, 2)}R` : 'N/A',
    },
    { label: 'Build Patterns', value: selectedBuildPatternsScanned || 'N/A' },
    { label: 'Rows Tested', value: selectedBuildHasStoredSummary ? formatNumber(selectedBuildSummary.result_rows || selectedBuildPrimaryEvalCount) : formatNumber(selectedBuildPrimaryEvalCount) },
    { label: 'Templates', value: selectedBuildTestsBuilt || 'N/A' },
    {
      label: 'Coverage',
      value: selectedBuildHasStoredSummary ? formatNumber(selectedBuildCoveragePatternCount) : 'N/A',
    },
    { label: 'Roots', value: selectedBuildRootCardValue || 'N/A' },
    {
      label: 'Exchanges',
      value: selectedBuildHasStoredSummary ? formatNumber(selectedBuildExchangeCount) : 'N/A',
    },
    {
      label: 'Result',
      value: selectedBuildPrimaryTemplate
        ? `${formatNumber(selectedBuildPrimaryPassCount)} W / ${formatNumber(selectedBuildPrimaryFailCount)} L / ${formatNumber(selectedBuildPrimaryNoEntryCount)} NE`
        : 'N/A',
      wide: true,
    },
    {
      label: 'Net R',
      value: selectedBuildPrimaryTemplate ? `${formatDecimal(selectedBuildPrimaryNetR, 1)}R` : 'N/A',
      tone: selectedBuildPrimaryNetR >= 0 ? 'win' : 'loss',
    },
    {
      label: 'Avg R',
      value: selectedBuildPrimaryTemplate ? `${formatDecimal(selectedBuildPrimaryAvgR, 3)}R` : 'N/A',
      tone: selectedBuildPrimaryAvgR >= 0 ? 'win' : 'loss',
    },
    {
      label: 'Raw Rows',
      value: Number(selectedBuildSummary?.result_rows || 0) > 0 ? 'Stored' : 'Summary only',
    },
    {
      label: 'Elapsed',
      value: selectedBuildSummary?.elapsed_ms ? `${formatDecimal(Number(selectedBuildSummary.elapsed_ms || 0) / 1000, 1)}s` : 'N/A',
    },
  ];
  const selectedEntryExitEntryOffset = getTemplateEntryOffset(selectedEntryExitTemplate ?? {});
  const selectedEntryExitMarketBreakdown = entryExitTemplateBreakdown.market ?? [];
  const selectedEntryExitHarmonicBreakdown = entryExitTemplateBreakdown.harmonic_type ?? [];
  const selectedEntryExitConditionBreakdown = entryExitTemplateBreakdown.conditions ?? [];
  const selectedEntryExitFamilyResults = entryExitTemplateBreakdown.family_results ?? [];
  const selectedEntryExitComboBreakdown = entryExitTemplateBreakdown.combos ?? [];
  const selectedEntryExitBullishPassCount = Number(selectedEntryExitTemplate?.bullish_pass_count || 0);
  const selectedEntryExitBearishPassCount = Number(selectedEntryExitTemplate?.bearish_pass_count || 0);
  const selectedEntryExitBullishFailCount = Number(selectedEntryExitTemplate?.bullish_fail_count || 0);
  const selectedEntryExitBearishFailCount = Number(selectedEntryExitTemplate?.bearish_fail_count || 0);
  const selectedEntryExitTopWinMarket =
    [...selectedEntryExitMarketBreakdown].sort((left, right) =>
      Number(right.pass_count || 0) - Number(left.pass_count || 0) ||
      Number(right.avg_r || 0) - Number(left.avg_r || 0)
    )[0] ?? null;
  const selectedEntryExitTopLossMarket =
    [...selectedEntryExitMarketBreakdown].sort((left, right) =>
      Number(right.fail_count || 0) - Number(left.fail_count || 0) ||
      Number(left.avg_r || 0) - Number(right.avg_r || 0)
    )[0] ?? null;
  const selectedEntryExitTopWinHarmonic =
    [...selectedEntryExitHarmonicBreakdown].sort((left, right) =>
      Number(right.pass_count || 0) - Number(left.pass_count || 0) ||
      Number(right.avg_r || 0) - Number(left.avg_r || 0)
    )[0] ?? null;
  const selectedEntryExitTopLossHarmonic =
    [...selectedEntryExitHarmonicBreakdown].sort((left, right) =>
      Number(right.fail_count || 0) - Number(left.fail_count || 0) ||
      Number(left.avg_r || 0) - Number(right.avg_r || 0)
    )[0] ?? null;
  const selectedEntryExitSingleEdgeRows = selectedEntryExitConditionBreakdown
    .filter((row) => ENTRY_EXIT_EDGE_FEATURE_ORDER.includes(row.condition_type))
    .filter((row) => Number(row.eval_count || 0) >= 50)
    .map((row) => {
      const evalCount = Number(row.eval_count || 0);
      const passCount = Number(row.pass_count || 0);
      const passRate = evalCount ? (passCount / evalCount) * 100 : 0;
      const avgR = Number(row.avg_r || 0);
      const avgRLift = avgR - Number(selectedEntryExitTemplate?.avg_r || 0);
      const wrLift = passRate - selectedEntryExitPassRate;
      return {
        ...row,
        evalCount,
        passCount,
        passRate,
        avgR,
        wrLift,
        avgRLift,
        edgeClass: classifyEntryExitEdge({ evalCount, avgRLift, wrLift }),
      };
    });
  const selectedEntryExitSingleEdgeMap = new Map(
    selectedEntryExitSingleEdgeRows.map((row) => [`${row.condition_type}::${row.condition_value}`, row])
  );
  const selectedEntryExitTopSingleEdges = [...selectedEntryExitSingleEdgeRows]
    .filter((row) => row.avgRLift > 0)
    .sort((left, right) =>
      right.avgRLift - left.avgRLift ||
      right.wrLift - left.wrLift ||
      right.evalCount - left.evalCount
    )
    .slice(0, 10);
  const selectedEntryExitComboEdgeRows = selectedEntryExitComboBreakdown
    .filter((row) => Number(row.eval_count || 0) >= 50)
    .map((row) => {
      const evalCount = Number(row.eval_count || 0);
      const passCount = Number(row.pass_count || 0);
      const passRate = evalCount ? (passCount / evalCount) * 100 : 0;
      const avgR = Number(row.avg_r || 0);
      const parentA = selectedEntryExitSingleEdgeMap.get(`${row.feature_a_type}::${row.feature_a_value}`);
      const parentB = selectedEntryExitSingleEdgeMap.get(`${row.feature_b_type}::${row.feature_b_value}`);
      const parentBestAvgR = Math.max(
        Number(parentA?.avgR ?? Number.NEGATIVE_INFINITY),
        Number(parentB?.avgR ?? Number.NEGATIVE_INFINITY)
      );
      const parentBestPassRate = Math.max(
        Number(parentA?.passRate ?? Number.NEGATIVE_INFINITY),
        Number(parentB?.passRate ?? Number.NEGATIVE_INFINITY)
      );
      const avgRLift = avgR - Number(selectedEntryExitTemplate?.avg_r || 0);
      const wrLift = passRate - selectedEntryExitPassRate;
      const avgRLiftVsParents = Number.isFinite(parentBestAvgR) ? avgR - parentBestAvgR : avgRLift;
      const wrLiftVsParents = Number.isFinite(parentBestPassRate) ? passRate - parentBestPassRate : wrLift;
      return {
        ...row,
        evalCount,
        passCount,
        passRate,
        avgR,
        parentA,
        parentB,
        avgRLift,
        wrLift,
        avgRLiftVsParents,
        wrLiftVsParents,
        edgeClass: classifyEntryExitEdge({ evalCount, avgRLift, wrLift }),
      };
    });
  const selectedEntryExitTrueComboEdges = [...selectedEntryExitComboEdgeRows]
    .filter((row) => row.avgRLift > 0 && row.avgRLiftVsParents > 0)
    .sort((left, right) =>
      right.avgRLiftVsParents - left.avgRLiftVsParents ||
      right.avgRLift - left.avgRLift ||
      right.evalCount - left.evalCount
    )
    .slice(0, 10);
  const selectedEntryExitBaselineOnlyCombos = [...selectedEntryExitComboEdgeRows]
    .filter((row) => row.avgRLift > 0 && row.avgRLiftVsParents <= 0)
    .sort((left, right) =>
      right.avgRLift - left.avgRLift ||
      right.evalCount - left.evalCount
    )
    .slice(0, 6);
  const selectedEntryExitAvoidCombos = [...selectedEntryExitComboEdgeRows]
    .filter((row) => row.avgRLift < 0)
    .sort((left, right) =>
      left.avgRLift - right.avgRLift ||
      left.avgR - right.avgR
    )
    .slice(0, 6);
  const formatEntryExitFeatureLabel = (featureType, featureValue) =>
    `${ENTRY_EXIT_CONDITION_LABELS[featureType] ?? formatRouteMode(featureType)}: ${featureValue || 'Unknown'}`;
  const selectedEntryExitWinningFamilyRows = [...selectedEntryExitFamilyResults]
    .filter((row) => Number(row.pass_count || 0) > 0)
    .sort((left, right) =>
      Number(right.pass_count || 0) - Number(left.pass_count || 0) ||
      Number(left.fail_count || 0) - Number(right.fail_count || 0) ||
      Number(right.avg_r || 0) - Number(left.avg_r || 0)
    );
  const selectedEntryExitFamilyTotals = selectedEntryExitWinningFamilyRows.reduce(
    (totals, row) => ({
      wins: totals.wins + Number(row.pass_count || 0),
      losses: totals.losses + Number(row.fail_count || 0),
      noEntries: totals.noEntries + Number(row.no_entry_count || 0),
      tests: totals.tests + Number(row.eval_count || 0),
    }),
    { wins: 0, losses: 0, noEntries: 0, tests: 0 }
  );
  const entryExitConditionSectionsForMode = (mode) =>
    ENTRY_EXIT_CONDITION_ORDER
      .filter((conditionType) => !['market', 'harmonic_type'].includes(conditionType))
      .map((conditionType) => {
        const rows = selectedEntryExitConditionBreakdown
          .filter((row) => row.condition_type === conditionType)
          .filter((row) => {
            if (mode === 'wins') return Number(row.pass_count || 0) > 0;
            if (mode === 'losses') return Number(row.fail_count || 0) > 0;
            return Number(row.eval_count || 0) > 0;
          })
          .sort((left, right) => {
            if (mode === 'wins') {
              return Number(right.pass_count || 0) - Number(left.pass_count || 0) ||
                Number(right.avg_r || 0) - Number(left.avg_r || 0);
            }
            if (mode === 'losses') {
              return Number(right.fail_count || 0) - Number(left.fail_count || 0) ||
                Number(left.avg_r || 0) - Number(right.avg_r || 0);
            }
            return Number(right.avg_r || 0) - Number(left.avg_r || 0) ||
              Number(right.pass_count || 0) - Number(left.pass_count || 0);
          })
          .slice(0, 8);

        return {
          title: `${ENTRY_EXIT_CONDITION_LABELS[conditionType] ?? formatRouteMode(conditionType)} ${
            mode === 'wins' ? 'Wins' : mode === 'losses' ? 'Losses' : 'Split'
          }`,
          items: rows.map((row) => {
            const evalCount = Number(row.eval_count || 0);
            const passCount = Number(row.pass_count || 0);
            const failCount = Number(row.fail_count || 0);
            const passRate = evalCount ? (passCount / evalCount) * 100 : 0;
            const failRate = evalCount ? (failCount / evalCount) * 100 : 0;
            const value = mode === 'wins'
              ? `${formatNumber(passCount)} wins / ${formatDecimal(passRate, 1)}% of tests / ${formatDecimal(row.avg_r, 3)}R`
              : mode === 'losses'
                ? `${formatNumber(failCount)} losses / ${formatDecimal(failRate, 1)}% of tests / ${formatDecimal(row.worst_r, 2)}R worst`
                : `${formatDecimal(passRate, 1)}% WR / ${formatDecimal(row.avg_r, 3)}R / ${formatNumber(evalCount)} tests`;
            return {
              label: row.condition_value || 'Unknown',
              value,
              title: `Pass ${formatNumber(passCount)} / Fail ${formatNumber(failCount)} / No Entry ${formatNumber(row.no_entry_count)} / Best ${formatDecimal(row.best_r, 2)}R / Worst ${formatDecimal(row.worst_r, 2)}R`,
              tone: mode === 'losses' || Number(row.avg_r) < 0 ? 'loss' : 'win',
              wide: true,
            };
          }),
        };
      })
      .filter((section) => section.items.length);
  const generatedEntryExitTemplateRows = displayedEntryExitTemplates.map((template, index) => {
    const evalCount = Number(template.eval_count || 0);
    const passRate = evalCount ? (Number(template.pass_count || 0) / evalCount) * 100 : 0;
    const bullishEvalCount = Number(template.bullish_eval_count || 0);
    const bullishPassRate = bullishEvalCount
      ? (Number(template.bullish_pass_count || 0) / bullishEvalCount) * 100
      : 0;
    const bearishEvalCount = Number(template.bearish_eval_count || 0);
    const bearishPassRate = bearishEvalCount
      ? (Number(template.bearish_pass_count || 0) / bearishEvalCount) * 100
      : 0;
    const isSelected =
      template.template_uid === (selectedEntryExitTemplate?.template_uid ?? null);

    return {
      template,
      label: `T${String(index + 1).padStart(2, '0')}`,
      rule: formatEntryExitTemplateRule(template),
      evalCount,
      passRate,
      bullishEvalCount,
      bullishPassRate,
      bearishEvalCount,
      bearishPassRate,
      isSelected,
    };
  });
  const entryExitRouterCurrentRun = entryExitRouterData.current_run;
  const entryExitRouterRuns = useMemo(() => entryExitRouterData.runs ?? [], [entryExitRouterData.runs]);
  const entryExitRouterSymbols = entryExitRouterData.symbols ?? [];
  const entryExitRouterFamilyRoutes = entryExitRouterData.family_routes ?? [];
  const entryExitTemplatePerformanceRows = entryExitRouterData.template_performance ?? [];
  const entryExitManualFamilyBans = entryExitRouterData.manual_family_bans ?? [];
  const entryExitManualSymbolBans = entryExitRouterData.manual_symbol_bans ?? [];
  const selectedEntryExitRouterRun =
    entryExitRouterRuns.find((run) => run.router_run_id === selectedEntryExitRouterRunId) ??
    entryExitRouterCurrentRun ??
    entryExitRouterRuns[0] ??
    null;
  const getEntryExitRouterTradeCount = (run = {}) =>
    Number(run.win_count || 0) + Number(run.loss_count || 0) + Number(run.no_entry_count || 0);
  const getEntryExitRouterWinRate = (run = {}) => {
    const tradeCount = getEntryExitRouterTradeCount(run);
    return tradeCount ? (Number(run.win_count || 0) / tradeCount) * 100 : 0;
  };
  const isEntryExitSimulationRun = (run = {}) =>
    Number(run.test_year || 0) > 0 ||
    Number(run.patterns_scanned || 0) > 0 ||
    getEntryExitRouterTradeCount(run) > 0;
  const entryExitPlaybookLabelByKey = useMemo(() => {
    const playbookRows = entryExitRouterRuns.map((run) => {
      const manualFamilyBansApplied = Number(run.manual_family_bans_applied || 0);
      const name = getEntryExitPlaybookName(run, manualFamilyBansApplied);
      const trainRunId = run.train_run_id || 'unknown';
      const buildLabel =
        entryExitModelDatasets.find((dataset) => dataset.id === trainRunId)?.buildLabel ??
        (run.train_run_id ? compactText(run.train_run_id, 10) : 'B?');
      return {
        key: `${trainRunId}::${name}`,
        buildLabel,
        name,
        rank: getEntryExitPlaybookRank(run, manualFamilyBansApplied),
      };
    });
    const rowsByBuild = playbookRows.reduce((map, row) => {
      const buildRows = map.get(row.buildLabel) ?? [];
      buildRows.push(row);
      map.set(row.buildLabel, buildRows);
      return map;
    }, new Map());
    const labels = new Map();

    rowsByBuild.forEach((buildRows, buildLabel) => {
      const orderedRows = [...new Map(
        buildRows
          .sort((left, right) => left.rank - right.rank || left.name.localeCompare(right.name))
          .map((row) => [row.key, row])
      ).values()];

      orderedRows.forEach((row, index) => {
        labels.set(row.key, `${buildLabel}-P${index + 1}`);
      });
    });

    return labels;
  }, [entryExitModelDatasets, entryExitRouterRuns]);
  const generatedEntryExitRouterRunRows = entryExitRouterRuns.map((run, index) => {
    const propPassed = Number(run.prop?.passed || 0);
    const propDailyFails = Number(run.prop?.daily_fails || 0);
    const propDrawdownFails = Number(run.prop?.drawdown_fails || 0);
    const propFailed = propDailyFails + propDrawdownFails;
    const propClosedTotal = propPassed + propFailed;
    const propTotal = Number(run.prop?.cycles || 0);
    const manualFamilyBansApplied = Number(run.manual_family_bans_applied || 0);
    const playbookName = getEntryExitPlaybookName(run, manualFamilyBansApplied);
    const playbookKey = `${run.train_run_id || 'unknown'}::${playbookName}`;
    const playbookBuildLabel =
      entryExitModelDatasets.find((dataset) => dataset.id === run.train_run_id)?.buildLabel ??
      (run.train_run_id ? compactText(run.train_run_id, 10) : 'B?');
    return {
      run,
      tradeCount: getEntryExitRouterTradeCount(run),
      winRate: getEntryExitRouterWinRate(run),
      propTotal,
      propClosedTotal,
      propPassed,
      propFailed,
      propDailyFails,
      propDrawdownFails,
      propIncomplete: Number(run.prop?.incomplete || 0),
      propClosedPassRate: Number(run.prop?.closed_pass_rate || 0),
      manualFamilyBansApplied,
      playbookBuildLabel,
      playbookKey,
      playbookLabel: entryExitPlaybookLabelByKey.get(playbookKey) ?? `${playbookBuildLabel}-P${index + 1}`,
      playbookName,
      isSelected: run.router_run_id === selectedEntryExitRouterRun?.router_run_id,
    };
  });
  const generatedEntryExitPlaybookTabRows = [...new Map(
    generatedEntryExitRouterRunRows.map((row) => [row.playbookKey, row])
  ).values()];
  const selectedEntryExitRouterRunRow =
    generatedEntryExitRouterRunRows.find(
      (row) => row.run.router_run_id === selectedEntryExitRouterRun?.router_run_id
    ) ??
    generatedEntryExitRouterRunRows[0] ??
    null;
  const hasSelectedEntryExitRouterRun = Boolean(selectedEntryExitRouterRunRow);
  const selectedEntryExitPlaybookKey = selectedEntryExitRouterRunRow?.playbookKey ?? '';
  const selectedPlaybookTestRows = generatedEntryExitRouterRunRows
    .filter(
      (row) =>
        row.playbookKey === selectedEntryExitPlaybookKey &&
        isEntryExitSimulationRun(row.run)
    )
    .sort((left, right) => Date.parse(right.run.created_at || 0) - Date.parse(left.run.created_at || 0));
  const selectedPlaybookTestYearRows = [...new Map(
    selectedPlaybookTestRows.map((row) => [
      getEntryExitSimulationYearKey(row.run),
      {
        key: getEntryExitSimulationYearKey(row.run),
        label: getEntryExitSimulationYearLabel(row.run),
        runCount: selectedPlaybookTestRows.filter(
          (testRow) =>
            getEntryExitSimulationYearKey(testRow.run) === getEntryExitSimulationYearKey(row.run)
        ).length,
      },
    ])
  ).values()].sort((left, right) => {
    if (left.key === 'all') return 1;
    if (right.key === 'all') return -1;
    return Number(right.key) - Number(left.key);
  });
  const selectedEntryExitSimulationYear =
    selectedPlaybookTestYearRows.some((row) => row.key === selectedEntryExitSimulationYearKey)
      ? selectedEntryExitSimulationYearKey
      : selectedPlaybookTestYearRows[0]?.key ?? '';
  const selectedEntryExitPlaybookName = selectedEntryExitRouterRunRow?.playbookName ?? '';
  const selectedEntryExitSimulationRunRow =
    selectedPlaybookTestRows.find(
      (row) => getEntryExitSimulationYearKey(row.run) === selectedEntryExitSimulationYear
    ) ??
    null;
  const selectedEntryExitLogicRunRow =
    selectedEntryExitSimulationRunRow ?? selectedEntryExitRouterRunRow;
  const hasSelectedEntryExitLogicRun = Boolean(selectedEntryExitLogicRunRow);
  const selectedEntryExitSimulationTestId =
    selectedEntryExitSimulationRunRow?.run.router_run_id ?? '';
  const selectedEntryExitPlaybookRunRow =
    generatedEntryExitRouterRunRows.find(
      (row) => row.playbookKey === selectedEntryExitPlaybookKey && !isEntryExitSimulationRun(row.run)
    ) ??
    selectedEntryExitRouterRunRow ??
    null;
  const selectedEntryExitPlaybookRuleRun =
    selectedEntryExitLogicRunRow?.run ?? selectedEntryExitPlaybookRunRow?.run ?? null;
  const selectedEntryExitPlaybookRuleOptions = selectedEntryExitPlaybookRunRow
    ? {
        run: selectedEntryExitPlaybookRuleRun,
        playbookLabel: selectedEntryExitPlaybookRunRow.playbookLabel,
        playbookName: selectedEntryExitPlaybookRunRow.playbookName,
        manualFamilyBansApplied: selectedEntryExitPlaybookRunRow.manualFamilyBansApplied,
      }
    : null;
  const selectedEntryExitPlaybookRuleSections = selectedEntryExitPlaybookRuleOptions
    ? buildEntryExitPlaybookRuleSections(selectedEntryExitPlaybookRuleOptions)
    : [];
  const selectedEntryExitPlaybookDescription = selectedEntryExitPlaybookRuleOptions
    ? buildEntryExitPlaybookDescription(selectedEntryExitPlaybookRuleOptions)
    : '';
  const selectedPlaybookBuildRunId = selectedEntryExitPlaybookRunRow?.run?.train_run_id ?? '';
  const selectedPlaybookBuildDataset = entryExitModelDatasets.find(
    (dataset) => dataset.id === selectedPlaybookBuildRunId
  );
  const selectedPlaybookBuildLabel =
    selectedPlaybookBuildDataset?.buildLabel ??
    (selectedPlaybookBuildRunId ? compactText(selectedPlaybookBuildRunId, 10) : 'Build ?');
  const selectedPlaybookLinkState = !selectedPlaybookBuildRunId
    ? 'missing'
    : selectedPlaybookBuildRunId === selectedBuildRunId
      ? 'connected'
      : 'different';
  const selectedPlaybookLinkStatus =
    selectedPlaybookLinkState === 'connected'
      ? 'Connected'
      : selectedPlaybookLinkState === 'different'
        ? 'Different Build'
        : 'No Build Link';
  const selectedEntryExitFamilyRoutes = entryExitRouterFamilyRoutes.filter(
    (route) => route.router_run_id === selectedEntryExitRouterRun?.router_run_id
  );
  const selectedEntryExitSimulationFamilyRoutes = selectedEntryExitSimulationRunRow
    ? entryExitRouterFamilyRoutes.filter(
        (route) => route.router_run_id === selectedEntryExitSimulationRunRow.run.router_run_id
      )
    : [];
  const selectedPlaybookTradeCount = Number(selectedEntryExitRouterRunRow?.run.trade_choices || 0);
  const selectedPlaybookWatchCount = Number(selectedEntryExitRouterRunRow?.run.watchlist_choices || 0);
  const selectedPlaybookSkipCount = Number(selectedEntryExitRouterRunRow?.run.skip_choices || 0);
  const selectedPlaybookRowCount =
    selectedPlaybookTradeCount + selectedPlaybookWatchCount + selectedPlaybookSkipCount ||
    selectedEntryExitFamilyRoutes.length;
  const selectedPlaybookPlaysCount = new Set(
    selectedEntryExitFamilyRoutes
      .filter((route) => route.route_status === 'TRADE')
      .map((route) => route.template_uid)
      .filter(Boolean)
  ).size;
  const selectedPlaybookUsedPlayRows = [
    ...selectedEntryExitFamilyRoutes
      .filter((route) => route.route_status === 'TRADE')
      .reduce((usedPlays, route) => {
        const playKey = route.template_uid || route.template_label || route.template_name;
        if (!playKey) {
          return usedPlays;
        }

        const current = usedPlays.get(playKey) ?? {
          familyKeys: new Set(),
          label: route.template_label || compactText(playKey, 10),
          name: route.template_name || 'Entry / exit test',
          templateUid: route.template_uid || playKey,
        };
        current.familyKeys.add(route.family_key || `${route.router_run_id}-${route.template_rank}`);
        usedPlays.set(playKey, current);
        return usedPlays;
      }, new Map())
      .values(),
  ]
    .map((row) => ({
      ...row,
      familyCount: row.familyKeys.size,
    }))
    .sort((left, right) => right.familyCount - left.familyCount || left.label.localeCompare(right.label));
  const selectedPlaybookTemplateTotal = selectedBuildSummary
    ? Number(selectedBuildSummary.templates_created || 0)
    : 0;
  const selectedPlaybookAssignedTemplateCount = selectedPlaybookUsedPlayRows.length;
  const selectedPlaybookUnassignedTemplateCount = Math.max(
    selectedPlaybookTemplateTotal - selectedPlaybookAssignedTemplateCount,
    0
  );
  const selectedPlaybookAssignmentRate =
    selectedPlaybookTemplateTotal > 0
      ? (selectedPlaybookAssignedTemplateCount / selectedPlaybookTemplateTotal) * 100
      : 0;
  const selectedPlaybookAssignmentMaxFamilyCount = Math.max(
    1,
    ...selectedPlaybookUsedPlayRows.map((row) => Number(row.familyCount || 0))
  );
  const selectedPlaybookAssignmentChartRows = selectedPlaybookUsedPlayRows.slice(0, 12);
  const selectedSimulationUsedPlayRows = [
    ...selectedEntryExitSimulationFamilyRoutes
      .filter((route) => route.route_status === 'TRADE' && Number(route.test_eval_count || 0) > 0)
      .reduce((usedPlays, route) => {
        const playKey = route.template_uid || route.template_label || route.template_name;
        if (!playKey) {
          return usedPlays;
        }

        const current = usedPlays.get(playKey) ?? {
          evalCount: 0,
          failCount: 0,
          familyKeys: new Set(),
          label: route.template_label || compactText(playKey, 10),
          name: route.template_name || 'Entry / exit test',
          noEntryCount: 0,
          passCount: 0,
          sumR: 0,
          templateUid: route.template_uid || playKey,
        };
        current.evalCount += Number(route.test_eval_count || 0);
        current.passCount += Number(route.test_pass_count || 0);
        current.failCount += Number(route.test_fail_count || 0);
        current.noEntryCount += Number(route.test_no_entry_count || 0);
        current.sumR += Number(route.test_sum_r || 0);
        current.familyKeys.add(route.family_key || `${route.router_run_id}-${route.template_rank}`);
        usedPlays.set(playKey, current);
        return usedPlays;
      }, new Map())
      .values(),
  ]
    .map((row) => ({
      ...row,
      avgR: row.evalCount ? row.sumR / row.evalCount : 0,
      familyCount: row.familyKeys.size,
      winRate: row.evalCount ? (row.passCount / row.evalCount) * 100 : 0,
    }))
    .sort((left, right) => right.evalCount - left.evalCount || left.label.localeCompare(right.label));
  const selectedSimulationTemplatePerformanceRows = entryExitTemplatePerformanceRows
    .filter((row) => row.sim_run_id === selectedEntryExitSimulationTestId)
    .map((row) => ({
      avgR: Number(row.avg_r || 0),
      evalCount: Number(row.eval_count || 0),
      failCount: Number(row.fail_count || 0),
      familyCount: Number(row.family_count || 0),
      label: row.template_label || compactText(row.template_uid || '', 10),
      name: row.template_name || 'Entry / exit test',
      noEntryCount: Number(row.no_entry_count || 0),
      passCount: Number(row.pass_count || 0),
      sumR: Number(row.sum_r || 0),
      templateUid: row.template_uid || '',
      winRate: Number(row.win_rate || 0),
    }))
    .sort(
      (left, right) =>
        Number(right.sumR || 0) - Number(left.sumR || 0) ||
        Number(right.avgR || 0) - Number(left.avgR || 0) ||
        Number(right.evalCount || 0) - Number(left.evalCount || 0) ||
        left.label.localeCompare(right.label)
    );
  const generatedEntryExitFamilyRouterRows = selectedEntryExitFamilyRoutes.map((route, index) => {
    const testEvalCount = Number(route.test_eval_count || 0);
    const testPassRate = testEvalCount
      ? (Number(route.test_pass_count || 0) / testEvalCount) * 100
      : 0;
    return {
      route,
      index: index + 1,
      testEvalCount,
      testPassRate,
      isSelected: route.template_uid === selectedEntryExitTemplate?.template_uid,
    };
  });
  const selectedEntryExitRouterSymbols = entryExitRouterSymbols.filter(
    (symbol) => symbol.router_run_id === selectedEntryExitRouterRun?.router_run_id
  );
  const selectedPlaybookSymbolRows = entryExitRouterSymbols.filter(
    (symbol) => symbol.router_run_id === selectedEntryExitPlaybookRunRow?.run.router_run_id
  );
  const selectedPlaybookTradeSymbolCount = selectedPlaybookSymbolRows.filter(
    (symbol) => symbol.route_status === 'TRADE'
  ).length;
  const selectedPlaybookSkippedSymbolCount = selectedPlaybookSymbolRows.filter(
    (symbol) => symbol.route_status !== 'TRADE'
  ).length;
  const entryExitRouterSkippedSymbols = selectedEntryExitRouterSymbols.filter(
    (symbol) => symbol.route_status === 'SKIP'
  );
  const entryExitRouterTradeSymbols = selectedEntryExitRouterSymbols.filter(
    (symbol) => symbol.route_status === 'TRADE'
  );
  const entryExitRouterSymbolGateRows = [
    ...entryExitRouterSkippedSymbols.slice(0, 10),
    ...entryExitRouterTradeSymbols.slice(0, 5),
  ];
  const selectedEntryExitManualSkipRows = generatedEntryExitFamilyRouterRows
    .filter((row) => row.route.router_run_id === selectedEntryExitRouterRun?.router_run_id)
    .filter((row) => row.route.route_status === 'SKIP')
    .filter((row) => String(row.route.status_reason || '').toLowerCase().includes('manual'));
  const selectedEntryExitManualFamilyBansApplied = selectedEntryExitRouterRunRow
    ? Number(
        selectedEntryExitRouterRunRow.run.manual_family_bans_applied ??
          selectedEntryExitRouterRunRow.manualFamilyBansApplied ??
          0
      )
    : 0;
  const currentManualBanTotal =
    entryExitManualFamilyBans.length + entryExitManualSymbolBans.length;
  const entryExitManualBanItems = [
    ...entryExitManualFamilyBans.map((ban) => ({
      label: `Family ${compactText(ban.family_key, 16)}`,
      value: ban.reason || 'Manual family ban',
      title: `${ban.family_key} | ${ban.reason || 'Manual family ban'}`,
      tone: 'loss',
      wide: true,
    })),
    ...entryExitManualSymbolBans.map((ban) => ({
      label: `Symbol ${ban.root_symbol}`,
      value: ban.reason || 'Manual symbol ban',
      title: `${ban.root_symbol} | ${ban.reason || 'Manual symbol ban'}`,
      tone: 'loss',
      wide: true,
    })),
  ];
  const selectedEntryExitOverlapRules = selectedEntryExitLogicRunRow
    ? [
        selectedEntryExitLogicRunRow.run.one_trade_at_a_time ? 'One active trade total' : null,
        !selectedEntryExitLogicRunRow.run.one_trade_at_a_time &&
        selectedEntryExitLogicRunRow.run.one_trade_per_root_symbol
          ? 'One active trade per root symbol'
          : null,
        Number(selectedEntryExitLogicRunRow.run.trade_cooldown_minutes || 0) > 0
          ? `One entry every ${formatNumber(selectedEntryExitLogicRunRow.run.trade_cooldown_minutes)} rolling minutes`
          : selectedEntryExitLogicRunRow.run.one_trade_per_minute ? 'One entry per minute' : null,
      ].filter(Boolean)
    : [];
  const selectedEntryExitOverlapRule = selectedEntryExitOverlapRules.length
    ? selectedEntryExitOverlapRules.join(' + ')
    : 'No active-trade overlap gate';
  const selectedPlaybookSymbolGateLabel = selectedEntryExitLogicRunRow
    ? selectedEntryExitLogicRunRow.run.symbol_filter_enabled ? 'On' : 'Off'
    : '';
  const selectedPlaybookSymbolGateDetail = selectedEntryExitLogicRunRow?.run.symbol_filter_enabled
    ? `${formatNumber(selectedEntryExitLogicRunRow.run.symbol_trade_roots)} allowed | ${formatNumber(
        selectedEntryExitLogicRunRow.run.symbol_skip_roots
      )} skipped`
    : selectedEntryExitLogicRunRow
      ? 'No symbol gate'
      : '';
  const selectedSimulationRunOverviewSections = selectedEntryExitSimulationRunRow
    ? [
        {
          title: 'Run',
          items: [
            {
              label: 'Sim ID',
              value: compactText(selectedEntryExitSimulationRunRow.run.router_run_id || 'N/A', 18),
              title: selectedEntryExitSimulationRunRow.run.router_run_id || 'N/A',
            },
            {
              label: 'Playbook',
              value: compactText(selectedEntryExitSimulationRunRow.run.playbook_id || 'N/A', 18),
              title: selectedEntryExitSimulationRunRow.run.playbook_id || 'N/A',
            },
            {
              label: 'Build',
              value: compactText(selectedEntryExitSimulationRunRow.run.train_run_id || 'N/A', 18),
              title: selectedEntryExitSimulationRunRow.run.train_run_id || 'N/A',
            },
            { label: 'Source', value: selectedEntryExitSimulationRunRow.run.source_scope || 'N/A' },
            { label: 'TF', value: selectedEntryExitSimulationRunRow.run.source_timeframe || 'N/A' },
            { label: 'Year', value: formatNumber(selectedEntryExitSimulationRunRow.run.test_year) },
            {
              label: 'Created',
              value: formatShortDateTime(selectedEntryExitSimulationRunRow.run.created_at),
              title: String(selectedEntryExitSimulationRunRow.run.created_at || 'N/A'),
            },
            { label: 'Elapsed', value: `${formatNumber(selectedEntryExitSimulationRunRow.run.elapsed_ms)} ms` },
          ],
        },
        {
          title: 'Execution',
          items: [
            { label: 'Sister Window', value: `${formatNumber(selectedEntryExitSimulationRunRow.run.sister_window_minutes)}m` },
            { label: 'One Account', value: selectedEntryExitSimulationRunRow.run.one_trade_at_a_time ? 'On' : 'Off' },
            { label: 'One Root', value: selectedEntryExitSimulationRunRow.run.one_trade_per_root_symbol ? 'On' : 'Off' },
            { label: 'One Minute', value: selectedEntryExitSimulationRunRow.run.one_trade_per_minute ? 'On' : 'Off' },
            { label: 'Cooldown', value: `${formatNumber(selectedEntryExitSimulationRunRow.run.trade_cooldown_minutes)}m` },
            { label: 'Daily Lockout', value: selectedEntryExitSimulationRunRow.run.daily_loss_lockout ? 'On' : 'Off' },
            { label: 'Near Pass', value: selectedEntryExitSimulationRunRow.run.near_pass_protection ? 'On' : 'Off' },
            { label: 'Near Within', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.near_pass_within_r, 2)}R` },
            { label: 'Near Daily', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.near_pass_daily_loss_r, 2)}R` },
            { label: 'Loss Cluster', value: selectedEntryExitSimulationRunRow.run.loss_cluster_day_lockout ? 'On' : 'Off' },
            { label: 'Cluster Count', value: formatNumber(selectedEntryExitSimulationRunRow.run.loss_cluster_loss_count) },
            { label: 'Cluster Window', value: `${formatNumber(selectedEntryExitSimulationRunRow.run.loss_cluster_window_minutes)}m` },
          ],
        },
        {
          title: 'Scan Flow',
          items: [
            { label: 'Patterns', value: formatNumber(selectedEntryExitSimulationRunRow.run.patterns_scanned) },
            { label: 'Routed', value: formatNumber(selectedEntryExitSimulationRunRow.run.routed_patterns), tone: 'win' },
            { label: 'No Route', value: formatNumber(selectedEntryExitSimulationRunRow.run.no_route_patterns) },
            { label: 'Non-Trade Skip', value: formatNumber(selectedEntryExitSimulationRunRow.run.skipped_non_trade_patterns) },
            { label: 'Symbol Skip', value: formatNumber(selectedEntryExitSimulationRunRow.run.skipped_symbol_patterns), tone: 'skipped' },
            { label: 'Overlap Skip', value: formatNumber(selectedEntryExitSimulationRunRow.run.skipped_overlap_patterns), tone: 'skipped' },
          ],
        },
        {
          title: 'Selection',
          items: [
            { label: 'Families', value: formatNumber(selectedEntryExitSimulationRunRow.run.families_selected) },
            { label: 'Trade', value: formatNumber(selectedEntryExitSimulationRunRow.run.trade_choices), tone: 'win' },
            { label: 'Watchlist', value: formatNumber(selectedEntryExitSimulationRunRow.run.watchlist_choices), tone: 'skipped' },
            { label: 'Skip', value: formatNumber(selectedEntryExitSimulationRunRow.run.skip_choices), tone: 'loss' },
            { label: 'Manual Bans', value: formatNumber(selectedEntryExitSimulationRunRow.run.manual_family_bans_applied) },
            { label: 'Trade Roots', value: formatNumber(selectedEntryExitSimulationRunRow.run.symbol_trade_roots), tone: 'win' },
            { label: 'Skip Roots', value: formatNumber(selectedEntryExitSimulationRunRow.run.symbol_skip_roots), tone: 'skipped' },
          ],
        },
        {
          title: 'Result',
          items: [
            { label: 'Wins', value: formatNumber(selectedEntryExitSimulationRunRow.run.win_count), tone: 'win' },
            { label: 'Losses', value: formatNumber(selectedEntryExitSimulationRunRow.run.loss_count), tone: 'loss' },
            { label: 'No Entry', value: formatNumber(selectedEntryExitSimulationRunRow.run.no_entry_count) },
            {
              label: 'Trade Win Rate',
              value: optionalNumber(selectedEntryExitSimulationRunRow.run.trade_win_rate) === null
                ? ''
                : `${formatDecimal(selectedEntryExitSimulationRunRow.run.trade_win_rate, 2)}%`,
            },
            { label: 'Avg R', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.avg_r, 4)}R`, tone: Number(selectedEntryExitSimulationRunRow.run.avg_r) < 0 ? 'loss' : 'win' },
            { label: 'Sum R', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.sum_r, 2)}R`, tone: Number(selectedEntryExitSimulationRunRow.run.sum_r) < 0 ? 'loss' : 'win' },
            { label: 'Best R', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.best_r, 2)}R`, tone: 'win' },
            { label: 'Worst R', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.worst_r, 2)}R`, tone: 'loss' },
          ],
        },
        {
          title: 'Gates',
          items: [
            { label: 'Min Train Tests', value: formatNumber(selectedEntryExitSimulationRunRow.run.min_train_tests) },
            { label: 'Prop Filter', value: selectedEntryExitSimulationRunRow.run.prop_filter_enabled ? 'On' : 'Off' },
            { label: 'Trade Tests', value: formatNumber(selectedEntryExitSimulationRunRow.run.trade_min_tests) },
            { label: 'Trade WR', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.trade_min_win_rate, 2)}%` },
            { label: 'Trade Avg R', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.trade_min_avg_r, 3)}R` },
            { label: 'Watch Tests', value: formatNumber(selectedEntryExitSimulationRunRow.run.watchlist_min_tests) },
            { label: 'Watch WR', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.watchlist_min_win_rate, 2)}%` },
            { label: 'Watch Avg R', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.watchlist_min_avg_r, 3)}R` },
            { label: 'Symbol Filter', value: selectedEntryExitSimulationRunRow.run.symbol_filter_enabled ? 'On' : 'Off' },
            { label: 'Symbol Tests', value: formatNumber(selectedEntryExitSimulationRunRow.run.symbol_min_tests) },
            { label: 'Symbol WR', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.symbol_min_win_rate, 2)}%` },
            { label: 'Symbol Avg R', value: `${formatDecimal(selectedEntryExitSimulationRunRow.run.symbol_min_avg_r, 3)}R` },
          ],
        },
      ]
    : [];
  const selectedPropSimulationSummary = selectedEntryExitSimulationRunRow?.run?.prop ?? {};
  const selectedPropSimulationOverviewSections = selectedEntryExitSimulationRunRow
    ? [
        {
          title: 'Rules',
          items: [
            { label: 'Profit Target', value: `${formatDecimal(selectedPropSimulationSummary.profit_target_r, 0)}R`, tone: 'win' },
            { label: 'Max Drawdown', value: `${formatDecimal(selectedPropSimulationSummary.max_drawdown_r_limit, 0)}R`, tone: 'loss' },
            { label: 'Daily Loss', value: `${formatDecimal(selectedPropSimulationSummary.daily_loss_r_limit, 0)}R`, tone: 'skipped' },
          ],
        },
        {
          title: 'Summary',
          items: [
            {
              label: 'Sim Plays Used',
              value: selectedPropSimulationSummary.sim_plays_used === null || selectedPropSimulationSummary.sim_plays_used === undefined
                ? ''
                : formatNumber(selectedPropSimulationSummary.sim_plays_used),
            },
            { label: 'Pass Rate', value: `${formatDecimal(selectedPropSimulationSummary.pass_rate, 2)}%`, tone: selectedPropSimulationSummary.pass_rate >= 80 ? 'win' : 'loss' },
            { label: 'Closed Pass Rate', value: `${formatDecimal(selectedPropSimulationSummary.closed_pass_rate, 2)}%`, tone: selectedPropSimulationSummary.closed_pass_rate >= 80 ? 'win' : 'loss' },
            { label: 'Max Drawdown', value: `${formatDecimal(selectedPropSimulationSummary.max_drawdown_r, 2)}R`, tone: 'loss' },
            { label: 'Max Loss Streak', value: formatNumber(selectedPropSimulationSummary.max_loss_streak), tone: 'loss' },
          ],
        },
      ]
    : [];
  const selectedSimulationOutcomeTotal = selectedEntryExitSimulationRunRow?.propTotal ?? 0;
  const selectedSimulationOutcomeSegments = selectedEntryExitSimulationRunRow
    ? [
        {
          label: 'Passed',
          value: selectedEntryExitSimulationRunRow.propPassed,
          tone: 'win',
        },
        {
          label: 'Daily Loss',
          value: selectedEntryExitSimulationRunRow.propDailyFails,
          tone: 'warning',
        },
        {
          label: 'Drawdown',
          value: selectedEntryExitSimulationRunRow.propDrawdownFails,
          tone: 'loss',
        },
        {
          label: 'Incomplete',
          value: selectedEntryExitSimulationRunRow.propIncomplete,
          tone: 'neutral',
        },
      ].map((segment) => ({
        ...segment,
        percent: selectedSimulationOutcomeTotal
          ? (Number(segment.value || 0) / selectedSimulationOutcomeTotal) * 100
          : 0,
      }))
    : [];
  const selectedSimulationEquityPoints =
    entryExitSimEquityCurve.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimEquityCurve.points ?? []
      : [];
  const selectedSimulationEquityWidth = 760;
  const selectedSimulationEquityHeight = 220;
  const selectedSimulationEquityPadding = { top: 22, right: 24, bottom: 34, left: 72 };
  const selectedSimulationEquityValues = selectedSimulationEquityPoints.map((point) => Number(point.cumulative_r || 0));
  const selectedSimulationEquityMin = Math.min(0, ...selectedSimulationEquityValues);
  const selectedSimulationEquityMax = Math.max(0, ...selectedSimulationEquityValues);
  const selectedSimulationEquitySpan = Math.max(1, selectedSimulationEquityMax - selectedSimulationEquityMin);
  const selectedSimulationEquityPlotWidth =
    selectedSimulationEquityWidth - selectedSimulationEquityPadding.left - selectedSimulationEquityPadding.right;
  const selectedSimulationEquityPlotHeight =
    selectedSimulationEquityHeight - selectedSimulationEquityPadding.top - selectedSimulationEquityPadding.bottom;
  const getSelectedSimulationEquityX = (index) =>
    selectedSimulationEquityPadding.left +
    (selectedSimulationEquityPoints.length > 1
      ? (index / (selectedSimulationEquityPoints.length - 1)) * selectedSimulationEquityPlotWidth
      : 0);
  const getSelectedSimulationEquityY = (value) =>
    selectedSimulationEquityPadding.top +
    ((selectedSimulationEquityMax - Number(value || 0)) / selectedSimulationEquitySpan) *
      selectedSimulationEquityPlotHeight;
  const selectedSimulationEquityPath = selectedSimulationEquityPoints
    .map((point, index) => {
      const command = index === 0 ? 'M' : 'L';
      return `${command}${getSelectedSimulationEquityX(index).toFixed(2)},${getSelectedSimulationEquityY(point.cumulative_r).toFixed(2)}`;
    })
    .join(' ');
  const selectedSimulationEquityZeroY = getSelectedSimulationEquityY(0);
  const selectedSimulationEquityLast =
    selectedSimulationEquityPoints[selectedSimulationEquityPoints.length - 1] ?? null;
  const selectedSimulationEquityPeak = selectedSimulationEquityValues.length
    ? Math.max(...selectedSimulationEquityValues)
    : 0;
  const selectedSimulationEquityWorstDrawdown = selectedSimulationEquityPoints.length
    ? Math.max(...selectedSimulationEquityPoints.map((point) => Number(point.drawdown_r || 0)))
    : 0;
  const selectedSimulationEquityStartLabel =
    selectedSimulationEquityPoints[0]?.event_date?.slice?.(0, 10) ?? '';
  const selectedSimulationEquityEndLabel =
    selectedSimulationEquityLast?.event_date?.slice?.(0, 10) ?? '';
  const selectedSimulationDrawdownLimit = ENTRY_EXIT_PROP_RULES.maxDrawdownR;
  const selectedSimulationDrawdownCurrent = Number(selectedSimulationEquityLast?.drawdown_r || 0);
  const selectedSimulationDailyRows =
    entryExitSimDailyRData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimDailyRData.days ?? []
      : [];
  const selectedSimulationDailyLossLimit = ENTRY_EXIT_PROP_RULES.dailyLossR;
  const selectedSimulationDailyValues = selectedSimulationDailyRows.map((row) => Number(row.total_r || 0));
  const selectedSimulationDailyDrawdownBarValues = selectedSimulationDailyRows.map((row) =>
    Math.max(0, -Number(row.worst_intraday_r || 0))
  );
  const selectedSimulationBestDay = selectedSimulationDailyRows.reduce(
    (best, row) => (Number(row.total_r || 0) > Number(best?.total_r ?? Number.NEGATIVE_INFINITY) ? row : best),
    null
  );
  const selectedSimulationWorstDay = selectedSimulationDailyRows.reduce(
    (worst, row) => (Number(row.total_r || 0) < Number(worst?.total_r ?? Number.POSITIVE_INFINITY) ? row : worst),
    null
  );
  const selectedSimulationWorstDrawdownDay = selectedSimulationDailyRows.reduce(
    (worst, row) =>
      Math.max(0, -Number(row.worst_intraday_r || 0)) >
      Math.max(0, -Number(worst?.worst_intraday_r ?? 0))
        ? row
        : worst,
    null
  );
  const selectedSimulationDailyLossHitRows = selectedSimulationDailyRows.filter(
    (row) => row.hit_daily_loss || Number(row.worst_intraday_r || 0) <= -selectedSimulationDailyLossLimit
  );
  const selectedSimulationDailyMin = Math.min(
    -selectedSimulationDailyLossLimit,
    0,
    ...selectedSimulationDailyRows.map((row) => Number(row.total_r || 0)),
    ...selectedSimulationDailyRows.map((row) => Number(row.worst_intraday_r || 0))
  );
  const selectedSimulationDailyMax = Math.max(0, ...selectedSimulationDailyValues);
  const selectedSimulationDailySpan = Math.max(1, selectedSimulationDailyMax - selectedSimulationDailyMin);
  const selectedSimulationDailySlotWidth =
    selectedSimulationEquityPlotWidth / Math.max(1, selectedSimulationDailyRows.length);
  const selectedSimulationDailyBarWidth = Math.max(
    2,
    Math.min(14, selectedSimulationDailySlotWidth * 0.64)
  );
  const getSelectedSimulationDailyY = (value) =>
    selectedSimulationEquityPadding.top +
    ((selectedSimulationDailyMax - Number(value || 0)) / selectedSimulationDailySpan) *
      selectedSimulationEquityPlotHeight;
  const getSelectedSimulationDailyX = (index) =>
    selectedSimulationEquityPadding.left +
    index * selectedSimulationDailySlotWidth +
    Math.max(0, (selectedSimulationDailySlotWidth - selectedSimulationDailyBarWidth) / 2);
  const selectedSimulationDailyZeroY = getSelectedSimulationDailyY(0);
  const selectedSimulationDailyLossLimitY = getSelectedSimulationDailyY(-selectedSimulationDailyLossLimit);
  const selectedSimulationDailyDrawdownChartMax =
    Math.max(selectedSimulationDailyLossLimit, ...selectedSimulationDailyDrawdownBarValues, 1) * 1.12;
  const getSelectedSimulationDailyDrawdownY = (value) =>
    selectedSimulationEquityPadding.top +
    (Number(value || 0) / selectedSimulationDailyDrawdownChartMax) * selectedSimulationEquityPlotHeight;
  const selectedSimulationDailyDrawdownZeroY = getSelectedSimulationDailyDrawdownY(0);
  const selectedSimulationDailyDrawdownBarLimitY =
    getSelectedSimulationDailyDrawdownY(selectedSimulationDailyLossLimit);
  const selectedSimulationDailyStartLabel =
    selectedSimulationDailyRows[0]?.trade_date?.slice?.(0, 10) ?? '';
  const selectedSimulationDailyEndLabel =
    selectedSimulationDailyRows[selectedSimulationDailyRows.length - 1]?.trade_date?.slice?.(0, 10) ?? '';
  const selectedSimulationDailyRow =
    selectedSimulationDailyRows.find((row) => String(row.trade_date || '').slice(0, 10) === selectedSimulationDailyDate) ??
    null;
  const selectedSimulationDailyTradeRows =
    entryExitSimDailyTradesData.sim_run_id === selectedEntryExitSimulationTestId &&
    entryExitSimDailyTradesData.trade_date === selectedSimulationDailyDate
      ? entryExitSimDailyTradesData.trades ?? []
      : [];
  const selectedSimulationRawTradeRows =
    entryExitSimRawTradesData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimRawTradesData.trades ?? []
      : [];
  const selectedSimulationRawTradeTotal =
    entryExitSimRawTradesData.sim_run_id === selectedEntryExitSimulationTestId
      ? Number(entryExitSimRawTradesData.total_rows || 0)
      : 0;
  const selectedSimulationDailyTradeNetR = selectedSimulationDailyTradeRows.reduce(
    (sum, trade) => sum + Number(trade.result_r || 0),
    0
  );
  const selectedSimulationHourlyRows =
    entryExitSimHourlyData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimHourlyData.hours ?? []
      : [];
  const selectedSimulationBestHour = selectedSimulationHourlyRows.reduce(
    (best, row) => (Number(row.sum_r || 0) > Number(best?.sum_r ?? Number.NEGATIVE_INFINITY) ? row : best),
    null
  );
  const selectedSimulationWorstHour = selectedSimulationHourlyRows.reduce(
    (worst, row) => (Number(row.sum_r || 0) < Number(worst?.sum_r ?? Number.POSITIVE_INFINITY) ? row : worst),
    null
  );
  const selectedSimulationMostDangerHour = selectedSimulationHourlyRows.reduce(
    (worst, row) =>
      Number(row.daily_loss_day_trades || 0) > Number(worst?.daily_loss_day_trades || 0) ? row : worst,
    null
  );
  const selectedSimulationTradeCadence =
    entryExitSimTradeCadenceData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimTradeCadenceData.cadence
      : null;
  const selectedSimulationTradeGapRows =
    entryExitSimTradeGapData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimTradeGapData.gaps ?? []
      : [];
  const selectedSimulationTradeTimelinePoints = selectedSimulationTradeGapRows.length
    ? [
        {
          event_at: selectedSimulationTradeGapRows[0].previous_event_at,
          gap_minutes: null,
          label: 'First trade',
          cycle_number: selectedSimulationTradeGapRows[0].previous_cycle_number,
          previous_cycle_number: 0,
          starts_new_cycle: false,
          sequence_number: 0,
        },
        ...selectedSimulationTradeGapRows.map((gap) => ({
          event_at: gap.event_at,
          gap_minutes: Number(gap.gap_minutes || 0),
          label: gap.bucket_label || '',
          cycle_number: gap.cycle_number,
          previous_cycle_number: gap.previous_cycle_number,
          starts_new_cycle: Boolean(gap.starts_new_cycle),
          sequence_number: gap.sequence_number,
        })),
      ]
    : [];
  const selectedSimulationTradeGapChart = selectedSimulationTradeGapRows.length
    ? {
        width: 920,
        height: 150,
        padding: { top: 12, right: 10, bottom: 22, left: 34 },
        minTime: Math.min(
          ...selectedSimulationTradeTimelinePoints.map((point) => Date.parse(point.event_at)).filter(Number.isFinite)
        ),
        maxTime: Math.max(
          ...selectedSimulationTradeTimelinePoints.map((point) => Date.parse(point.event_at)).filter(Number.isFinite)
        ),
        ticks: [],
      }
    : null;
  if (selectedSimulationTradeGapChart) {
    const tickCount = 5;
    const timeRange = Math.max(
      selectedSimulationTradeGapChart.maxTime - selectedSimulationTradeGapChart.minTime,
      1
    );
    selectedSimulationTradeGapChart.ticks = Array.from({ length: tickCount }, (_, index) => {
      const ratio = tickCount === 1 ? 0 : index / (tickCount - 1);
      return {
        label: formatTimelineTick(new Date(selectedSimulationTradeGapChart.minTime + timeRange * ratio).toISOString()),
        ratio,
      };
    });
  }
  const selectedSimulationTradeWorkload =
    entryExitSimTradeWorkloadData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimTradeWorkloadData.workload
      : null;
  const selectedSimulationTestFrequencyRows =
    entryExitSimTestFrequencyData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimTestFrequencyData.tests ?? []
      : [];
  const selectedDayTradingSummary =
    entryExitDayTradingSimData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitDayTradingSimData.summary
      : null;
  const selectedSimulationTestFrequencyCompletedRows = selectedSimulationTestFrequencyRows.filter(
    (row) => row.outcome !== 'open_incomplete'
  );
  const selectedSimulationAvgTradesPerTest = selectedSimulationTestFrequencyRows.length
    ? selectedSimulationTestFrequencyRows.reduce((sum, row) => sum + Number(row.trades || 0), 0) /
      selectedSimulationTestFrequencyRows.length
    : 0;
  const selectedSimulationAvgTestDurationMinutes = selectedSimulationTestFrequencyRows.length
    ? selectedSimulationTestFrequencyRows.reduce((sum, row) => sum + Number(row.duration_minutes || 0), 0) /
      selectedSimulationTestFrequencyRows.length
    : 0;
  const selectedSimulationMaxTestTrades = selectedSimulationTestFrequencyRows.reduce(
    (max, row) => Math.max(max, Number(row.trades || 0)),
    0
  );
  const selectedSimulationLongestTestMinutes = selectedSimulationTestFrequencyRows.reduce(
    (max, row) => Math.max(max, Number(row.duration_minutes || 0)),
    0
  );
  const selectedSimulationMarketTrendData =
    entryExitSimMarketTrendData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimMarketTrendData
      : { performance: [], alignment: [], direction_alignment: [] };
  const selectedSimulationMarketTrendPerformanceRows =
    selectedSimulationMarketTrendData.performance ?? [];
  const selectedSimulationMarketTrendAlignmentRows =
    selectedSimulationMarketTrendData.alignment ?? [];
  const selectedSimulationMarketTrendDirectionRows =
    selectedSimulationMarketTrendData.direction_alignment ?? [];
  const selectedSimulationMarketTrendDirectionSummaryRows = ['with_trend', 'against_trend', 'neutral', 'unknown']
    .map((alignmentKey) => {
      const rows = selectedSimulationMarketTrendDirectionRows.filter(
        (row) => row.trend_alignment === alignmentKey
      );
      const trades = rows.reduce((total, row) => total + Number(row.trades || 0), 0);
      const wins = rows.reduce((total, row) => total + Number(row.wins || 0), 0);
      const losses = rows.reduce((total, row) => total + Number(row.losses || 0), 0);
      const noEntries = rows.reduce((total, row) => total + Number(row.no_entries || 0), 0);
      const sumR = rows.reduce((total, row) => total + Number(row.sum_r || 0), 0);
      return {
        trend_alignment: alignmentKey,
        trades,
        wins,
        losses,
        no_entries: noEntries,
        win_rate: trades ? (wins / trades) * 100 : 0,
        avg_r: trades ? sumR / trades : 0,
        sum_r: sumR,
      };
    })
    .filter((row) => row.trades > 0);
  const selectedSimulationBestMarketTrend = selectedSimulationMarketTrendPerformanceRows.reduce(
    (best, row) => (Number(row.sum_r || 0) > Number(best?.sum_r ?? Number.NEGATIVE_INFINITY) ? row : best),
    null
  );
  const selectedSimulationBestMarketTrendAlignment = selectedSimulationMarketTrendAlignmentRows.reduce(
    (best, row) => (Number(row.sum_r || 0) > Number(best?.sum_r ?? Number.NEGATIVE_INFINITY) ? row : best),
    null
  );
  const selectedSimulationTradeCadenceGapCount = Number(
    selectedSimulationTradeCadence?.gap_count ?? 0
  );
  const selectedSimulationTradeCadenceBuckets = selectedSimulationTradeCadence
    ? [
        { label: '0-1m', count: selectedSimulationTradeCadence.gap_0_1m },
        { label: '>1-5m', count: selectedSimulationTradeCadence.gap_1_5m },
        { label: '>5-15m', count: selectedSimulationTradeCadence.gap_5_15m },
        { label: '>15-30m', count: selectedSimulationTradeCadence.gap_15_30m },
        { label: '>30-60m', count: selectedSimulationTradeCadence.gap_30_60m },
        { label: '>60m', count: selectedSimulationTradeCadence.gap_over_60m },
      ].map((bucket) => ({
        ...bucket,
        count: Number(bucket.count || 0),
        percent: selectedSimulationTradeCadenceGapCount
          ? (Number(bucket.count || 0) / selectedSimulationTradeCadenceGapCount) * 100
          : 0,
      }))
    : [];
  const selectedSimulationTradeWorkloadCards = selectedSimulationTradeWorkload
    ? [
        {
          key: 'daily',
          title: 'Daily',
          value: formatDecimal(selectedSimulationTradeWorkload.avg_trades_per_day, 1),
          unit: 'avg trades / day',
          stats: [
            { label: 'Min', value: formatNumber(selectedSimulationTradeWorkload.min_trades_per_day) },
            { label: 'Max', value: formatNumber(selectedSimulationTradeWorkload.max_trades_per_day) },
            { label: 'Trading Days', value: formatNumber(selectedSimulationTradeWorkload.active_days) },
            { label: 'Days > 20', value: formatNumber(selectedSimulationTradeWorkload.days_over_20_trades) },
          ],
        },
        {
          key: 'hourly',
          title: 'Hourly',
          value: formatDecimal(selectedSimulationTradeWorkload.avg_trades_per_hour, 1),
          unit: 'avg trades / active hour',
          stats: [
            { label: 'Max', value: formatNumber(selectedSimulationTradeWorkload.max_trades_per_hour) },
            { label: 'Hours > 5', value: formatNumber(selectedSimulationTradeWorkload.hours_over_5_trades) },
          ],
        },
        {
          key: 'weekly',
          title: 'Weekly',
          value: formatDecimal(selectedSimulationTradeWorkload.avg_trades_per_week, 1),
          unit: 'avg trades / active week',
          stats: [
            { label: 'Max', value: formatNumber(selectedSimulationTradeWorkload.max_trades_per_week) },
            { label: 'Active Weeks', value: formatNumber(selectedSimulationTradeWorkload.active_weeks) },
          ],
        },
        {
          key: 'monthly',
          title: 'Monthly',
          value: formatDecimal(selectedSimulationTradeWorkload.avg_trades_per_month, 1),
          unit: 'avg trades / active month',
          stats: [
            { label: 'Max', value: formatNumber(selectedSimulationTradeWorkload.max_trades_per_month) },
            { label: 'Active Months', value: formatNumber(selectedSimulationTradeWorkload.active_months) },
          ],
        },
      ]
    : [];
  const selectedSimulationLossCluster =
    entryExitSimLossClusterData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimLossClusterData
      : { summary: null, buckets: [], windows: [] };
  const selectedSimulationLossSummary = selectedSimulationLossCluster.summary ?? null;
  const selectedSimulationLossBuckets = selectedSimulationLossCluster.buckets ?? [];
  const selectedSimulationLossWindows = selectedSimulationLossCluster.windows ?? [];
  const selectedSimulationSymbolContributionRows =
    entryExitSimSymbolContributionData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimSymbolContributionData.symbols ?? []
      : [];
  const selectedSimulationFamilyContributionRows =
    entryExitSimFamilyContributionData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimFamilyContributionData.families ?? []
      : [];
  const selectedSimulationFamilyRouteByKey = new Map(
    selectedEntryExitSimulationFamilyRoutes.map((route) => [route.family_key, route])
  );
  const selectedSimulationFamilyPerformanceRows = (
    selectedSimulationFamilyContributionRows.filter((row) => Number(row.sum_r || 0) > 0).length
      ? selectedSimulationFamilyContributionRows.filter((row) => Number(row.sum_r || 0) > 0)
      : selectedSimulationFamilyContributionRows
  )
    .slice(0, 10)
    .map((row, index) => {
      const route = selectedSimulationFamilyRouteByKey.get(row.family_key) ?? null;
      return {
        ...row,
        avgR: Number(row.avg_r || 0),
        label: formatEntryExitFamilyRouteLabel(route, row.family_key),
        netR: Number(row.sum_r || 0),
        rank: index + 1,
        route,
        winRate: Number(row.win_rate || 0),
      };
    });
  const selectedSimulationFamilyPerformanceMaxR = Math.max(
    1,
    ...selectedSimulationFamilyPerformanceRows.map((row) => Math.max(0, Number(row.netR || 0)))
  );
  const selectedSimulationTopSymbol = selectedSimulationSymbolContributionRows[0] ?? null;
  const selectedSimulationWorstSymbol = selectedSimulationSymbolContributionRows.reduce(
    (worst, row) => (Number(row.sum_r || 0) < Number(worst?.sum_r ?? Number.POSITIVE_INFINITY) ? row : worst),
    null
  );
  const selectedSimulationMostDangerSymbol = selectedSimulationSymbolContributionRows.reduce(
    (worst, row) =>
      Number(row.daily_loss_day_trades || 0) > Number(worst?.daily_loss_day_trades || 0) ? row : worst,
    null
  );
  const selectedSimulationTopFamily = selectedSimulationFamilyContributionRows[0] ?? null;
  const selectedSimulationWorstFamily = selectedSimulationFamilyContributionRows.reduce(
    (worst, row) => (Number(row.sum_r || 0) < Number(worst?.sum_r ?? Number.POSITIVE_INFINITY) ? row : worst),
    null
  );
  const selectedSimulationMostDangerFamily = selectedSimulationFamilyContributionRows.reduce(
    (worst, row) =>
      Number(row.daily_loss_day_trades || 0) > Number(worst?.daily_loss_day_trades || 0) ? row : worst,
    null
  );
  const selectedSimulationStreakRows =
    entryExitSimStreakData.sim_run_id === selectedEntryExitSimulationTestId
      ? entryExitSimStreakData.streaks ?? []
      : [];
  const selectedSimulationWinStreakRows = selectedSimulationStreakRows.filter(
    (row) => row.streak_type === 'win'
  );
  const selectedSimulationLossStreakRows = selectedSimulationStreakRows.filter(
    (row) => row.streak_type === 'loss'
  );
  const selectedSimulationLargestWinStreak = selectedSimulationWinStreakRows.reduce(
    (best, row) => (Number(row.streak_length || 0) > Number(best?.streak_length || 0) ? row : best),
    null
  );
  const selectedSimulationLargestLossStreak = selectedSimulationLossStreakRows.reduce(
    (best, row) => (Number(row.streak_length || 0) > Number(best?.streak_length || 0) ? row : best),
    null
  );
  const selectedSimulationMaxStreakLength = Math.max(
    1,
    ...selectedSimulationStreakRows.map((row) => Number(row.streak_length || 0))
  );
  const selectedSimulationStreakDistribution = Array.from(
    { length: selectedSimulationMaxStreakLength },
    (_, index) => {
      const length = index + 1;
      return {
        length,
        wins: selectedSimulationWinStreakRows.filter((row) => Number(row.streak_length || 0) === length).length,
        losses: selectedSimulationLossStreakRows.filter((row) => Number(row.streak_length || 0) === length).length,
      };
    }
  ).filter((row) => row.wins || row.losses);
  const selectedSimulationMaxStreakBucketCount = Math.max(
    1,
    ...selectedSimulationStreakDistribution.flatMap((row) => [row.wins, row.losses])
  );
  const selectedEntryExitRouterPropSections = [
    {
      title: 'Prop Firm Rule Logic',
      items: selectedEntryExitRouterRunRow
        ? [
            {
              label: 'Pass Target',
              value: `+${formatDecimal(ENTRY_EXIT_PROP_RULES.profitTargetR, 0)}R`,
              tone: 'win',
            },
            {
              label: selectedEntryExitLogicRunRow?.run.daily_loss_lockout ? 'Daily Lockout' : 'Daily Loss Fail',
              value: `-${formatDecimal(ENTRY_EXIT_PROP_RULES.dailyLossR, 0)}R`,
              tone: selectedEntryExitLogicRunRow?.run.daily_loss_lockout ? 'skipped' : 'loss',
            },
            {
              label: 'Max Drawdown Fail',
              value: `${formatDecimal(ENTRY_EXIT_PROP_RULES.maxDrawdownR, 0)}R from peak`,
              tone: 'loss',
            },
            {
              label: 'Cycle Logic',
              value: selectedEntryExitLogicRunRow?.run.daily_loss_lockout
                ? 'Chronological routed trades reset after pass or max drawdown'
                : 'Chronological routed trades reset after pass or fail',
              wide: true,
            },
            {
              label: selectedEntryExitLogicRunRow?.run.daily_loss_lockout ? 'Daily Trade Lock' : 'Daily Reset',
              value: selectedEntryExitLogicRunRow?.run.daily_loss_lockout
                ? 'After daily limit, the playbook skips the rest of that trade date'
                : 'Daily R resets when the trade date changes',
              wide: true,
            },
            {
              label: 'Near Pass Protect',
              value: selectedEntryExitLogicRunRow?.run.near_pass_protection
                ? `After the cycle gets within ${formatDecimal(selectedEntryExitLogicRunRow.run.near_pass_within_r, 0)}R of target, daily lockout tightens to -${formatDecimal(selectedEntryExitLogicRunRow.run.near_pass_daily_loss_r, 0)}R`
                : 'Off',
              tone: selectedEntryExitLogicRunRow?.run.near_pass_protection ? 'skipped' : '',
              wide: true,
            },
            {
              label: 'Loss Cluster Guard',
              value: selectedEntryExitLogicRunRow?.run.loss_cluster_day_lockout
                ? `${formatNumber(selectedEntryExitLogicRunRow.run.loss_cluster_loss_count || 3)} losses inside ${formatNumber(selectedEntryExitLogicRunRow.run.loss_cluster_window_minutes || 60)} minutes locks the rest of that trade date`
                : 'Off',
              tone: selectedEntryExitLogicRunRow?.run.loss_cluster_day_lockout ? 'skipped' : '',
              wide: true,
            },
            {
              label: 'Trade Feed',
              value: 'Only routed playbook trades are replayed into prop cycles',
              wide: true,
            },
          ]
        : [],
    },
    {
      title: 'Current Manual Ban List',
      items: [
        {
          label: 'Current Families',
          value: entryExitManualFamilyBans.length
            ? `${formatNumber(entryExitManualFamilyBans.length)} banned`
            : 'None',
          tone: entryExitManualFamilyBans.length ? 'loss' : 'win',
        },
        {
          label: 'Current Symbols',
          value: entryExitManualSymbolBans.length
            ? `${formatNumber(entryExitManualSymbolBans.length)} banned`
            : 'None',
          tone: entryExitManualSymbolBans.length ? 'loss' : 'win',
        },
        {
          label: 'Applied To Selected Run',
          value: selectedEntryExitManualFamilyBansApplied
            ? `${formatNumber(selectedEntryExitManualFamilyBansApplied)} family ban applied`
            : '0 applied',
          tone: selectedEntryExitManualFamilyBansApplied ? 'skipped' : currentManualBanTotal ? 'loss' : 'win',
          wide: true,
        },
        {
          label: 'Run Note',
          value: currentManualBanTotal
            ? selectedEntryExitManualFamilyBansApplied
              ? 'This selected run used the manual family ban list'
              : 'This selected run did not use the current manual family bans'
            : 'No current manual bans configured',
          wide: true,
        },
        {
          label: 'Purpose',
          value: 'Current bans are user decisions; selected-run applied count is historical',
          wide: true,
        },
        ...entryExitManualBanItems.slice(0, 8),
      ],
    },
    {
      title: 'Playbook Decision Logic',
      items: selectedEntryExitRouterRunRow
        ? [
            {
              label: 'Selected Playbook',
              value: `${selectedEntryExitRouterRunRow.playbookLabel} | ${selectedEntryExitRouterRunRow.playbookName}`,
              wide: true,
            },
            {
              label: 'Strategy Description',
              value: selectedEntryExitPlaybookDescription,
              title: selectedEntryExitPlaybookDescription,
              wide: true,
              narrative: true,
            },
            {
              label: 'Family Gate',
              value: `${formatNumber(selectedEntryExitRouterRunRow.run.trade_choices)} trade | ${formatNumber(selectedEntryExitRouterRunRow.run.watchlist_choices)} watch | ${formatNumber(selectedEntryExitRouterRunRow.run.skip_choices)} skip`,
              wide: true,
            },
            {
              label: 'Manual Ban Status',
              value: `${formatNumber(entryExitManualFamilyBans.length)} current | ${formatNumber(selectedEntryExitManualFamilyBansApplied)} applied to this run`,
              tone: entryExitManualFamilyBans.length || selectedEntryExitManualFamilyBansApplied ? 'skipped' : 'win',
              wide: true,
            },
            {
              label: 'Symbol Gate',
              value: selectedEntryExitRouterRunRow.run.symbol_filter_enabled
                ? `${formatNumber(selectedEntryExitRouterRunRow.run.symbol_trade_roots)} roots allowed | ${formatNumber(selectedEntryExitRouterRunRow.run.symbol_skip_roots)} roots skipped`
                : 'Off',
              tone: selectedEntryExitRouterRunRow.run.symbol_filter_enabled ? 'win' : 'skipped',
              wide: true,
            },
            {
              label: 'Skipped By Symbol',
              value: `${formatNumber(selectedEntryExitRouterRunRow.run.skipped_symbol_patterns)} patterns`,
              tone: selectedEntryExitRouterRunRow.run.skipped_symbol_patterns ? 'skipped' : 'win',
            },
            {
              label: 'Twin Rule',
              value: `One chosen candidate per twin event | ${formatNumber(selectedEntryExitRouterRunRow.run.sister_window_minutes)} min window`,
              wide: true,
            },
            {
              label: 'Overlap Rule',
              value: selectedEntryExitOverlapRule,
              tone: selectedEntryExitRouterRunRow.run.one_trade_at_a_time ||
                selectedEntryExitRouterRunRow.run.one_trade_per_root_symbol ||
                selectedEntryExitRouterRunRow.run.one_trade_per_minute ||
                Number(selectedEntryExitRouterRunRow.run.trade_cooldown_minutes || 0) > 0
                ? 'skipped'
                : 'win',
              wide: true,
            },
            {
              label: 'Near Pass Rule',
              value: selectedEntryExitRouterRunRow.run.near_pass_protection
                ? `Within ${formatDecimal(selectedEntryExitRouterRunRow.run.near_pass_within_r, 0)}R | -${formatDecimal(selectedEntryExitRouterRunRow.run.near_pass_daily_loss_r, 0)}R daily lock`
                : 'Off',
              tone: selectedEntryExitRouterRunRow.run.near_pass_protection ? 'skipped' : 'win',
              wide: true,
            },
            {
              label: 'Loss Cluster Guard',
              value: selectedEntryExitRouterRunRow.run.loss_cluster_day_lockout
                ? `${formatNumber(selectedEntryExitRouterRunRow.run.loss_cluster_loss_count || 3)} losses inside ${formatNumber(selectedEntryExitRouterRunRow.run.loss_cluster_window_minutes || 60)}m = stop that day`
                : 'Off',
              tone: selectedEntryExitRouterRunRow.run.loss_cluster_day_lockout ? 'skipped' : 'win',
              wide: true,
            },
            {
              label: 'Skipped By Overlap',
              value: `${formatNumber(selectedEntryExitRouterRunRow.run.skipped_overlap_patterns)} patterns`,
              tone: selectedEntryExitRouterRunRow.run.skipped_overlap_patterns ? 'skipped' : 'win',
            },
            {
              label: 'Watch / Skip Behavior',
              value: 'WATCHLIST and SKIP family rows are not eligible for replay trades',
              wide: true,
            },
            ...selectedEntryExitManualSkipRows.slice(0, 3).map((row) => ({
              label: `${row.route.harmonic_type} ${row.route.market}`,
              value: `${row.route.family_bin} | ${row.route.family_size_bucket} | ${row.route.family_time_bin} | ${row.route.family_x_strictness}`,
              title: `${row.route.family_key} | ${row.route.status_reason}`,
              tone: 'skipped',
              wide: true,
            })),
          ]
        : [],
    },
    {
      title: 'Selected Simulation Test',
      items: selectedEntryExitRouterRunRow
        ? [
            {
              label: 'Playbook',
              value: selectedEntryExitRouterRunRow.playbookLabel,
              tone: 'win',
            },
            {
              label: 'Playbook Setup',
              value: selectedEntryExitRouterRunRow.playbookName,
              wide: true,
            },
            {
              label: 'Simulation Test ID',
              value: compactText(selectedEntryExitRouterRunRow.run.router_run_id, 28),
              title: selectedEntryExitRouterRunRow.run.router_run_id,
              wide: true,
            },
            { label: 'Test Year', value: selectedEntryExitRouterRunRow.run.test_year || 'All' },
            { label: 'Prop Total', value: formatNumber(selectedEntryExitRouterRunRow.propTotal) },
            { label: 'Closed Tests', value: formatNumber(selectedEntryExitRouterRunRow.propClosedTotal) },
            { label: 'Passed', value: formatNumber(selectedEntryExitRouterRunRow.propPassed), tone: 'win' },
            { label: 'Failed', value: formatNumber(selectedEntryExitRouterRunRow.propFailed), tone: 'loss' },
            {
              label: 'Closed Pass WR',
              value: `${formatDecimal(selectedEntryExitRouterRunRow.propClosedPassRate, 2)}%`,
              tone: Number(selectedEntryExitRouterRunRow.propClosedPassRate) >= 80 ? 'win' : 'skipped',
            },
            {
              label: 'All-Cycle Pass WR',
              value: `${formatDecimal(selectedEntryExitRouterRunRow.run.prop?.pass_rate || 0, 2)}%`,
            },
            {
              label: 'Daily Loss Fails',
              value: formatNumber(selectedEntryExitRouterRunRow.propDailyFails),
              tone: 'loss',
            },
            {
              label: 'Drawdown Fails',
              value: formatNumber(selectedEntryExitRouterRunRow.propDrawdownFails),
              tone: 'loss',
            },
            { label: 'Open / Incomplete', value: formatNumber(selectedEntryExitRouterRunRow.propIncomplete) },
            {
              label: 'Manual Bans Applied',
              value: `${formatNumber(selectedEntryExitManualFamilyBansApplied)} family`,
              tone: selectedEntryExitManualFamilyBansApplied ? 'skipped' : 'win',
            },
            {
              label: 'Max Loss Streak',
              value: formatNumber(selectedEntryExitRouterRunRow.run.prop?.max_loss_streak || 0),
              tone: 'skipped',
            },
            {
              label: 'Max Drawdown Seen',
              value: `${formatDecimal(selectedEntryExitRouterRunRow.run.prop?.max_drawdown_r || 0, 2)}R`,
              tone: 'loss',
            },
          ]
        : [],
    },
    {
      title: 'Simulation Filters',
      items: selectedEntryExitRouterRunRow
        ? [
            {
              label: 'Prop Filter',
              value: selectedEntryExitRouterRunRow.run.prop_filter_enabled ? 'On' : 'Off',
              tone: selectedEntryExitRouterRunRow.run.prop_filter_enabled ? 'win' : 'skipped',
            },
            {
              label: 'Trade Bar',
              value: `${formatNumber(selectedEntryExitRouterRunRow.run.trade_min_tests)} tests | ${formatDecimal(Number(selectedEntryExitRouterRunRow.run.trade_min_win_rate || 0) * 100, 1)}% WR | ${formatDecimal(selectedEntryExitRouterRunRow.run.trade_min_avg_r, 3)}R`,
              wide: true,
            },
            {
              label: 'Watchlist Bar',
              value: `${formatNumber(selectedEntryExitRouterRunRow.run.watchlist_min_tests)} tests | ${formatDecimal(Number(selectedEntryExitRouterRunRow.run.watchlist_min_win_rate || 0) * 100, 1)}% WR | ${formatDecimal(selectedEntryExitRouterRunRow.run.watchlist_min_avg_r, 3)}R`,
              wide: true,
            },
            {
              label: 'Symbol Filter',
              value: selectedEntryExitRouterRunRow.run.symbol_filter_enabled ? 'On' : 'Off',
              tone: selectedEntryExitRouterRunRow.run.symbol_filter_enabled ? 'win' : 'skipped',
            },
            {
              label: 'Symbol Gate',
              value: `${formatNumber(selectedEntryExitRouterRunRow.run.symbol_min_tests)} tests | ${formatDecimal(Number(selectedEntryExitRouterRunRow.run.symbol_min_win_rate || 0) * 100, 1)}% WR | ${formatDecimal(selectedEntryExitRouterRunRow.run.symbol_min_avg_r, 3)}R`,
              wide: true,
            },
            {
              label: 'Twin Window',
              value: `${formatNumber(selectedEntryExitRouterRunRow.run.sister_window_minutes)} min`,
            },
          ]
        : [],
    },
    {
      title: 'Replay Scope',
      items: selectedEntryExitRouterRunRow
        ? [
            {
              label: 'Train Run',
              value: compactText(selectedEntryExitRouterRunRow.run.train_run_id, 28),
              title: selectedEntryExitRouterRunRow.run.train_run_id,
              wide: true,
            },
            { label: 'Patterns Scanned', value: formatNumber(selectedEntryExitRouterRunRow.run.patterns_scanned) },
            { label: 'Routed Trades', value: formatNumber(selectedEntryExitRouterRunRow.tradeCount), tone: 'win' },
            { label: 'Trade WR', value: `${formatDecimal(selectedEntryExitRouterRunRow.winRate, 2)}%` },
            {
              label: 'Avg R',
              value: `${formatDecimal(selectedEntryExitRouterRunRow.run.avg_r, 4)}R`,
              tone: Number(selectedEntryExitRouterRunRow.run.avg_r) < 0 ? 'loss' : 'win',
            },
            {
              label: 'Total R',
              value: `${formatDecimal(selectedEntryExitRouterRunRow.run.sum_r, 2)}R`,
              tone: Number(selectedEntryExitRouterRunRow.run.sum_r) < 0 ? 'loss' : 'win',
            },
            { label: 'No Route', value: formatNumber(selectedEntryExitRouterRunRow.run.no_route_patterns) },
            { label: 'Skipped Non-Trade', value: formatNumber(selectedEntryExitRouterRunRow.run.skipped_non_trade_patterns) },
            { label: 'Skipped Symbol', value: formatNumber(selectedEntryExitRouterRunRow.run.skipped_symbol_patterns), tone: 'skipped' },
            { label: 'Skipped Overlap', value: formatNumber(selectedEntryExitRouterRunRow.run.skipped_overlap_patterns), tone: selectedEntryExitRouterRunRow.run.skipped_overlap_patterns ? 'skipped' : 'win' },
            {
              label: 'Family Choices',
              value: `${formatNumber(selectedEntryExitRouterRunRow.run.trade_choices)} trade | ${formatNumber(selectedEntryExitRouterRunRow.run.watchlist_choices)} watch | ${formatNumber(selectedEntryExitRouterRunRow.run.skip_choices)} skip`,
              wide: true,
            },
            {
              label: 'Symbol Choices',
              value: `${formatNumber(selectedEntryExitRouterRunRow.run.symbol_trade_roots)} trade | ${formatNumber(selectedEntryExitRouterRunRow.run.symbol_skip_roots)} skip`,
              wide: true,
            },
          ]
        : [],
    },
    {
      title: 'Symbol Filter',
      items: selectedEntryExitRouterRunRow
        ? [
            {
              label: 'Allowed Roots',
              value: formatNumber(selectedEntryExitRouterRunRow.run.symbol_trade_roots),
              tone: 'win',
            },
            {
              label: 'Skipped Roots',
              value: formatNumber(selectedEntryExitRouterRunRow.run.symbol_skip_roots),
              tone: selectedEntryExitRouterRunRow.run.symbol_skip_roots ? 'loss' : 'win',
            },
            ...entryExitRouterSymbolGateRows.map((symbol) => ({
              label: `${symbol.root_symbol} / ${symbol.route_status}`,
              value: `${formatNumber(symbol.train_eval_count)} tests | ${formatRatePercent(symbol.train_win_rate, 1)}% WR | ${formatDecimal(symbol.train_avg_r, 3)}R`,
              title: symbol.status_reason,
              tone: symbol.route_status === 'SKIP' ? 'loss' : 'win',
              wide: true,
            })),
          ]
        : [],
    },
  ];
  const entryExitTemplateSections = [
    {
      title: 'Template Library Source',
      items: entryExitRun
        ? [
            { label: 'Template Build ID', value: compactText(entryExitRun.run_id, 24), title: entryExitRun.run_id, compact: true, wide: true },
            { label: 'Patterns', value: formatNumber(entryExitRun.scanned_patterns) },
            { label: 'Templates', value: formatNumber(entryExitRun.templates_created), tone: 'win' },
            { label: 'Prior Passes', value: formatNumber(entryExitRun.existing_template_passes), tone: 'win' },
            { label: 'Failed Create', value: formatNumber(entryExitRun.failed_to_create), tone: entryExitRun.failed_to_create ? 'loss' : '' },
            { label: 'Results', value: formatNumber(entryExitRun.result_rows) },
            { label: 'Scope', value: `${formatRouteMode(entryExitRun.source_scope)} / ${entryExitRun.period_year || 'All'}` },
            { label: 'Elapsed', value: `${formatDecimal((entryExitRun.elapsed_ms || 0) / 1000, 2)}s` },
            {
              label: 'Best Template',
              value: entryExitBestTemplate
                ? `${getEntryExitTemplateLabel(entryExitBestTemplate)} | ${formatDecimal(entryExitBestTemplate.avg_r, 2)}R`
                : 'N/A',
              compact: true,
              tone: entryExitBestTemplate && Number(entryExitBestTemplate.avg_r) < 0 ? 'loss' : 'win',
              wide: true,
            },
          ]
        : [],
    },
    {
      title: `Selected Template ${selectedEntryExitTemplateLabel}`,
      items: selectedEntryExitTemplate
        ? [
            {
              label: 'Template ID',
              value: compactText(selectedEntryExitTemplate.template_uid, 30),
              title: selectedEntryExitTemplate.template_uid,
              compact: true,
              wide: true,
            },
            {
              label: 'Rule Name',
              value: selectedEntryExitTemplate.template_name,
              title: selectedEntryExitTemplate.template_name,
              wide: true,
            },
            { label: 'Rule', value: formatEntryExitTemplateRule(selectedEntryExitTemplate), wide: true },
            {
              label: 'Direction',
              value: selectedEntryExitTemplate.direction_mode === 'inverse_pattern' ? 'Inverse Pattern' : 'Pattern Direction',
            },
            {
              label: 'Entry',
              value: selectedEntryExitEntryOffset
                ? `Confirmation +${selectedEntryExitEntryOffset} open`
                : formatRouteMode(selectedEntryExitTemplate.entry_kind),
              wide: true,
            },
            { label: 'Risk', value: `${formatDecimal(selectedEntryExitTemplate.risk_multiple, 3)} ${getEntryExitTemplateRiskLabel(selectedEntryExitTemplate)}` },
            { label: 'Target', value: `${formatDecimal(selectedEntryExitTemplate.target_r, 2)}R`, tone: 'win' },
            { label: 'Hold', value: `${selectedEntryExitTemplate.max_hold_multiple || 0}x pattern` },
            { label: 'Evaluations', value: formatNumber(selectedEntryExitTemplate.eval_count) },
            { label: 'Passes', value: formatNumber(selectedEntryExitTemplate.pass_count), tone: 'win' },
            { label: 'Fails', value: formatNumber(selectedEntryExitTemplate.fail_count), tone: 'loss' },
            { label: 'No Entry', value: formatNumber(selectedEntryExitTemplate.no_entry_count), tone: 'skipped' },
            { label: 'Pass Rate', value: `${formatDecimal(selectedEntryExitPassRate, 1)}%`, tone: 'win' },
            { label: 'Fail Rate', value: `${formatDecimal(selectedEntryExitFailRate, 1)}%`, tone: 'loss' },
            { label: 'No Entry Rate', value: `${formatDecimal(selectedEntryExitNoEntryRate, 1)}%`, tone: 'skipped' },
            {
              label: 'Avg R',
              value: `${formatDecimal(selectedEntryExitTemplate.avg_r, 3)}R`,
              tone: Number(selectedEntryExitTemplate.avg_r) < 0 ? 'loss' : 'win',
            },
            { label: 'Origin Symbol', value: selectedEntryExitTemplate.created_from_symbol || 'N/A' },
            { label: 'Origin Market', value: selectedEntryExitTemplate.created_from_market || 'N/A' },
            {
              label: 'Origin Family',
              value: compactText(selectedEntryExitTemplate.created_from_family_key || 'N/A', 24),
              title: selectedEntryExitTemplate.created_from_family_key,
              compact: true,
            },
            { label: 'Origin D Confirm', value: formatDate(selectedEntryExitTemplate.created_from_d_confirm_date), wide: true },
            {
              label: 'Origin Setup',
              value: compactText(selectedEntryExitTemplate.created_from_setup_id || 'N/A', 28),
              title: selectedEntryExitTemplate.created_from_setup_id,
              compact: true,
              wide: true,
            },
            {
              label: 'Origin Pattern',
              value: compactText(selectedEntryExitTemplate.created_from_pattern_id || 'N/A', 28),
              title: selectedEntryExitTemplate.created_from_pattern_id,
              compact: true,
              wide: true,
            },
          ]
        : [],
    },
    {
      title: 'Pattern Market Split',
      items: isEntryExitTemplateBreakdownLoading
        ? [
            {
              label: 'Loading',
              value: 'Fetching Bullish / Bearish split...',
              wide: true,
            },
          ]
        : entryExitTemplateBreakdownError
        ? [
            {
              label: 'Error',
              value: entryExitTemplateBreakdownError,
              tone: 'loss',
              wide: true,
            },
          ]
        : selectedEntryExitMarketBreakdown.map((row) => {
            const evalCount = Number(row.eval_count || 0);
            const passRate = evalCount ? (Number(row.pass_count || 0) / evalCount) * 100 : 0;
            const failRate = evalCount ? (Number(row.fail_count || 0) / evalCount) * 100 : 0;
            const noEntryRate = evalCount ? (Number(row.no_entry_count || 0) / evalCount) * 100 : 0;
            return {
              label: `${row.market || 'Unknown'} | ${formatNumber(evalCount)} tests`,
              value: `${formatDecimal(passRate, 1)}% WR / ${formatDecimal(row.avg_r, 3)}R`,
              title: `Pass ${formatNumber(row.pass_count)} / Fail ${formatNumber(row.fail_count)} / No Entry ${formatNumber(row.no_entry_count)} | Fail ${formatDecimal(failRate, 1)}% | No Entry ${formatDecimal(noEntryRate, 1)}% | Best ${formatDecimal(row.best_r, 2)}R | Worst ${formatDecimal(row.worst_r, 2)}R`,
              tone: Number(row.avg_r) < 0 ? 'loss' : 'win',
              wide: true,
            };
          }),
    },
    {
      title: 'Harmonic Type Split',
      items: isEntryExitTemplateBreakdownLoading
        ? [
            {
              label: 'Loading',
              value: 'Fetching harmonic type split...',
              wide: true,
            },
          ]
        : entryExitTemplateBreakdownError
        ? [
            {
              label: 'Error',
              value: entryExitTemplateBreakdownError,
              tone: 'loss',
              wide: true,
            },
          ]
        : selectedEntryExitHarmonicBreakdown.map((row) => {
            const evalCount = Number(row.eval_count || 0);
            const passRate = evalCount ? (Number(row.pass_count || 0) / evalCount) * 100 : 0;
            return {
              label: `${row.harmonic_type || 'Unknown'} | ${formatNumber(evalCount)} tests`,
              value: `${formatDecimal(passRate, 1)}% WR / ${formatDecimal(row.avg_r, 3)}R`,
              title: `Pass ${formatNumber(row.pass_count)} / Fail ${formatNumber(row.fail_count)} / No Entry ${formatNumber(row.no_entry_count)} | Best ${formatDecimal(row.best_r, 2)}R | Worst ${formatDecimal(row.worst_r, 2)}R`,
              tone: Number(row.avg_r) < 0 ? 'loss' : 'win',
              wide: true,
            };
          }),
    },
    {
      title: `Generated Templates (${formatNumber(displayedEntryExitTemplates.length)})`,
      items: [],
      tableRows: generatedEntryExitTemplateRows,
      variant: 'templateTable',
      wide: true,
    },
  ];
  const entryExitTemplateWinSections = [
    {
      title: `Selected Template Wins ${selectedEntryExitTemplateLabel}`,
      items: selectedEntryExitTemplate
        ? [
            { label: 'Passes', value: formatNumber(selectedEntryExitTemplate.pass_count), tone: 'win' },
            { label: 'Pass Rate', value: `${formatDecimal(selectedEntryExitPassRate, 1)}%`, tone: 'win' },
            { label: 'Bull Wins', value: formatNumber(selectedEntryExitBullishPassCount), tone: 'win' },
            { label: 'Bear Wins', value: formatNumber(selectedEntryExitBearishPassCount), tone: 'win' },
            {
              label: 'Top Win Market',
              value: selectedEntryExitTopWinMarket
                ? `${selectedEntryExitTopWinMarket.market || 'Unknown'} / ${formatNumber(selectedEntryExitTopWinMarket.pass_count)} wins`
                : 'N/A',
              tone: 'win',
              wide: true,
            },
            {
              label: 'Top Harmonic Type',
              value: selectedEntryExitTopWinHarmonic
                ? `${selectedEntryExitTopWinHarmonic.harmonic_type || 'Unknown'} / ${formatNumber(selectedEntryExitTopWinHarmonic.pass_count)} wins`
                : 'N/A',
              tone: 'win',
              wide: true,
            },
            {
              label: 'Rule',
              value: formatEntryExitTemplateRule(selectedEntryExitTemplate),
              wide: true,
            },
            { label: 'Target', value: `${formatDecimal(selectedEntryExitTemplate.target_r, 2)}R`, tone: 'win' },
            { label: 'Risk', value: `${formatDecimal(selectedEntryExitTemplate.risk_multiple, 3)} ${getEntryExitTemplateRiskLabel(selectedEntryExitTemplate)}` },
          ]
        : [],
    },
    {
      title: 'Win Market Split',
      items: isEntryExitTemplateBreakdownLoading
        ? [{ label: 'Loading', value: 'Fetching win split...', wide: true }]
        : selectedEntryExitMarketBreakdown.map((row) => {
            const evalCount = Number(row.eval_count || 0);
            const passCount = Number(row.pass_count || 0);
            const passRate = evalCount ? (passCount / evalCount) * 100 : 0;
            return {
              label: `${row.market || 'Unknown'} Wins`,
              value: `${formatNumber(passCount)} / ${formatDecimal(passRate, 1)}% WR / ${formatDecimal(row.avg_r, 3)}R`,
              title: `Evaluated ${formatNumber(evalCount)} / Best ${formatDecimal(row.best_r, 2)}R`,
              tone: 'win',
              wide: true,
            };
          }),
    },
    {
      title: 'Harmonic Type Wins',
      items: isEntryExitTemplateBreakdownLoading
        ? [{ label: 'Loading', value: 'Fetching harmonic wins...', wide: true }]
        : entryExitTemplateBreakdownError
        ? [{ label: 'Error', value: entryExitTemplateBreakdownError, tone: 'loss', wide: true }]
        : [...selectedEntryExitHarmonicBreakdown]
          .filter((row) => Number(row.pass_count || 0) > 0)
          .sort((left, right) =>
            Number(right.pass_count || 0) - Number(left.pass_count || 0) ||
            Number(right.avg_r || 0) - Number(left.avg_r || 0)
          )
          .map((row) => {
            const evalCount = Number(row.eval_count || 0);
            const passCount = Number(row.pass_count || 0);
            const passRate = evalCount ? (passCount / evalCount) * 100 : 0;
            return {
              label: row.harmonic_type || 'Unknown',
              value: `${formatNumber(passCount)} wins / ${formatDecimal(passRate, 1)}% of tests / ${formatDecimal(row.avg_r, 3)}R`,
              title: `Evaluated ${formatNumber(evalCount)} / Fail ${formatNumber(row.fail_count)} / No Entry ${formatNumber(row.no_entry_count)} / Best ${formatDecimal(row.best_r, 2)}R / Worst ${formatDecimal(row.worst_r, 2)}R`,
              tone: 'win',
              wide: true,
            };
          }),
    },
  ];
  const entryExitTemplateLossSections = [
    {
      title: `Selected Template Losses ${selectedEntryExitTemplateLabel}`,
      items: selectedEntryExitTemplate
        ? [
            { label: 'Fails', value: formatNumber(selectedEntryExitTemplate.fail_count), tone: 'loss' },
            { label: 'Fail Rate', value: `${formatDecimal(selectedEntryExitFailRate, 1)}%`, tone: 'loss' },
            { label: 'Bull Fails', value: formatNumber(selectedEntryExitBullishFailCount), tone: 'loss' },
            { label: 'Bear Fails', value: formatNumber(selectedEntryExitBearishFailCount), tone: 'loss' },
            { label: 'No Entry', value: formatNumber(selectedEntryExitTemplate.no_entry_count), tone: 'skipped' },
            { label: 'No Entry Rate', value: `${formatDecimal(selectedEntryExitNoEntryRate, 1)}%`, tone: 'skipped' },
            {
              label: 'Top Loss Market',
              value: selectedEntryExitTopLossMarket
                ? `${selectedEntryExitTopLossMarket.market || 'Unknown'} / ${formatNumber(selectedEntryExitTopLossMarket.fail_count)} fails`
                : 'N/A',
              tone: 'loss',
              wide: true,
            },
            {
              label: 'Top Loss Harmonic',
              value: selectedEntryExitTopLossHarmonic
                ? `${selectedEntryExitTopLossHarmonic.harmonic_type || 'Unknown'} / ${formatNumber(selectedEntryExitTopLossHarmonic.fail_count)} fails`
                : 'N/A',
              tone: 'loss',
              wide: true,
            },
            {
              label: 'Worst Market R',
              value: selectedEntryExitTopLossMarket
                ? `${formatDecimal(selectedEntryExitTopLossMarket.avg_r, 3)}R avg`
                : 'N/A',
              tone: 'loss',
            },
          ]
        : [],
    },
    {
      title: 'Loss Market Split',
      items: isEntryExitTemplateBreakdownLoading
        ? [{ label: 'Loading', value: 'Fetching loss split...', wide: true }]
        : selectedEntryExitMarketBreakdown.map((row) => {
            const evalCount = Number(row.eval_count || 0);
            const failCount = Number(row.fail_count || 0);
            const failRate = evalCount ? (failCount / evalCount) * 100 : 0;
            return {
              label: `${row.market || 'Unknown'} Losses`,
              value: `${formatNumber(failCount)} / ${formatDecimal(failRate, 1)}% fail / ${formatDecimal(row.worst_r, 2)}R worst`,
              title: `Pass ${formatNumber(row.pass_count)} / No Entry ${formatNumber(row.no_entry_count)} / Avg ${formatDecimal(row.avg_r, 3)}R`,
              tone: 'loss',
              wide: true,
            };
          }),
    },
    {
      title: 'Loss Harmonic Split',
      items: isEntryExitTemplateBreakdownLoading
        ? [{ label: 'Loading', value: 'Fetching harmonic losses...', wide: true }]
        : selectedEntryExitHarmonicBreakdown.map((row) => {
            const evalCount = Number(row.eval_count || 0);
            const failCount = Number(row.fail_count || 0);
            const failRate = evalCount ? (failCount / evalCount) * 100 : 0;
            return {
              label: `${row.harmonic_type || 'Unknown'} Losses`,
              value: `${formatNumber(failCount)} / ${formatDecimal(failRate, 1)}% fail / ${formatDecimal(row.worst_r, 2)}R worst`,
              title: `Pass ${formatNumber(row.pass_count)} / No Entry ${formatNumber(row.no_entry_count)} / Avg ${formatDecimal(row.avg_r, 3)}R`,
              tone: 'loss',
              wide: true,
            };
      }),
    },
  ];
  const entryExitTemplateFamilySections = [
    {
      title: `Winning Families ${selectedEntryExitTemplateLabel}`,
      items: selectedEntryExitTemplate
        ? [
            {
              label: 'Families Won',
              value: formatNumber(selectedEntryExitWinningFamilyRows.length),
              tone: 'win',
            },
            {
              label: 'Family Wins',
              value: formatNumber(selectedEntryExitFamilyTotals.wins),
              tone: 'win',
            },
            {
              label: 'Family Losses',
              value: formatNumber(selectedEntryExitFamilyTotals.losses),
              tone: 'loss',
            },
            {
              label: 'No Entry',
              value: formatNumber(selectedEntryExitFamilyTotals.noEntries),
              tone: 'skipped',
            },
            {
              label: 'Rows',
              value: `${formatNumber(selectedEntryExitFamilyTotals.tests)} tests`,
              wide: true,
            },
          ]
        : [],
    },
    {
      title: 'Family Win / Loss Counts',
      items: isEntryExitTemplateBreakdownLoading
        ? [{ label: 'Loading', value: 'Fetching winning family counts...', wide: true }]
        : entryExitTemplateBreakdownError
        ? [{ label: 'Error', value: entryExitTemplateBreakdownError, tone: 'loss', wide: true }]
        : selectedEntryExitWinningFamilyRows.map((row) => {
            const evalCount = Number(row.eval_count || 0);
            const passCount = Number(row.pass_count || 0);
            const failCount = Number(row.fail_count || 0);
            const noEntryCount = Number(row.no_entry_count || 0);
            const passRate = evalCount ? (passCount / evalCount) * 100 : 0;
            const familyLabel = [
              row.harmonic_type || 'Unknown',
              row.market || 'Market N/A',
              row.family_bin || 'Bin N/A',
              row.family_size_bucket || 'Size N/A',
              row.family_time_bin || 'Time N/A',
              compactText(row.family_key || 'N/A', 10),
            ]
              .filter(Boolean)
              .join(' / ');

            return {
              label: familyLabel,
              value: `${formatNumber(passCount)}W / ${formatNumber(failCount)}L / ${formatNumber(noEntryCount)}NE / ${formatNumber(evalCount)} tests / ${formatDecimal(passRate, 1)}% WR / ${formatDecimal(row.avg_r, 3)}R`,
              title: `${row.family_key} | Bin ${row.family_bin || 'N/A'} | X ${row.family_x_strictness || 'N/A'} | Best ${formatDecimal(row.best_r, 2)}R | Worst ${formatDecimal(row.worst_r, 2)}R`,
              tone: Number(row.avg_r) < 0 ? 'loss' : 'win',
              wide: true,
            };
          }),
    },
  ];
  const entryExitTemplateEdgeSections = [
    {
      title: `Edge Baseline ${selectedEntryExitTemplateLabel}`,
      items: selectedEntryExitTemplate
        ? [
            { label: 'Template WR', value: `${formatDecimal(selectedEntryExitPassRate, 1)}%` },
            {
              label: 'Template Avg R',
              value: `${formatDecimal(selectedEntryExitTemplate.avg_r, 3)}R`,
              tone: Number(selectedEntryExitTemplate.avg_r) < 0 ? 'loss' : 'win',
            },
            { label: 'Min Sample', value: '50 tests' },
            { label: 'Rule', value: formatEntryExitTemplateRule(selectedEntryExitTemplate), wide: true },
            {
              label: 'Combo Test',
              value: 'Beats template baseline and both single-feature parents',
              wide: true,
            },
          ]
        : [],
    },
    {
      title: 'Single Feature Edge',
      items: isEntryExitTemplateBreakdownLoading
        ? [{ label: 'Loading', value: 'Finding single-feature lifts...', wide: true }]
        : entryExitTemplateBreakdownError
        ? [{ label: 'Error', value: entryExitTemplateBreakdownError, tone: 'loss', wide: true }]
        : selectedEntryExitTopSingleEdges.map((row) => ({
            label: formatEntryExitFeatureLabel(row.condition_type, row.condition_value),
            value: `${formatNumber(row.evalCount)} tests / ${formatDecimal(row.passRate, 1)}% WR / ${formatDecimal(row.avgR, 3)}R / ${formatDecimal(row.avgRLift, 3)}R lift / ${formatDecimal(row.wrLift, 1)}% WR lift / ${row.edgeClass}`,
            title: `${formatNumber(row.passCount)} wins / ${formatNumber(row.fail_count)} losses / Best ${formatDecimal(row.best_r, 2)}R / Worst ${formatDecimal(row.worst_r, 2)}R`,
            tone: row.edgeClass === 'Thin Sample' ? 'skipped' : row.avgRLift > 0 ? 'win' : 'loss',
            wide: true,
          })),
    },
    {
      title: 'Two Feature Combo Edge',
      items: isEntryExitTemplateBreakdownLoading
        ? [{ label: 'Loading', value: 'Building combo result families...', wide: true }]
        : entryExitTemplateBreakdownError
        ? [{ label: 'Error', value: entryExitTemplateBreakdownError, tone: 'loss', wide: true }]
        : selectedEntryExitTrueComboEdges.map((row) => ({
            label: `${formatEntryExitFeatureLabel(row.feature_a_type, row.feature_a_value)} + ${formatEntryExitFeatureLabel(row.feature_b_type, row.feature_b_value)}`,
            value: `${formatNumber(row.evalCount)} tests / ${formatDecimal(row.passRate, 1)}% WR / ${formatDecimal(row.avgR, 3)}R / ${formatDecimal(row.avgRLift, 3)}R vs base / ${formatDecimal(row.avgRLiftVsParents, 3)}R vs singles / ${row.edgeClass}`,
            title: `${formatNumber(row.passCount)} wins / ${formatNumber(row.fail_count)} losses / ${formatDecimal(row.wrLift, 1)}% WR lift / ${formatDecimal(row.wrLiftVsParents, 1)}% WR vs singles`,
            tone: row.edgeClass === 'Thin Sample' ? 'skipped' : 'win',
            wide: true,
          })),
    },
    {
      title: 'Baseline Edge Only',
      items: selectedEntryExitBaselineOnlyCombos.map((row) => ({
        label: `${formatEntryExitFeatureLabel(row.feature_a_type, row.feature_a_value)} + ${formatEntryExitFeatureLabel(row.feature_b_type, row.feature_b_value)}`,
        value: `${formatNumber(row.evalCount)} tests / ${formatDecimal(row.passRate, 1)}% WR / ${formatDecimal(row.avgR, 3)}R beats base / ${formatDecimal(row.avgRLiftVsParents, 3)}R vs singles / ${row.edgeClass}`,
        title: `${formatNumber(row.passCount)} wins / this combo is not stronger than its best parent feature`,
        tone: 'skipped',
        wide: true,
      })),
    },
    {
      title: 'Combo Avoids',
      items: selectedEntryExitAvoidCombos.map((row) => ({
        label: `${formatEntryExitFeatureLabel(row.feature_a_type, row.feature_a_value)} + ${formatEntryExitFeatureLabel(row.feature_b_type, row.feature_b_value)}`,
        value: `${formatDecimal(row.avgR, 3)}R / ${formatDecimal(row.avgRLift, 3)}R below base / ${formatNumber(row.evalCount)} tests`,
        title: `${formatNumber(row.fail_count)} fails / ${formatDecimal(row.passRate, 1)}% WR / Worst ${formatDecimal(row.worst_r, 2)}R`,
        tone: 'loss',
        wide: true,
      })),
    },
  ].filter((section) => section.items.length);
  const entryExitTemplateDetailSections = entryExitTemplateSections.filter(
    (section) => section.title !== 'Template Library Source' && section.variant !== 'templateTable'
  );
  // Dormant while the data dashboard is rebuilt from a blank slate.
  void entryExitProfileTab;
  void entryExitConditionSectionsForMode;
  void selectedEntryExitRouterPropSections;
  void entryExitTemplateWinSections;
  void entryExitTemplateLossSections;
  void entryExitTemplateFamilySections;
  void entryExitTemplateEdgeSections;
  void entryExitTemplateDetailSections;
  const patternLibrarySections = [
    {
      title: 'AI Model Overview',
      wide: true,
      items: patternAiStage1OverviewItems,
    },
    {
      title: 'AI Exit Model Overview',
      wide: true,
      items: patternAiExitModelOverviewItems,
    },
    {
      title: 'AI Exit Main 180-Bar Changed Trades',
      variant: 'patternAiExitModelTradeTable',
      wide: true,
      tableRows: patternAiExitModelTradeRows,
    },
    {
      title: 'AI 2026 Taken Trades - 180 Exit',
      variant: 'patternAiStage1TradeTable',
      wide: true,
      tableRows: patternAiExitModelTakenTradeRows,
    },
    {
      title: 'XA Reversal Overview',
      wide: true,
      items: patternXaOutcomeRun
        ? [
            {
              label: 'Run',
              value: `${compactText(patternXaOutcomeRun.run_id, 18)} | ${patternXaOutcomeRun.scan_year_label}`,
              wide: true,
            },
            {
              label: 'Reversal XA',
              value: `${formatNumber(patternXaOutcomeRun.reversal_count)} / ${formatDecimal(patternXaOutcomeRate(patternXaOutcomeRun.reversal_count), 2)}%`,
              tone: 'win',
            },
            {
              label: 'Continuation XA',
              value: `${formatNumber(patternXaOutcomeRun.continuation_count)} / ${formatDecimal(patternXaOutcomeRate(patternXaOutcomeRun.continuation_count), 2)}%`,
              tone: 'loss',
            },
            {
              label: 'No Hit',
              value: `${formatNumber(patternXaOutcomeRun.none_count)} / ${formatDecimal(patternXaOutcomeRate(patternXaOutcomeRun.none_count), 2)}%`,
              tone: 'skipped',
            },
            {
              label: 'Ambiguous',
              value: `${formatNumber(patternXaOutcomeRun.ambiguous_count)} / ${formatDecimal(patternXaOutcomeRate(patternXaOutcomeRun.ambiguous_count), 2)}%`,
            },
          ]
        : [
            {
              label: isPatternXaOutcomeLoading ? 'Loading' : 'XA Outcomes',
              value: patternXaOutcomeError || (isPatternXaOutcomeLoading ? 'Loading reversal rows...' : 'No outcome run loaded.'),
              wide: true,
              tone: patternXaOutcomeError ? 'loss' : 'skipped',
            },
          ],
    },
    {
      title: 'XA Reversal Families',
      variant: 'patternXaFamilyTable',
      wide: true,
      tableRows: patternXaOutcomeFamilyRows,
    },
    {
      title: 'AI Reversal Overview',
      wide: true,
      items: patternReversalAiRun
        ? [
            {
              label: 'AI Run',
              value: `${compactText(patternReversalAiRun.ai_run_id, 18)} | ${compactText(patternReversalAiRun.source_xa_run_id, 18)}`,
              title: `${patternReversalAiRun.ai_run_id} | source ${patternReversalAiRun.source_xa_run_id}`,
              wide: true,
            },
            { label: 'Model', value: formatRouteMode(patternReversalAiRun.model_type), wide: true },
            { label: 'Threshold', value: `${formatDecimal(patternReversalAiRun.decision_threshold * 100, 1)}%` },
            { label: 'Scored', value: formatNumber(patternReversalAiRun.score_rows) },
            {
              label: 'AI Said Reverse',
              value: formatNumber(patternReversalAiRun.predicted_reversal_count),
              tone: 'win',
            },
            {
              label: 'Actually Reversed',
              value: formatNumber(patternReversalAiRun.predicted_reversal_actual_reversal_count),
              tone: 'win',
            },
            {
              label: 'AI Reverse Hit %',
              value: `${formatDecimal(patternReversalAiRun.predicted_reversal_actual_rate * 100, 2)}%`,
              tone: 'win',
            },
            {
              label: 'Baseline',
              value: `${formatDecimal(patternReversalAiRun.baseline_reversal_rate * 100, 2)}%`,
            },
            {
              label: 'Lift',
              value: `${formatDecimal(patternReversalAiRun.lift_vs_baseline * 100, 2)}pp`,
              tone: Number(patternReversalAiRun.lift_vs_baseline || 0) >= 0 ? 'win' : 'loss',
            },
          ]
        : [
            {
              label: isPatternReversalAiLoading ? 'Loading' : 'AI Scores',
              value: patternReversalAiError || (isPatternReversalAiLoading ? 'Loading AI score rows...' : 'No AI score run loaded.'),
              wide: true,
              tone: patternReversalAiError ? 'loss' : 'skipped',
            },
          ],
    },
    {
      title: 'AI Threshold Tests',
      variant: 'patternReversalAiThresholds',
      wide: true,
      tableRows: patternReversalAiThresholdRows,
    },
    {
      title: 'AI Reversal Buckets',
      variant: 'patternReversalAiBuckets',
      wide: true,
      tableRows: patternReversalAiBucketRows,
    },
    {
      title: 'AI Reversal Pattern Rows',
      variant: 'patternReversalAiTable',
      wide: true,
      tableRows: patternReversalAiRows,
    },
    {
      title: 'XA Reversal Pattern Rows',
      variant: 'patternXaOutcomeTable',
      wide: true,
      tableRows: patternXaOutcomeRows,
    },
  ];
  void selectedTestSymbolSections;
  void selectedFamilyOverviewSections;
  void selectedRouteFamiliesSections;
  void supplyOverviewSections;
  const activeTestOverviewSections = patternLibrarySections;

  useEffect(() => {
    if (testOverviewTab !== 'patterns') {
      setTestOverviewTab('patterns');
    }
  }, [testOverviewTab]);

  useEffect(() => {
    if (testOverviewTab !== 'patterns' || patternXaOutcomeRows.length || isPatternXaOutcomeLoading) {
      return;
    }

    void loadPatternXaOutcomes({ offset: 0, append: false });
  }, [
    isPatternXaOutcomeLoading,
    loadPatternXaOutcomes,
    patternXaOutcomeRows.length,
    testOverviewTab,
  ]);
  useEffect(() => {
    if (testOverviewTab !== 'patterns' || patternReversalAiRows.length || isPatternReversalAiLoading) {
      return;
    }

    void loadPatternReversalAiScores({ offset: 0, append: false });
  }, [
    isPatternReversalAiLoading,
    loadPatternReversalAiScores,
    patternReversalAiRows.length,
    testOverviewTab,
  ]);
  useEffect(() => {
    const shouldLoadAiExitTrades =
      testOverviewTab === 'patterns' ||
      (testOverviewTab === 'simTesting' && entryExitSimulationTab === 'aiTrades');
    if (
      !shouldLoadAiExitTrades ||
      isPatternAiExitModelTradeLoading ||
      (isPatternAiExitModelMainLoaded && patternAiExitModelTradeRows.length)
    ) {
      return;
    }

    void loadPatternAiExitModelTrades({ offset: 0, append: false });
  }, [
    entryExitSimulationTab,
    isPatternAiExitModelMainLoaded,
    isPatternAiExitModelTradeLoading,
    loadPatternAiExitModelTrades,
    patternAiExitModelTradeRows.length,
    testOverviewTab,
  ]);

  useEffect(() => {
    const shouldLoadAiExitTakenTrades =
      testOverviewTab === 'patterns' ||
      (testOverviewTab === 'simTesting' && entryExitSimulationTab === 'aiTrades');
    if (
      !shouldLoadAiExitTakenTrades ||
      isPatternAiExitModelTakenTradeLoading ||
      (isPatternAiExitModelTakenMainLoaded && patternAiExitModelTakenTradeRows.length)
    ) {
      return;
    }

    void loadPatternAiExitModelTakenTrades({ offset: 0, append: false });
  }, [
    entryExitSimulationTab,
    isPatternAiExitModelTakenMainLoaded,
    isPatternAiExitModelTakenTradeLoading,
    loadPatternAiExitModelTakenTrades,
    patternAiExitModelTakenTradeRows.length,
    testOverviewTab,
  ]);

  useEffect(() => {
    const shouldLoadAiStage1Trades =
      testOverviewTab === 'patterns' ||
      (testOverviewTab === 'simTesting' && entryExitSimulationTab === 'aiTrades');
    if (
      !shouldLoadAiStage1Trades ||
      isPatternAiStage1TradeLoading ||
      (isPatternAiStage1LatestRunLoaded && patternAiStage1TradeRows.length)
    ) {
      return;
    }

    void loadPatternAiStage1Trades({ offset: 0, append: false });
  }, [
    entryExitSimulationTab,
    isPatternAiStage1LatestRunLoaded,
    isPatternAiStage1TradeLoading,
    loadPatternAiStage1Trades,
    patternAiStage1TradeRows.length,
    testOverviewTab,
  ]);

  useEffect(() => {
    if (initialFamilyKey) {
      setSelectedFamilyKey(initialFamilyKey);
    }
  }, [initialFamilyKey]);

  useEffect(() => {
    if (!isPatternCardHovered) {
      return undefined;
    }

    const handlePatternCardKeyDown = (event) => {
      if (
        browsePanel ||
        event.defaultPrevented ||
        event.altKey ||
        event.ctrlKey ||
        event.metaKey ||
        event.shiftKey
      ) {
        return;
      }

      const target = event.target;
      const tagName = target?.tagName?.toLowerCase();
      if (target?.isContentEditable || ['input', 'select', 'textarea'].includes(tagName)) {
        return;
      }

      if (event.key === 'ArrowDown') {
        event.preventDefault();
        moveSelectedPattern(1);
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault();
        moveSelectedPattern(-1);
      }
    };

    window.addEventListener('keydown', handlePatternCardKeyDown);
    return () => {
      window.removeEventListener('keydown', handlePatternCardKeyDown);
    };
  }, [browsePanel, isPatternCardHovered, moveSelectedPattern]);

  useEffect(() => {
    if (isLoading) return;
    if (!selectedFamilyKey) return;

    const selectedStillVisible = visibleFamilies.some(
      (family) => family.family_key === selectedFamilyKey
    );
    if (!selectedStillVisible) {
      setSelectedFamilyKey(null);
    }
  }, [isLoading, selectedFamilyKey, visibleFamilies]);

  useEffect(() => {
    if (ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) {
      setFamilyPatterns([]);
      setFamilyPatternsError('');
      setFamilyPatternsLoading(false);
      return undefined;
    }

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
            sourceTimeframe: timeframeFilter === 'All' ? null : timeframeFilter,
            year: yearFilter === 'All' ? null : yearFilter,
          },
          { limit: 10000, offset: 0, includeCount: true }
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
  }, [isEntryExitStandalone, selectedFamilyKey, sourceScope, timeframeFilter, yearFilter]);

  useEffect(() => {
    setPatternBrowseSymbol('All');
    setPatternBrowseSearch('');
    setPatternBrowseRows([]);
    setPatternBrowseMeta({ totalCount: 0, hasMore: false });
    setAppliedPatternNavigation(null);
  }, [patternBrowseScope, sourceScope, timeframeFilter, yearFilter]);

  useEffect(() => {
    if (browsePanel !== 'patterns') {
      return undefined;
    }

    if (patternBrowseScope === 'selected' && !selectedFamilyKey) {
      setPatternBrowseRows([]);
      setPatternBrowseMeta({ totalCount: 0, hasMore: false });
      setPatternBrowseError('');
      setPatternBrowseLoading(false);
      return undefined;
    }

    let isCancelled = false;

    const loadBrowsePatterns = async () => {
      try {
        setPatternBrowseLoading(true);
        setPatternBrowseError('');
        const isSelectedFamilyScope = patternBrowseScope === 'selected';
        const result = await fetchPhase1FamilyPatterns(
          {
            familyKey: isSelectedFamilyScope ? selectedFamilyKey : null,
            allFamilies: !isSelectedFamilyScope,
            sourceScope,
            sourceTimeframe: timeframeFilter === 'All' ? null : timeframeFilter,
            symbol: patternBrowseSymbol === 'All' ? null : patternBrowseSymbol,
            year: yearFilter === 'All' ? null : yearFilter,
          },
          {
            limit: isSelectedFamilyScope ? 10000 : 5000,
            offset: 0,
            includeCount: true,
          }
        );

        if (!isCancelled) {
          setPatternBrowseRows(result.patterns ?? []);
          setPatternBrowseMeta({
            totalCount: result.total_count ?? result.patterns?.length ?? 0,
            hasMore: Boolean(result.has_more),
          });
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setPatternBrowseError('Could not load browse patterns.');
          setPatternBrowseRows([]);
          setPatternBrowseMeta({ totalCount: 0, hasMore: false });
        }
      } finally {
        if (!isCancelled) {
          setPatternBrowseLoading(false);
        }
      }
    };

    void loadBrowsePatterns();

    return () => {
      isCancelled = true;
    };
  }, [browsePanel, patternBrowseScope, patternBrowseSymbol, selectedFamilyKey, sourceScope, timeframeFilter, yearFilter]);

  useEffect(() => {
    const patternsForSelectedFamily = familyPatterns.filter((pattern) => {
      const patternFamilyKey = getPatternFamilyKey(pattern);
      return !selectedFamilyKey || !patternFamilyKey || patternFamilyKey === selectedFamilyKey;
    });
    const selectedPatternStillVisible = [...patternsForSelectedFamily, ...patternBrowseRows].some((pattern) => {
      const patternFamilyKey = getPatternFamilyKey(pattern);
      return (
        getFamilyPatternKey(pattern) === selectedFamilyPatternKey &&
        (!selectedFamilyKey || !patternFamilyKey || patternFamilyKey === selectedFamilyKey)
      );
    });

    if (!selectedPatternStillVisible) {
      setSelectedFamilyPatternKey(
        patternsForSelectedFamily[0] ? getFamilyPatternKey(patternsForSelectedFamily[0]) : null
      );
    }
  }, [familyPatterns, patternBrowseRows, selectedFamilyKey, selectedFamilyPatternKey]);

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
    if (ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) {
      setPhase1Results([]);
      setPhase1Error('');
      setPhase1Loading(false);
      return undefined;
    }

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
  }, [isEntryExitStandalone, selectedFamilyKey, sourceScope, yearFilter]);

  useEffect(() => {
    const selectedRouteStillVisible = phase1Results.some(
      (result) => getRouteKey(result) === selectedRouteKey
    );

    if (!selectedRouteStillVisible) {
      setSelectedRouteKey(phase1Results[0] ? getRouteKey(phase1Results[0]) : null);
    }
  }, [phase1Results, selectedRouteKey]);

  useEffect(() => {
    if (ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) {
      setSupplyData({ symbols: [], families: [] });
      setSupplyError('');
      setSupplyLoading(false);
      return undefined;
    }

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
  }, [isEntryExitStandalone, sourceScope, yearFilter]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitTemplates = async () => {
      if (!selectedBuildRunId) {
        setEntryExitLoading(false);
        setEntryExitError('');
        setEntryExitData({ run: null, build_summary: null, templates: [], coverage: [] });
        setEntryExitRouterData({
          current_run: null,
          runs: [],
          symbols: [],
          family_routes: [],
          template_performance: [],
          manual_family_bans: [],
          manual_symbol_bans: [],
        });
        setSelectedEntryExitRouterRunId(null);
        return;
      }

      try {
        setEntryExitLoading(true);
        setEntryExitError('');
        setEntryExitData({ run: null, build_summary: null, templates: [], coverage: [] });
        setEntryExitRouterData({
          current_run: null,
          runs: [],
          symbols: [],
          family_routes: [],
          template_performance: [],
          manual_family_bans: [],
          manual_symbol_bans: [],
        });
        setSelectedEntryExitRouterRunId(null);
        const data = await fetchEntryExitTemplates({
          runId: selectedBuildRunId,
          sourceScope,
          year: yearFilter === 'All' ? null : yearFilter,
          limit: 250,
        });

        if (!isCancelled) {
          setEntryExitData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitError('Could not load Entry / Exit build dashboard.');
          setEntryExitData({ run: null, build_summary: null, templates: [], coverage: [] });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitLoading(false);
        }
      }
    };

    void loadEntryExitTemplates();

    return () => {
      isCancelled = true;
    };
  }, [
    isEntryExitStandalone,
    selectedBuildRunId,
    sourceScope,
    yearFilter,
  ]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitRouterRuns = async () => {
      if (!entryExitData.run?.run_id) {
        setEntryExitRouterData({
          current_run: null,
          runs: [],
          symbols: [],
          family_routes: [],
          template_performance: [],
          manual_family_bans: [],
          manual_symbol_bans: [],
        });
        setSelectedEntryExitRouterRunId(null);
        setEntryExitRouterError('');
        setEntryExitRouterLoading(false);
        return;
      }

      try {
        setEntryExitRouterLoading(true);
        setEntryExitRouterError('');
        const data = await fetchEntryExitRouterRuns({
          trainRunId: entryExitData.run.run_id,
          limit: 8,
        });

        if (!isCancelled) {
          setEntryExitRouterData(data);
          setSelectedEntryExitRouterRunId((currentRunId) => {
            const loadedRuns = data?.runs ?? [];
            const stillLoaded = currentRunId && loadedRuns.some((run) => run.router_run_id === currentRunId);
            if (stillLoaded) {
              return currentRunId;
            }

            return data?.current_run?.router_run_id ?? loadedRuns[0]?.router_run_id ?? null;
          });
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitRouterError('Could not load stored playbooks.');
          setEntryExitRouterData({
            current_run: null,
            runs: [],
            symbols: [],
            family_routes: [],
            template_performance: [],
            manual_family_bans: [],
            manual_symbol_bans: [],
          });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitRouterLoading(false);
        }
      }
    };

    void loadEntryExitRouterRuns();

    return () => {
      isCancelled = true;
    };
  }, [entryExitData.run?.run_id, entryExitRouterRefreshKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimEquityCurve = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimEquityCurve({ sim_run_id: '', points: [] });
        setEntryExitSimEquityError('');
        setEntryExitSimEquityLoading(false);
        return;
      }

      try {
        setEntryExitSimEquityLoading(true);
        setEntryExitSimEquityError('');
        const data = await fetchEntryExitSimEquityCurve({
          simRunId: selectedEntryExitSimulationTestId,
          pointLimit: 5000,
        });

        if (!isCancelled) {
          setEntryExitSimEquityCurve(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimEquityError('Could not load equity curve.');
          setEntryExitSimEquityCurve({ sim_run_id: selectedEntryExitSimulationTestId, points: [] });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimEquityLoading(false);
        }
      }
    };

    void loadEntryExitSimEquityCurve();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId, entryExitRouterRefreshKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitDayTradingSim = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitDayTradingSimData({ sim_run_id: '', summary: null, daily: [], monthly: [] });
        setEntryExitDayTradingSimError('');
        setEntryExitDayTradingSimLoading(false);
        return;
      }

      try {
        setEntryExitDayTradingSimLoading(true);
        setEntryExitDayTradingSimError('');
        const data = await fetchEntryExitDayTradingSim({
          simRunId: selectedEntryExitSimulationTestId,
        });

        if (!isCancelled) {
          setEntryExitDayTradingSimData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitDayTradingSimError('Could not load day trading simulation.');
          setEntryExitDayTradingSimData({
            sim_run_id: selectedEntryExitSimulationTestId,
            summary: null,
            daily: [],
            monthly: [],
          });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitDayTradingSimLoading(false);
        }
      }
    };

    void loadEntryExitDayTradingSim();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId, entryExitRouterRefreshKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimContributions = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimSymbolContributionData({ sim_run_id: '', symbols: [] });
        setEntryExitSimFamilyContributionData({ sim_run_id: '', families: [] });
        setEntryExitSimSymbolContributionError('');
        setEntryExitSimFamilyContributionError('');
        setEntryExitSimSymbolContributionLoading(false);
        setEntryExitSimFamilyContributionLoading(false);
        return;
      }

      try {
        setEntryExitSimSymbolContributionLoading(true);
        setEntryExitSimFamilyContributionLoading(true);
        setEntryExitSimSymbolContributionError('');
        setEntryExitSimFamilyContributionError('');
        const [symbolData, familyData] = await Promise.all([
          fetchEntryExitSimSymbolContribution({
            simRunId: selectedEntryExitSimulationTestId,
            limit: 160,
          }),
          fetchEntryExitSimFamilyContribution({
            simRunId: selectedEntryExitSimulationTestId,
            limit: 160,
          }),
        ]);

        if (!isCancelled) {
          setEntryExitSimSymbolContributionData(symbolData);
          setEntryExitSimFamilyContributionData(familyData);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimSymbolContributionError('Could not load symbol contribution.');
          setEntryExitSimFamilyContributionError('Could not load family contribution.');
          setEntryExitSimSymbolContributionData({ sim_run_id: selectedEntryExitSimulationTestId, symbols: [] });
          setEntryExitSimFamilyContributionData({ sim_run_id: selectedEntryExitSimulationTestId, families: [] });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimSymbolContributionLoading(false);
          setEntryExitSimFamilyContributionLoading(false);
        }
      }
    };

    void loadEntryExitSimContributions();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId, entryExitRouterRefreshKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimDailyR = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimDailyRData({ sim_run_id: '', days: [] });
        setEntryExitSimDailyRError('');
        setEntryExitSimDailyRLoading(false);
        return;
      }

      try {
        setEntryExitSimDailyRLoading(true);
        setEntryExitSimDailyRError('');
        const data = await fetchEntryExitSimDailyR({
          simRunId: selectedEntryExitSimulationTestId,
        });

        if (!isCancelled) {
          setEntryExitSimDailyRData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimDailyRError('Could not load daily R bars.');
          setEntryExitSimDailyRData({ sim_run_id: selectedEntryExitSimulationTestId, days: [] });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimDailyRLoading(false);
        }
      }
    };

    void loadEntryExitSimDailyR();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId]);

  useEffect(() => {
    setSelectedSimulationDailyDate('');
    setSelectedSimulationRawTradeKey(null);
    setSelectedSimulationRawTrade(null);
    setEntryExitSimDailyTradesData({ sim_run_id: '', trade_date: '', trades: [] });
    setEntryExitSimDailyTradesError('');
    setEntryExitSimDailyTradesLoading(false);
  }, [selectedEntryExitSimulationTestId]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimDailyTrades = async () => {
      if (!selectedEntryExitSimulationTestId || !selectedSimulationDailyDate) {
        setEntryExitSimDailyTradesData({ sim_run_id: '', trade_date: '', trades: [] });
        setEntryExitSimDailyTradesError('');
        setEntryExitSimDailyTradesLoading(false);
        return;
      }

      try {
        setEntryExitSimDailyTradesLoading(true);
        setEntryExitSimDailyTradesError('');
        const data = await fetchEntryExitSimDailyTrades({
          simRunId: selectedEntryExitSimulationTestId,
          tradeDate: selectedSimulationDailyDate,
        });

        if (!isCancelled) {
          setEntryExitSimDailyTradesData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimDailyTradesError('Could not load daily trades.');
          setEntryExitSimDailyTradesData({
            sim_run_id: selectedEntryExitSimulationTestId,
            trade_date: selectedSimulationDailyDate,
            trades: [],
          });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimDailyTradesLoading(false);
        }
      }
    };

    void loadEntryExitSimDailyTrades();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId, selectedSimulationDailyDate]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimRawTrades = async () => {
      if (!selectedEntryExitSimulationTestId || entryExitSimulationTab !== 'rawTrades') {
        setEntryExitSimRawTradesData({
          sim_run_id: '',
          total_rows: 0,
          limit: 500,
          offset: 0,
          trades: [],
        });
        setEntryExitSimRawTradesError('');
        setEntryExitSimRawTradesLoading(false);
        return;
      }

      try {
        setEntryExitSimRawTradesLoading(true);
        setEntryExitSimRawTradesError('');
        const data = await fetchEntryExitSimRawTrades({
          simRunId: selectedEntryExitSimulationTestId,
          limit: 500,
          offset: 0,
        });

        if (!isCancelled) {
          setEntryExitSimRawTradesData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimRawTradesError('Could not load raw simulation trades.');
          setEntryExitSimRawTradesData({
            sim_run_id: selectedEntryExitSimulationTestId,
            total_rows: 0,
            limit: 500,
            offset: 0,
            trades: [],
          });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimRawTradesLoading(false);
        }
      }
    };

    void loadEntryExitSimRawTrades();

    return () => {
      isCancelled = true;
    };
  }, [entryExitSimulationTab, selectedEntryExitSimulationTestId]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimHourly = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimHourlyData({ sim_run_id: '', hours: [] });
        setEntryExitSimHourlyError('');
        setEntryExitSimHourlyLoading(false);
        return;
      }

      try {
        setEntryExitSimHourlyLoading(true);
        setEntryExitSimHourlyError('');
        const data = await fetchEntryExitSimHourly({
          simRunId: selectedEntryExitSimulationTestId,
        });

        if (!isCancelled) {
          setEntryExitSimHourlyData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimHourlyError('Could not load time-of-day performance.');
          setEntryExitSimHourlyData({ sim_run_id: selectedEntryExitSimulationTestId, hours: [] });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimHourlyLoading(false);
        }
      }
    };

    void loadEntryExitSimHourly();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimTradeCadence = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimTradeCadenceData({ sim_run_id: '', cadence: null });
        setEntryExitSimTradeCadenceError('');
        setEntryExitSimTradeCadenceLoading(false);
        return;
      }

      try {
        setEntryExitSimTradeCadenceLoading(true);
        setEntryExitSimTradeCadenceError('');
        const data = await fetchEntryExitSimTradeCadence({
          simRunId: selectedEntryExitSimulationTestId,
        });

        if (!isCancelled) {
          setEntryExitSimTradeCadenceData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimTradeCadenceError('Could not load trade cadence.');
          setEntryExitSimTradeCadenceData({ sim_run_id: selectedEntryExitSimulationTestId, cadence: null });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimTradeCadenceLoading(false);
        }
      }
    };

    void loadEntryExitSimTradeCadence();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimTradeGaps = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimTradeGapData({ sim_run_id: '', gaps: [] });
        setEntryExitSimTradeGapError('');
        setEntryExitSimTradeGapLoading(false);
        return;
      }

      try {
        setEntryExitSimTradeGapLoading(true);
        setEntryExitSimTradeGapError('');
        const data = await fetchEntryExitSimTradeGaps({
          simRunId: selectedEntryExitSimulationTestId,
          limit: 5000,
        });

        if (!isCancelled) {
          setEntryExitSimTradeGapData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimTradeGapError('Could not load trade gap timeline.');
          setEntryExitSimTradeGapData({ sim_run_id: selectedEntryExitSimulationTestId, gaps: [] });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimTradeGapLoading(false);
        }
      }
    };

    void loadEntryExitSimTradeGaps();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId, entryExitRouterRefreshKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimTradeWorkload = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimTradeWorkloadData({ sim_run_id: '', workload: null });
        setEntryExitSimTradeWorkloadError('');
        setEntryExitSimTradeWorkloadLoading(false);
        return;
      }

      try {
        setEntryExitSimTradeWorkloadLoading(true);
        setEntryExitSimTradeWorkloadError('');
        const data = await fetchEntryExitSimTradeWorkload({
          simRunId: selectedEntryExitSimulationTestId,
        });

        if (!isCancelled) {
          setEntryExitSimTradeWorkloadData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimTradeWorkloadError('Could not load trade workload.');
          setEntryExitSimTradeWorkloadData({ sim_run_id: selectedEntryExitSimulationTestId, workload: null });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimTradeWorkloadLoading(false);
        }
      }
    };

    void loadEntryExitSimTradeWorkload();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId, entryExitRouterRefreshKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimTestFrequency = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimTestFrequencyData({ sim_run_id: '', tests: [] });
        setEntryExitSimTestFrequencyError('');
        setEntryExitSimTestFrequencyLoading(false);
        return;
      }

      try {
        setEntryExitSimTestFrequencyLoading(true);
        setEntryExitSimTestFrequencyError('');
        const data = await fetchEntryExitSimTestFrequency({
          simRunId: selectedEntryExitSimulationTestId,
          limit: 500,
        });

        if (!isCancelled) {
          setEntryExitSimTestFrequencyData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimTestFrequencyError('Could not load test frequency.');
          setEntryExitSimTestFrequencyData({ sim_run_id: selectedEntryExitSimulationTestId, tests: [] });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimTestFrequencyLoading(false);
        }
      }
    };

    void loadEntryExitSimTestFrequency();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId, entryExitRouterRefreshKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimMarketTrends = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimMarketTrendData({ sim_run_id: '', performance: [], alignment: [], direction_alignment: [] });
        setEntryExitSimMarketTrendError('');
        setEntryExitSimMarketTrendLoading(false);
        return;
      }

      try {
        setEntryExitSimMarketTrendLoading(true);
        setEntryExitSimMarketTrendError('');
        const data = await fetchEntryExitSimMarketTrends({
          simRunId: selectedEntryExitSimulationTestId,
        });

        if (!isCancelled) {
          setEntryExitSimMarketTrendData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimMarketTrendError('Could not load market trend snapshot.');
          setEntryExitSimMarketTrendData({
            sim_run_id: selectedEntryExitSimulationTestId,
            performance: [],
            alignment: [],
            direction_alignment: [],
          });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimMarketTrendLoading(false);
        }
      }
    };

    void loadEntryExitSimMarketTrends();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId, entryExitRouterRefreshKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimLossClustering = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimLossClusterData({ sim_run_id: '', summary: null, buckets: [], windows: [] });
        setEntryExitSimLossClusterError('');
        setEntryExitSimLossClusterLoading(false);
        return;
      }

      try {
        setEntryExitSimLossClusterLoading(true);
        setEntryExitSimLossClusterError('');
        const data = await fetchEntryExitSimLossClustering({
          simRunId: selectedEntryExitSimulationTestId,
          windowLimit: 60,
        });

        if (!isCancelled) {
          setEntryExitSimLossClusterData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimLossClusterError('Could not load loss behavior.');
          setEntryExitSimLossClusterData({
            sim_run_id: selectedEntryExitSimulationTestId,
            summary: null,
            buckets: [],
            windows: [],
          });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimLossClusterLoading(false);
        }
      }
    };

    void loadEntryExitSimLossClustering();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId]);

  useEffect(() => {
    let isCancelled = false;

    const loadEntryExitSimStreaks = async () => {
      if (!selectedEntryExitSimulationTestId) {
        setEntryExitSimStreakData({ sim_run_id: '', streaks: [] });
        setEntryExitSimStreakError('');
        setEntryExitSimStreakLoading(false);
        return;
      }

      try {
        setEntryExitSimStreakLoading(true);
        setEntryExitSimStreakError('');
        const data = await fetchEntryExitSimStreaks({
          simRunId: selectedEntryExitSimulationTestId,
        });

        if (!isCancelled) {
          setEntryExitSimStreakData(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitSimStreakError('Could not load streak chart.');
          setEntryExitSimStreakData({ sim_run_id: selectedEntryExitSimulationTestId, streaks: [] });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitSimStreakLoading(false);
        }
      }
    };

    void loadEntryExitSimStreaks();

    return () => {
      isCancelled = true;
    };
  }, [selectedEntryExitSimulationTestId]);

  useEffect(() => {
    if (ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) {
      setEntryExitTemplateBreakdown({ market: [], harmonic_type: [], conditions: [], family_results: [], combos: [] });
      setEntryExitTemplateBreakdownError('');
      setEntryExitTemplateBreakdownLoading(false);
      return undefined;
    }

    let isCancelled = false;

    const loadEntryExitTemplateBreakdown = async () => {
      if (!selectedEntryExitTemplate?.template_uid || !selectedEntryExitTemplate?.origin_run_id) {
        setEntryExitTemplateBreakdown({ market: [], harmonic_type: [], conditions: [], family_results: [], combos: [] });
        setEntryExitTemplateBreakdownError('');
        setEntryExitTemplateBreakdownLoading(false);
        return;
      }

      try {
        setEntryExitTemplateBreakdownLoading(true);
        setEntryExitTemplateBreakdownError('');
        const data = await fetchEntryExitTemplateBreakdown({
          runId: selectedEntryExitTemplate.origin_run_id,
          templateUid: selectedEntryExitTemplate.template_uid,
        });

        if (!isCancelled) {
          setEntryExitTemplateBreakdown(data);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setEntryExitTemplateBreakdownError('Could not load pattern market split.');
          setEntryExitTemplateBreakdown({ market: [], harmonic_type: [], conditions: [], family_results: [], combos: [] });
        }
      } finally {
        if (!isCancelled) {
          setEntryExitTemplateBreakdownLoading(false);
        }
      }
    };

    void loadEntryExitTemplateBreakdown();

    return () => {
      isCancelled = true;
    };
  }, [isEntryExitStandalone, selectedEntryExitTemplate?.origin_run_id, selectedEntryExitTemplate?.template_uid]);

  useEffect(() => {
    if (ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) {
      setRouteFamilyRows([]);
      setRouteFamiliesError('');
      setRouteFamiliesLoading(false);
      return undefined;
    }

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
  }, [isEntryExitStandalone, selectedRoute?.route_id, sourceScope, yearFilter]);

  useEffect(() => {
    if (ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) {
      setRouteTrades([]);
      setRouteTradesError('');
      setRouteTradesLoading(false);
      return undefined;
    }

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
    isEntryExitStandalone,
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
    if (ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) {
      setSelectedPatternRouteTrade(null);
      setPatternRouteTradeError('');
      setPatternRouteTradeLoading(false);
      return undefined;
    }

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
  }, [
    isEntryExitStandalone,
    selectedFamilyKey,
    selectedFamilyPattern,
    selectedRoute,
    selectedRouteTradeMatchesSelectedPattern,
    simContractsCount,
  ]);

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

  const loadLiveCanvasCandles = useCallback(
    async ({ showLoading = false, force = false, symbol = null } = {}) => {
      if (liveCanvasRefreshInFlightRef.current) {
        return false;
      }

      const requestedSymbol = String(symbol || liveCanvasSymbolRef.current || LIVE_CANVAS_SYMBOL).toUpperCase();
      const targetSymbol = LIVE_CANVAS_ALL_CONTRACTS ? LIVE_CANVAS_ROOT : requestedSymbol;
      liveCanvasSymbolRef.current = targetSymbol;
      setLiveCanvasSymbol(targetSymbol);

      liveCanvasRefreshInFlightRef.current = true;
      setLiveCanvasRefreshMeta((current) => ({
        ...current,
        isRefreshing: true,
        error: '',
      }));
      if (showLoading) {
        setCanvasLoading(true);
      }

      try {
        const candles = await getCandles(targetSymbol, {
          rootSymbol: LIVE_CANVAS_ALL_CONTRACTS ? LIVE_CANVAS_ROOT : null,
          sourceTimeframe: LIVE_CANVAS_TIMEFRAME,
          allContracts: LIVE_CANVAS_ALL_CONTRACTS,
          startDate: LIVE_CANVAS_START_DATE,
          endDate: LIVE_CANVAS_END_DATE,
          limit: LIVE_CANVAS_ALL_CONTRACTS ? LIVE_CANVAS_INITIAL_CANDLE_PAGE_LIMIT : null,
        }).then(normalizeCandles).then(addLiveCanvasDisplayDates);
        liveCanvasCandlesRef.current = candles;
        liveCanvasLoadedAllOlderRef.current =
          LIVE_CANVAS_ALL_CONTRACTS && candles.length < LIVE_CANVAS_INITIAL_CANDLE_PAGE_LIMIT;
        updateLiveCanvasCandlePageMetaRef.current({
          isLoadingOlder: false,
          hasOlder: !LIVE_CANVAS_ALL_CONTRACTS || candles.length >= LIVE_CANVAS_INITIAL_CANDLE_PAGE_LIMIT,
          newestCandleAt: getLiveCanvasDisplayDate(candles[0]),
          oldestCandleAt: getLiveCanvasDisplayDate(candles[candles.length - 1]),
          loadedCount: candles.length,
        });
        const signature = `${targetSymbol}|${getLiveCanvasSignature(candles)}`;
        const changed = force || signature !== liveCanvasSignatureRef.current;

        setLiveCanvasRefreshMeta((current) => ({
          ...current,
          checkedAt: new Date(),
          latestCandleAt: getLiveCanvasDisplayDate(candles[0]),
          candleCount: candles.length,
          isRefreshing: false,
          changed,
          error: '',
        }));

        if (!changed) {
          return false;
        }
        if (selectedFamilyKeyRef.current) {
          return false;
        }

        liveCanvasSignatureRef.current = signature;
        const chartPattern = buildLiveCanvasPattern(candles, selectedLiveTrendRef.current, targetSymbol);
        setCanvasPattern(chartPattern);
        setCanvasChartData({
          candles,
          snr_lines: [],
          rust_patterns: chartPattern,
        });
        setCanvasError('');
        return true;
      } catch (loadError) {
        console.error(loadError);
        setLiveCanvasRefreshMeta((current) => ({
          ...current,
          checkedAt: new Date(),
          isRefreshing: false,
          changed: false,
          error: `Could not check ${targetSymbol} ${LIVE_CANVAS_TIMEFRAME}`,
        }));
        if (showLoading) {
          setCanvasChartData({ candles: [], rust_patterns: null });
          setCanvasPattern(null);
          setCanvasError(`Could not load ${targetSymbol} ${LIVE_CANVAS_TIMEFRAME} candles.`);
        }
        return false;
      } finally {
        liveCanvasRefreshInFlightRef.current = false;
        setLiveCanvasRefreshMeta((current) => (
          current.isRefreshing ? { ...current, isRefreshing: false } : current
        ));
        if (showLoading) {
          setCanvasLoading(false);
        }
      }
    },
    []
  );

  const loadLiveTrendEvents = useCallback(async () => {
    if (liveTrendRefreshInFlightRef.current) {
      return false;
    }

    liveTrendRefreshInFlightRef.current = true;
    setLiveTrendEvents((current) => ({
      ...current,
      isLoading: true,
      error: '',
    }));

    try {
      const data = await fetchNinjaTraderTrendEvents({
        rootSymbol: LIVE_CANVAS_ROOT,
        timeframe: LIVE_CANVAS_TIMEFRAME,
        runId: LIVE_CANVAS_TREND_RUN_ID,
        startDate: LIVE_CANVAS_START_DATE,
        endDate: LIVE_CANVAS_END_DATE,
        limit: LIVE_CANVAS_TREND_LIMIT,
      });

      setLiveTrendEvents({
        runId: data.runId ?? null,
        totalRows: data.totalRows ?? 0,
        checkedAt: new Date(),
        isLoading: false,
        error: data.error ?? '',
        rows: Array.isArray(data.rows) ? data.rows : [],
      });
      return true;
    } catch (trendError) {
      console.error(trendError);
      setLiveTrendEvents((current) => ({
        ...current,
        checkedAt: new Date(),
        isLoading: false,
        error: `Could not load ${LIVE_CANVAS_ROOT} trend events.`,
      }));
      return false;
    } finally {
      liveTrendRefreshInFlightRef.current = false;
    }
  }, []);

  const loadLiveScannerActivity = useCallback(async () => {
    setLiveScannerActivity((current) => ({
      ...current,
      isLoading: true,
      error: '',
    }));

    try {
      const data = await fetchNinjaTraderScannerActivity({
        rootSymbol: LIVE_CANVAS_ROOT,
        timeframe: LIVE_CANVAS_TIMEFRAME,
        runId: LIVE_CANVAS_TREND_RUN_ID,
        startDate: LIVE_CANVAS_START_DATE,
        endDate: LIVE_CANVAS_END_DATE,
        limit: LIVE_CANVAS_SCANNER_LIMIT,
      });

      setLiveScannerActivity({
        runId: data.runId ?? LIVE_CANVAS_TREND_RUN_ID,
        totalRows: data.totalRows ?? 0,
        checkedAt: new Date(),
        isLoading: false,
        error: data.error ?? '',
        rows: Array.isArray(data.rows) ? data.rows : [],
      });
      return true;
    } catch (scannerError) {
      console.error(scannerError);
      setLiveScannerActivity((current) => ({
        ...current,
        checkedAt: new Date(),
        isLoading: false,
        error: `Could not load ${LIVE_CANVAS_ROOT} scanner activity.`,
      }));
      return false;
    }
  }, []);

  const loadLiveTradeSignals = useCallback(async () => {
    setLiveTradeSignals((current) => ({
      ...current,
      isLoading: true,
      error: '',
    }));

    try {
      const data = await fetchNinjaTraderSignalHistory({
        accountName: LIVE_CANVAS_ACCOUNT_NAME,
        instrument: LIVE_CANVAS_SIGNAL_INSTRUMENT,
        rootSymbol: LIVE_CANVAS_ROOT,
        expectedAiRunId: LIVE_CANVAS_SIGNAL_RUN_ID,
        includeCancelled: false,
        limit: 50,
      });

      setLiveTradeSignals({
        runId: LIVE_CANVAS_SIGNAL_RUN_ID,
        totalRows: data.totalRows ?? 0,
        summary: data.summary ?? null,
        checkedAt: new Date(),
        isLoading: false,
        error: data.error ?? '',
        rows: Array.isArray(data.rows) ? data.rows : [],
      });
      return true;
    } catch (signalError) {
      console.error(signalError);
      setLiveTradeSignals((current) => ({
        ...current,
        checkedAt: new Date(),
        isLoading: false,
        error: `Could not load ${LIVE_CANVAS_SIGNAL_INSTRUMENT} live trades.`,
      }));
      return false;
    }
  }, []);

  const refreshLiveCanvasSnapshot = useCallback(async () => {
    if (LIVE_CANVAS_FIXED_HISTORY_WINDOW || selectedFamilyKeyRef.current) {
      return false;
    }

    try {
      const snapshot = await fetchNinjaTraderLiveBarSnapshot({
        rootSymbol: LIVE_CANVAS_ROOT,
        timeframe: LIVE_CANVAS_TIMEFRAME,
        instrument: LIVE_CANVAS_SIGNAL_INSTRUMENT,
      });
      const snapshotCandle = mapLiveBarSnapshotToCanvasCandle(snapshot);
      const snapshotSignature = snapshotCandle
        ? [
            snapshotCandle.symbol,
            snapshotCandle.candle_date,
            snapshotCandle.candle_open,
            snapshotCandle.candle_high,
            snapshotCandle.candle_low,
            snapshotCandle.candle_close,
            snapshotCandle.candle_volume,
            snapshotCandle.candle_snapshot_time,
          ].join('|')
        : '';

      setLiveCanvasRefreshMeta((current) => ({
        ...current,
        snapshotCheckedAt: new Date(),
        latestSnapshotAt: snapshotCandle?.candle_snapshot_time ?? current.latestSnapshotAt,
        latestSnapshotPrice: snapshotCandle?.candle_last_price ?? current.latestSnapshotPrice,
        latestCandleAt: snapshotCandle?.candle_display_date ?? current.latestCandleAt,
        error: '',
      }));

      if (!snapshotCandle || snapshotSignature === liveCanvasSnapshotSignatureRef.current) {
        return false;
      }

      const currentCandles = liveCanvasCandlesRef.current;
      if (!currentCandles.length) {
        liveCanvasSnapshotSignatureRef.current = snapshotSignature;
        return false;
      }

      const newestLoadedTime = getDateTimeForCompare(currentCandles[0]?.candle_date);
      const snapshotTime = getDateTimeForCompare(snapshotCandle.candle_date);
      if (
        newestLoadedTime !== null &&
        snapshotTime !== null &&
        snapshotTime < newestLoadedTime - getTimeframeMs(LIVE_CANVAS_TIMEFRAME)
      ) {
        liveCanvasSnapshotSignatureRef.current = snapshotSignature;
        return false;
      }

      const mergedCandles = mergeLiveCanvasCandles(currentCandles, [snapshotCandle]);
      const visualSignature = `${LIVE_CANVAS_ROOT}|${getLiveCanvasSignature(mergedCandles)}`;
      liveCanvasSnapshotSignatureRef.current = snapshotSignature;
      liveCanvasSignatureRef.current = visualSignature;
      liveCanvasCandlesRef.current = mergedCandles;

      updateLiveCanvasCandlePageMetaRef.current((current) => ({
        ...current,
        newestCandleAt: getLiveCanvasDisplayDate(mergedCandles[0]) ?? current.newestCandleAt,
        oldestCandleAt: getLiveCanvasDisplayDate(mergedCandles[mergedCandles.length - 1]) ?? current.oldestCandleAt,
        loadedCount: mergedCandles.length,
      }));
      setLiveCanvasRefreshMeta((current) => ({
        ...current,
        latestCandleAt: getLiveCanvasDisplayDate(mergedCandles[0]) ?? current.latestCandleAt,
        candleCount: mergedCandles.length,
        changed: true,
      }));

      const chartPattern = {
        ...buildLiveCanvasPattern(
          mergedCandles,
          selectedLiveTrendRef.current,
          LIVE_CANVAS_ROOT
        ),
        raw_preserve_viewport_on_count_change: true,
        raw_prepend_candle_count: 0,
      };
      setCanvasPattern(chartPattern);
      setCanvasChartData((current) => ({
        ...current,
        candles: mergedCandles,
        snr_lines: current.snr_lines ?? [],
        rust_patterns: chartPattern,
      }));
      return true;
    } catch (error) {
      console.error(error);
      setLiveCanvasRefreshMeta((current) => ({
        ...current,
        snapshotCheckedAt: new Date(),
        error: `Could not load latest ${LIVE_CANVAS_SIGNAL_INSTRUMENT} snapshot.`,
      }));
      return false;
    }
  }, []);

  const refreshLatestLiveCanvasCandles = useCallback(async () => {
    if (LIVE_CANVAS_FIXED_HISTORY_WINDOW) {
      return false;
    }

    if (!LIVE_CANVAS_ALL_CONTRACTS) {
      return loadLiveCanvasCandles({ showLoading: false, force: false });
    }

    if (liveCanvasRefreshInFlightRef.current) {
      return false;
    }

    const currentCandles = liveCanvasCandlesRef.current;
    if (!currentCandles.length) {
      return loadLiveCanvasCandles({ showLoading: false, force: true });
    }

    const newestLoadedCandle = currentCandles[0];
    const refreshStartDate =
      addTimeframeBars(newestLoadedCandle?.candle_date, LIVE_CANVAS_TIMEFRAME, -3) ??
      newestLoadedCandle?.candle_date;

    if (!refreshStartDate) {
      return false;
    }

    liveCanvasRefreshInFlightRef.current = true;
    setLiveCanvasRefreshMeta((current) => ({
      ...current,
      isRefreshing: true,
      error: '',
    }));

    try {
      const recentCandles = await getCandles(LIVE_CANVAS_ROOT, {
        rootSymbol: LIVE_CANVAS_ROOT,
        sourceTimeframe: LIVE_CANVAS_TIMEFRAME,
        allContracts: true,
        startDate: refreshStartDate,
        limit: LIVE_CANVAS_RECENT_REFRESH_LIMIT,
      }).then(normalizeCandles).then(addLiveCanvasDisplayDates);

      if (!recentCandles.length) {
        setLiveCanvasRefreshMeta((current) => ({
          ...current,
          checkedAt: new Date(),
          isRefreshing: false,
          changed: false,
          error: '',
        }));
        return false;
      }

      const previousNewestKey = getLiveCanvasCandleKey(currentCandles[0]);
      const mergedCandles = mergeLiveCanvasCandles(currentCandles, recentCandles);
      const prependCandleCount = Math.max(
        0,
        mergedCandles.findIndex((candle) => getLiveCanvasCandleKey(candle) === previousNewestKey)
      );
      const signature = `${LIVE_CANVAS_ROOT}|${getLiveCanvasSignature(mergedCandles)}`;
      const changed = signature !== liveCanvasSignatureRef.current;

      setLiveCanvasRefreshMeta((current) => ({
        ...current,
        checkedAt: new Date(),
        latestCandleAt: getLiveCanvasDisplayDate(mergedCandles[0]),
        candleCount: mergedCandles.length,
        isRefreshing: false,
        changed,
        error: '',
      }));
      updateLiveCanvasCandlePageMetaRef.current((current) => ({
        ...current,
        newestCandleAt: getLiveCanvasDisplayDate(mergedCandles[0]) ?? current.newestCandleAt,
        oldestCandleAt: getLiveCanvasDisplayDate(mergedCandles[mergedCandles.length - 1]) ?? current.oldestCandleAt,
        loadedCount: mergedCandles.length,
      }));

      if (!changed) {
        return false;
      }

      liveCanvasSignatureRef.current = signature;
      liveCanvasCandlesRef.current = mergedCandles;

      if (selectedFamilyKeyRef.current) {
        return false;
      }

      const chartPattern = {
        ...buildLiveCanvasPattern(
          mergedCandles,
          selectedLiveTrendRef.current,
          LIVE_CANVAS_ROOT
        ),
        raw_preserve_viewport_on_count_change: true,
        raw_prepend_candle_count: prependCandleCount,
      };
      setCanvasPattern(chartPattern);
      setCanvasChartData((current) => ({
        ...current,
        candles: mergedCandles,
        snr_lines: current.snr_lines ?? [],
        rust_patterns: chartPattern,
      }));
      setCanvasError('');
      return true;
    } catch (error) {
      console.error(error);
      setLiveCanvasRefreshMeta((current) => ({
        ...current,
        checkedAt: new Date(),
        isRefreshing: false,
        changed: false,
        error: `Could not refresh latest ${LIVE_CANVAS_ROOT} ${LIVE_CANVAS_TIMEFRAME}`,
      }));
      return false;
    } finally {
      liveCanvasRefreshInFlightRef.current = false;
      setLiveCanvasRefreshMeta((current) => (
        current.isRefreshing ? { ...current, isRefreshing: false } : current
      ));
    }
  }, [loadLiveCanvasCandles]);

  const loadOlderLiveCanvasCandles = useCallback(async () => {
    if (
      !LIVE_CANVAS_ALL_CONTRACTS ||
      liveCanvasOlderCandlesInFlightRef.current ||
      liveCanvasLoadedAllOlderRef.current
    ) {
      return false;
    }

    const currentCandles = liveCanvasCandlesRef.current;
    const oldestCandle = currentCandles[currentCandles.length - 1];
    const beforeDate = addTimeframeBars(oldestCandle?.candle_date, LIVE_CANVAS_TIMEFRAME, -1);

    if (!beforeDate) {
      return false;
    }

    liveCanvasOlderCandlesInFlightRef.current = true;
    updateLiveCanvasCandlePageMetaRef.current((current) => ({
      ...current,
      isLoadingOlder: true,
    }));

    try {
      const olderCandles = await getCandles(LIVE_CANVAS_ROOT, {
        rootSymbol: LIVE_CANVAS_ROOT,
        sourceTimeframe: LIVE_CANVAS_TIMEFRAME,
        allContracts: true,
        startDate: LIVE_CANVAS_START_DATE,
        endDate: beforeDate,
        limit: LIVE_CANVAS_CANDLE_PAGE_LIMIT,
      }).then(normalizeCandles).then(addLiveCanvasDisplayDates);

      if (!olderCandles.length) {
        liveCanvasLoadedAllOlderRef.current = true;
        updateLiveCanvasCandlePageMetaRef.current((current) => ({
          ...current,
          isLoadingOlder: false,
          hasOlder: false,
        }));
        return false;
      }

      const mergedCandles = mergeLiveCanvasCandles(currentCandles, olderCandles);
      liveCanvasCandlesRef.current = mergedCandles;
      liveCanvasLoadedAllOlderRef.current = olderCandles.length < LIVE_CANVAS_CANDLE_PAGE_LIMIT;
      const chartPattern = {
        ...buildLiveCanvasPattern(
          mergedCandles,
          selectedLiveTrendRef.current,
          LIVE_CANVAS_ROOT
        ),
        raw_preserve_viewport_on_count_change: true,
      };

      updateLiveCanvasCandlePageMetaRef.current({
        isLoadingOlder: false,
        hasOlder: !liveCanvasLoadedAllOlderRef.current,
        newestCandleAt: getLiveCanvasDisplayDate(mergedCandles[0]),
        oldestCandleAt: getLiveCanvasDisplayDate(mergedCandles[mergedCandles.length - 1]),
        loadedCount: mergedCandles.length,
      });
      setLiveCanvasRefreshMeta((current) => ({
        ...current,
        candleCount: mergedCandles.length,
      }));
      setCanvasPattern(chartPattern);
      setCanvasChartData((current) => ({
        ...current,
        candles: mergedCandles,
        snr_lines: current.snr_lines ?? [],
        rust_patterns: chartPattern,
      }));
      return true;
    } catch (error) {
      console.error(error);
      updateLiveCanvasCandlePageMetaRef.current((current) => ({
        ...current,
        isLoadingOlder: false,
      }));
      return false;
    } finally {
      liveCanvasOlderCandlesInFlightRef.current = false;
    }
  }, []);

  const loadLiveOracleTrendEvents = useCallback(async () => {
    if (liveOracleTrendRefreshInFlightRef.current) {
      return false;
    }

    liveOracleTrendRefreshInFlightRef.current = true;
    setLiveOracleTrendEvents((current) => ({
      ...current,
      isLoading: true,
      error: '',
    }));

    try {
      const data = await fetchNinjaTraderOracleTrends({
        rootSymbol: LIVE_CANVAS_ROOT,
        timeframe: LIVE_CANVAS_TIMEFRAME,
        runId: LIVE_CANVAS_ORACLE_RUN_ID,
        startDate: LIVE_CANVAS_START_DATE,
        endDate: LIVE_CANVAS_END_DATE,
        limit: LIVE_CANVAS_ORACLE_LIMIT,
      });

      setLiveOracleTrendEvents({
        runId: data.runId ?? LIVE_CANVAS_ORACLE_RUN_ID,
        totalRows: data.totalRows ?? 0,
        checkedAt: new Date(),
        isLoading: false,
        error: data.error ?? '',
        rows: Array.isArray(data.rows) ? data.rows : [],
      });
      return true;
    } catch (oracleError) {
      console.error(oracleError);
      setLiveOracleTrendEvents((current) => ({
        ...current,
        checkedAt: new Date(),
        isLoading: false,
        error: `Could not load ${LIVE_CANVAS_ROOT} oracle trends.`,
      }));
      return false;
    } finally {
      liveOracleTrendRefreshInFlightRef.current = false;
    }
  }, []);

  const loadFocusedLiveCanvasCandles = useCallback(
    async (trend, { canvasSymbol = LIVE_CANVAS_ROOT } = {}) => {
      const requestedSymbol = String(canvasSymbol || LIVE_CANVAS_ROOT).toUpperCase();
      const targetSymbol = LIVE_CANVAS_ALL_CONTRACTS ? LIVE_CANVAS_ROOT : requestedSymbol;
      const windowParams = buildLiveCanvasTrendCandleWindow(trend);

      setCanvasLoading(true);
      try {
        const candles = await getCandles(targetSymbol, {
          rootSymbol: LIVE_CANVAS_ALL_CONTRACTS ? LIVE_CANVAS_ROOT : null,
          sourceTimeframe: LIVE_CANVAS_TIMEFRAME,
          allContracts: LIVE_CANVAS_ALL_CONTRACTS,
          startDate: windowParams.startDate,
          endDate: windowParams.endDate,
          limit: windowParams.limit,
        }).then(normalizeCandles).then(addLiveCanvasDisplayDates);

        if (!candles.length) {
          return null;
        }

        liveCanvasCandlesRef.current = candles;
        liveCanvasLoadedAllOlderRef.current =
          LIVE_CANVAS_ALL_CONTRACTS && candles.length < windowParams.limit;
        liveCanvasSignatureRef.current = `${targetSymbol}|${getLiveCanvasSignature(candles)}`;
        setLiveCanvasSymbol(targetSymbol);
        liveCanvasSymbolRef.current = targetSymbol;
        updateLiveCanvasCandlePageMetaRef.current({
          isLoadingOlder: false,
          hasOlder: !LIVE_CANVAS_ALL_CONTRACTS || candles.length >= windowParams.limit,
          newestCandleAt: getLiveCanvasDisplayDate(candles[0]),
          oldestCandleAt: getLiveCanvasDisplayDate(candles[candles.length - 1]),
          loadedCount: candles.length,
        });
        setLiveCanvasRefreshMeta((current) => ({
          ...current,
          checkedAt: new Date(),
          latestCandleAt: getLiveCanvasDisplayDate(candles[0]) ?? current.latestCandleAt,
          candleCount: candles.length,
          changed: true,
          error: '',
        }));

        return candles;
      } catch (error) {
        console.error(error);
        setLiveCanvasRefreshMeta((current) => ({
          ...current,
          checkedAt: new Date(),
          changed: false,
          error: `Could not load focused ${targetSymbol} ${LIVE_CANVAS_TIMEFRAME}`,
        }));
        return null;
      } finally {
        setCanvasLoading(false);
      }
    },
    []
  );

  const applyLiveCanvasTrend = useCallback(
    (trend, { canvasSymbol = LIVE_CANVAS_ROOT, preserveViewport = false } = {}) => {
      const candles = liveCanvasCandlesRef.current.length
        ? liveCanvasCandlesRef.current
        : canvasChartData.candles ?? [];

      if (!candles.length) {
        return false;
      }

      const chartPattern = {
        ...buildLiveCanvasPattern(candles, trend, canvasSymbol),
        ...(preserveViewport ? { raw_preserve_viewport_on_count_change: true } : {}),
      };
      setCanvasPattern(chartPattern);
      setCanvasChartData((current) => ({
        ...current,
        candles,
        snr_lines: current.snr_lines ?? [],
        rust_patterns: chartPattern,
      }));
      setCanvasError('');
      return true;
    },
    [canvasChartData.candles]
  );

  const handleLiveTrendSelect = useCallback(
    async (trend) => {
      const trendKey = getTrendEventKey(trend);
      const trendSymbol = String(trend?.model_symbol || LIVE_CANVAS_SYMBOL).toUpperCase();
      const canvasSymbol = LIVE_CANVAS_ALL_CONTRACTS ? LIVE_CANVAS_ROOT : trendSymbol;
      const needsSymbolReload = !LIVE_CANVAS_ALL_CONTRACTS && trendSymbol !== liveCanvasSymbolRef.current;

      selectedLiveTrendRef.current = trend ?? null;
      setSelectedLiveTrendKey(trendKey);

      if (needsSymbolReload) {
        liveCanvasSignatureRef.current = '';
        const focusedCandles = await loadFocusedLiveCanvasCandles(trend, { canvasSymbol });
        if (focusedCandles?.length) {
          applyLiveCanvasTrend(trend, { canvasSymbol });
        } else {
          await loadLiveCanvasCandles({ showLoading: true, force: true, symbol: canvasSymbol });
        }
        return;
      }

      const focusedCandles = await loadFocusedLiveCanvasCandles(trend, { canvasSymbol });
      const candles = focusedCandles?.length
        ? focusedCandles
        : liveCanvasCandlesRef.current.length
          ? liveCanvasCandlesRef.current
          : canvasChartData.candles ?? [];

      if (!candles.length) {
        await loadLiveCanvasCandles({ showLoading: true, force: true, symbol: canvasSymbol });
        return;
      }

      applyLiveCanvasTrend(trend, { canvasSymbol });
    },
    [applyLiveCanvasTrend, canvasChartData.candles, loadFocusedLiveCanvasCandles, loadLiveCanvasCandles]
  );

  const handleLiveOracleTrendSelect = useCallback(
    async (oracleTrend) => {
      const trend = mapOracleTrendToLiveCanvasTrend(oracleTrend);
      const trendKey = getTrendEventKey(trend);
      const trendSymbol = String(oracleTrend?.symbol || LIVE_CANVAS_SYMBOL).toUpperCase();
      const canvasSymbol = LIVE_CANVAS_ALL_CONTRACTS ? LIVE_CANVAS_ROOT : trendSymbol;
      const needsSymbolReload = !LIVE_CANVAS_ALL_CONTRACTS && trendSymbol !== liveCanvasSymbolRef.current;

      selectedLiveTrendRef.current = trend;
      setSelectedLiveTrendKey(trendKey);

      if (needsSymbolReload) {
        liveCanvasSignatureRef.current = '';
        const focusedCandles = await loadFocusedLiveCanvasCandles(trend, { canvasSymbol });
        if (focusedCandles?.length) {
          applyLiveCanvasTrend(trend, { canvasSymbol });
        } else {
          await loadLiveCanvasCandles({ showLoading: true, force: true, symbol: canvasSymbol });
        }
        return;
      }

      const focusedCandles = await loadFocusedLiveCanvasCandles(trend, { canvasSymbol });
      const candles = focusedCandles?.length
        ? focusedCandles
        : liveCanvasCandlesRef.current.length
          ? liveCanvasCandlesRef.current
          : canvasChartData.candles ?? [];

      if (!candles.length) {
        await loadLiveCanvasCandles({ showLoading: true, force: true, symbol: canvasSymbol });
        return;
      }

      applyLiveCanvasTrend(trend, { canvasSymbol });
    },
    [applyLiveCanvasTrend, canvasChartData.candles, loadFocusedLiveCanvasCandles, loadLiveCanvasCandles]
  );

  const handleLiveTradeSignalSelect = useCallback(
    async (trade) => {
      const trend = mapLiveTradeSignalToCanvasTrend(trade);
      const trendKey = getTrendEventKey(trend);

      selectedLiveTrendRef.current = trend;
      setSelectedLiveTrendKey(trendKey);

      const focusedCandles = await loadFocusedLiveCanvasCandles(trend, { canvasSymbol: LIVE_CANVAS_ROOT });
      const candles = focusedCandles?.length
        ? focusedCandles
        : liveCanvasCandlesRef.current.length
          ? liveCanvasCandlesRef.current
          : canvasChartData.candles ?? [];

      if (!candles.length) {
        await loadLiveCanvasCandles({ showLoading: true, force: true, symbol: LIVE_CANVAS_ROOT });
        return;
      }

      applyLiveCanvasTrend(trend, { canvasSymbol: LIVE_CANVAS_ROOT });
    },
    [applyLiveCanvasTrend, canvasChartData.candles, loadFocusedLiveCanvasCandles, loadLiveCanvasCandles]
  );

  const handleRawCandleViewportEdge = useCallback(
    ({ edge }) => {
      if (edge === 'older') {
        void loadOlderLiveCanvasCandles();
      }
    },
    [loadOlderLiveCanvasCandles]
  );

  useEffect(() => {
    if (ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) {
      setCanvasChartData({ candles: [], rust_patterns: null });
      setCanvasPattern(null);
      setCanvasError('');
      setCanvasLoading(false);
      return undefined;
    }

    let isCancelled = false;

    const loadCanvasPreview = async () => {
      if (!selectedFamilyKey) {
        if (!isCancelled) {
          void loadLiveCanvasCandles({ showLoading: true, force: true });
        }
        return;
      }

      try {
        setCanvasLoading(true);
        setCanvasError('');

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
              sourceTimeframe: timeframeFilter === 'All' ? null : timeframeFilter,
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
          clipCandlesAfterCanvasEnd(candles, chartPattern),
          chartPattern,
          snrLines,
          setCanvasChartData
        );
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setCanvasError('Could not load the family canvas.');
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
  }, [
    isEntryExitStandalone,
    loadLiveCanvasCandles,
    selectedFamilyKey,
    selectedFamilyPattern,
    selectedPatternTrade,
    selectedRouteTrade,
    sourceScope,
    timeframeFilter,
    yearFilter,
  ]);

  useEffect(() => {
    if ((ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) || selectedFamilyKey) {
      return undefined;
    }

    void refreshLiveCanvasSnapshot();

    if (LIVE_CANVAS_FIXED_HISTORY_WINDOW) {
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      if (document.hidden) {
        return;
      }

      void refreshLiveCanvasSnapshot();
    }, LIVE_CANVAS_SNAPSHOT_POLL_MS);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [
    isEntryExitStandalone,
    refreshLiveCanvasSnapshot,
    selectedFamilyKey,
  ]);

  useEffect(() => {
    if ((ENTRY_EXIT_STANDALONE_BUILD_ONLY && isEntryExitStandalone) || selectedFamilyKey) {
      return undefined;
    }

    void loadLiveTrendEvents();
    void loadLiveScannerActivity();
    void loadLiveTradeSignals();
    if (liveTrendPanelMode === 'oracle') {
      void loadLiveOracleTrendEvents();
    }

    if (LIVE_CANVAS_FIXED_HISTORY_WINDOW) {
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      if (document.hidden) {
        return;
      }

      void refreshLatestLiveCanvasCandles();
      void loadLiveTrendEvents();
      void loadLiveScannerActivity();
      void loadLiveTradeSignals();
      if (liveTrendPanelMode === 'oracle') {
        void loadLiveOracleTrendEvents();
      }
    }, LIVE_CANVAS_POLL_MS);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [
    isEntryExitStandalone,
    liveTrendPanelMode,
    loadLiveCanvasCandles,
    loadLiveOracleTrendEvents,
    loadLiveScannerActivity,
    loadLiveTradeSignals,
    loadLiveTrendEvents,
    refreshLatestLiveCanvasCandles,
    selectedFamilyKey,
  ]);

  useEffect(() => {
    if (selectedFamilyKey || !canvasChartData.rust_patterns?.raw_candle_view || !canvasChartData.candles?.length) {
      return;
    }

    const markers = buildWatchingTrendMarkers(canvasChartData.candles, liveWatchingTrendRows);
    const currentMarkers = canvasChartData.rust_patterns.raw_watching_trends ?? [];

    if (getWatchingTrendMarkerSignature(markers) === getWatchingTrendMarkerSignature(currentMarkers)) {
      return;
    }

    const chartPattern = {
      ...canvasChartData.rust_patterns,
      raw_watching_trends: markers,
    };

    setCanvasPattern(chartPattern);
    setCanvasChartData((current) => ({
      ...current,
      rust_patterns: chartPattern,
    }));
  }, [
    canvasChartData.candles,
    canvasChartData.rust_patterns,
    liveWatchingTrendRows,
    selectedFamilyKey,
  ]);

  const livePanelModeTitle =
    liveTrendPanelMode === 'trades'
      ? 'Live Trades'
      : liveTrendPanelMode === 'scanner'
        ? 'Scanner Feed'
      : liveTrendPanelMode === 'oracle'
        ? 'Oracle Trends'
        : 'Trend Detector';
  const livePanelRunId =
    liveTrendPanelMode === 'trades'
      ? liveTradeSignals.runId
      : liveTrendPanelMode === 'scanner'
        ? liveScannerActivity.runId
      : liveTrendPanelMode === 'oracle'
        ? liveOracleTrendEvents.runId
        : liveTrendEvents.runId;
  const liveTradeShownLabel = `${formatNumber(
    liveTradeSignals.summary?.trades_taken ?? liveTradeSignals.rows.length
  )} taken / ${formatNumber(liveTradeSignals.summary?.wins ?? 0)} wins`;
  const liveFeedLabel = liveCanvasRefreshMeta.error
    ? 'Error'
    : liveCanvasRefreshMeta.latestSnapshotAt
      ? 'Live NT'
      : 'Live DB';
  const liveTradePnlValue = Number(liveTradeSignals.summary?.total_accounting_pnl);
  const liveTradePnlTone = Number.isFinite(liveTradePnlValue)
    ? liveTradePnlValue > 0
      ? 'win'
      : liveTradePnlValue < 0
        ? 'loss'
        : 'flat'
    : 'flat';
  const livePanelShownLabel =
    liveTrendPanelMode === 'trades'
      ? liveTradeSignals.isLoading && !liveTradeSignals.rows.length && !liveTradeSignals.summary
        ? 'Checking'
        : liveTradeShownLabel
      : liveTrendPanelMode === 'scanner'
        ? liveScannerActivity.isLoading
          ? 'Checking'
          : `${formatNumber(liveScannerActivity.rows.length)} events`
      : liveTrendPanelMode === 'oracle'
        ? liveOracleTrendEvents.isLoading
          ? 'Checking'
          : `${formatNumber(visibleLiveOracleTrendRows.length)} shown`
      : liveTrendEvents.isLoading
        ? 'Checking'
        : `${formatNumber(visibleLiveTrendRows.length)} shown`;

  const inspectorDetailTitle =
    inspectorDetailMode === 'trade'
      ? 'Current Trade'
      : inspectorDetailMode === 'pattern'
        ? 'Pattern Details'
        : inspectorDetailMode === 'family'
          ? 'Family Details'
          : 'Entry / Exit Logic';
  const inspectorDetailHeading =
    inspectorDetailMode === 'trade'
      ? selectedTradeSource?.symbol ?? 'No trade'
      : inspectorDetailMode === 'pattern'
        ? ''
        : inspectorDetailMode === 'family'
          ? selectedFamily?.harmonic_type ?? 'No family'
          : selectedTradeTemplate
            ? selectedTradeTemplate.template_label || compactText(selectedTradeTemplate.template_uid, 12)
            : selectedRoute
            ? `Route #${selectedRoute.result_rank}`
            : 'No route';
  const inspectorDetailSubheading =
    inspectorDetailMode === 'trade'
      ? selectedTradeSource
        ? `${formatDate(selectedTradeEntryDate)} to ${formatDate(selectedTradeExitDate)}`
        : 'Select a trade'
      : inspectorDetailMode === 'pattern'
        ? ''
      : inspectorDetailMode === 'family'
          ? selectedFamily?.family_key ?? 'Select a family'
          : selectedTradeTemplate
            ? selectedTradeTemplateRule ?? selectedTradeTemplate.template_name ?? 'Template logic'
            : selectedRoute?.route_label ?? 'Select a route';
  const inspectorDetailStats =
    inspectorDetailMode === 'trade'
      ? selectedTradeDetailStats
      : inspectorDetailMode === 'pattern'
        ? selectedPatternDetailStats
        : inspectorDetailMode === 'family'
          ? selectedFamilyDetailStats
          : selectedLogicStats;
  const showRouteLogicPanel = inspectorDetailMode === 'logic' && Boolean(selectedRoute || selectedTradeTemplate);
  const CanvasCollapseWrapper = isEntryExitStandalone ? React.Fragment : 'section';
  const canvasCollapseWrapperProps = isEntryExitStandalone
    ? {}
    : {
        className: 'pattern-family-canvas-collapse-workspace',
        'aria-hidden': !isInspectorCollapsed,
      };
  const renderSimulationRawTradeRow = (trade, index) => {
    const tradeKey = getSimulationRawTradeKey(trade, index);

    return (
      <tr
        className={[
          Number(trade.result_r || 0) < 0 ? 'is-loss' : 'is-win',
          tradeKey === selectedSimulationRawTradeKey ? 'pattern-family-selected-simulation-raw-trade-row--selected' : '',
        ].filter(Boolean).join(' ')}
        key={tradeKey}
        onClick={() => handleSimulationRawTradeClick(trade, index)}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            handleSimulationRawTradeClick(trade, index);
          }
        }}
        role="button"
        tabIndex={0}
        title={`${trade.setup_id} | ${trade.pattern_id || trade.pattern_group_id || 'Pattern'} | ${trade.template_uid}`}
      >
        <td>{formatNumber(Number(entryExitSimRawTradesData.offset || 0) + index + 1)}</td>
        <td>{formatShortDateTime(trade.entry_date || trade.d_confirm_date)}</td>
        <td>{trade.symbol || 'N/A'}</td>
        <td>{trade.source_timeframe || 'N/A'}</td>
        <td title={trade.family_key}>{compactText(trade.family_key || 'N/A', 12)}</td>
        <td title={trade.template_uid}>{trade.template_label || compactText(trade.template_uid || 'N/A', 8)}</td>
        <td>{String(trade.trade_direction || 'N/A').toUpperCase()}</td>
        <td>{formatDecimal(trade.result_r, 2)}R</td>
        <td>{trade.outcome || 'N/A'}</td>
        <td>{trade.exit_reason || 'N/A'}</td>
        <td>{formatGapDuration(trade.duration_minutes)}</td>
        <td>{formatDecimal(trade.entry_price, 2)}</td>
        <td>{formatDecimal(trade.stop_price, 2)}</td>
        <td title={trade.target_price === null || trade.target_price === undefined ? 'No fixed target' : ''}>
          {formatNullableDecimal(trade.target_price, 2)}
        </td>
        <td>{formatDecimal(trade.exit_price, 2)}</td>
        <td>{formatDecimal(trade.tp_progress_pct_after, 1)}%</td>
        <td>{formatDecimal(trade.drawdown_progress_pct_after, 1)}%</td>
      </tr>
    );
  };
  const renderAiStage1TemplatePerformanceTable = () => (
    <table>
      <thead>
        <tr>
          <th>Template</th>
          <th>Trades</th>
          <th>WR</th>
          <th>Avg R</th>
          <th>Net R</th>
          <th>Best</th>
          <th>Worst</th>
          <th>Wins</th>
          <th>Losses</th>
          <th>Families</th>
          <th>Symbols</th>
          <th>AI R</th>
          <th>Margin</th>
        </tr>
      </thead>
      <tbody>
        {patternAiStage1TemplatePerformanceRows.map((row) => {
          const netR = Number(row.sum_r || 0);
          return (
            <tr className={netR >= 0 ? 'is-win' : 'is-loss'} key={row.template_uid}>
              <td title={row.template_uid}>{compactText(row.template_name || row.template_uid || 'N/A', 18)}</td>
              <td>{formatNumber(row.trades)}</td>
              <td>{formatDecimal(row.win_rate, 1)}%</td>
              <td>{formatDecimal(row.avg_r, 3)}R</td>
              <td>{formatDecimal(netR, 1)}R</td>
              <td>{formatDecimal(row.best_r, 1)}R</td>
              <td>{formatDecimal(row.worst_r, 1)}R</td>
              <td>{formatNumber(row.wins)}</td>
              <td>{formatNumber(row.losses)}</td>
              <td>{formatNumber(row.family_count)}</td>
              <td>{formatNumber(row.symbol_count)}</td>
              <td>{formatDecimal(row.avg_predicted_expected_r, 3)}R</td>
              <td>{formatDecimal(row.avg_score_margin_top2, 3)}R</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
  const renderAiStage1DailyTable = () => (
    <table>
      <thead>
        <tr>
          <th>Date</th>
          <th>Trades</th>
          <th>WR</th>
          <th>Daily R</th>
          <th>Cum R</th>
          <th>Avg R</th>
          <th>Best</th>
          <th>Worst</th>
          <th>Wins</th>
          <th>Losses</th>
        </tr>
      </thead>
      <tbody>
        {patternAiStage1DailyRows.map((row) => {
          const dailyR = Number(row.total_r || 0);
          return (
            <tr className={dailyR >= 0 ? 'is-win' : 'is-loss'} key={row.trade_date}>
              <td>{formatDate(row.trade_date)}</td>
              <td>{formatNumber(row.trades)}</td>
              <td>{formatDecimal(row.win_rate, 1)}%</td>
              <td>{formatDecimal(dailyR, 1)}R</td>
              <td>{formatDecimal(row.cumulative_r, 1)}R</td>
              <td>{formatDecimal(row.avg_r, 3)}R</td>
              <td>{formatDecimal(row.best_r, 1)}R</td>
              <td>{formatDecimal(row.worst_r, 1)}R</td>
              <td>{formatNumber(row.wins)}</td>
              <td>{formatNumber(row.losses)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
  const renderAiStage1HourlyTable = () => (
    <table>
      <thead>
        <tr>
          <th>Hour</th>
          <th>Trades</th>
          <th>WR</th>
          <th>Avg R</th>
          <th>Net R</th>
          <th>Best</th>
          <th>Worst</th>
          <th>Wins</th>
          <th>Losses</th>
        </tr>
      </thead>
      <tbody>
        {patternAiStage1HourlyRows.map((row) => {
          const netR = Number(row.sum_r || 0);
          return (
            <tr className={netR >= 0 ? 'is-win' : 'is-loss'} key={row.entry_hour}>
              <td>{formatHourLabel(row.entry_hour)}</td>
              <td>{formatNumber(row.trades)}</td>
              <td>{formatDecimal(row.win_rate, 1)}%</td>
              <td>{formatDecimal(row.avg_r, 3)}R</td>
              <td>{formatDecimal(netR, 1)}R</td>
              <td>{formatDecimal(row.best_r, 1)}R</td>
              <td>{formatDecimal(row.worst_r, 1)}R</td>
              <td>{formatNumber(row.wins)}</td>
              <td>{formatNumber(row.losses)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
  const renderAiStage1SymbolContributionTable = () => (
    <table>
      <thead>
        <tr>
          <th>Symbol</th>
          <th>Trades</th>
          <th>WR</th>
          <th>Avg R</th>
          <th>Net R</th>
          <th>Best</th>
          <th>Worst</th>
          <th>Wins</th>
          <th>Losses</th>
          <th>Families</th>
          <th>Templates</th>
        </tr>
      </thead>
      <tbody>
        {patternAiStage1SymbolContributionRows.map((row) => {
          const netR = Number(row.sum_r || 0);
          return (
            <tr className={netR >= 0 ? 'is-win' : 'is-loss'} key={row.root_symbol}>
              <td>{row.root_symbol || 'N/A'}</td>
              <td>{formatNumber(row.trades)}</td>
              <td>{formatDecimal(row.win_rate, 1)}%</td>
              <td>{formatDecimal(row.avg_r, 3)}R</td>
              <td>{formatDecimal(netR, 1)}R</td>
              <td>{formatDecimal(row.best_r, 1)}R</td>
              <td>{formatDecimal(row.worst_r, 1)}R</td>
              <td>{formatNumber(row.wins)}</td>
              <td>{formatNumber(row.losses)}</td>
              <td>{formatNumber(row.family_count)}</td>
              <td>{formatNumber(row.template_count)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
  const renderAiStage1FamilyContributionTable = () => (
    <table>
      <thead>
        <tr>
          <th>#</th>
          <th>Family Features</th>
          <th>Family ID</th>
          <th>Trades</th>
          <th>WR</th>
          <th>Avg R</th>
          <th>Net R</th>
          <th>Best</th>
          <th>Worst</th>
          <th>Symbols</th>
          <th>Templates</th>
        </tr>
      </thead>
      <tbody>
        {patternAiStage1FamilyContributionRows.map((row, index) => {
          const netR = Number(row.sum_r || 0);
          const features = [
            row.harmonic_type,
            row.family_bin,
            row.family_size_bucket,
            row.family_time_bin,
            row.family_x_strictness,
          ].filter(Boolean).join(' | ');
          return (
            <tr className={netR >= 0 ? 'is-win' : 'is-loss'} key={row.family_key}>
              <td>{formatNumber(index + 1)}</td>
              <td title={features}>{features || 'N/A'}</td>
              <td title={row.family_key}>{compactText(row.family_key || 'N/A', 14)}</td>
              <td>{formatNumber(row.trades)}</td>
              <td>{formatDecimal(row.win_rate, 1)}%</td>
              <td>{formatDecimal(row.avg_r, 3)}R</td>
              <td>{formatDecimal(netR, 1)}R</td>
              <td>{formatDecimal(row.best_r, 1)}R</td>
              <td>{formatDecimal(row.worst_r, 1)}R</td>
              <td>{formatNumber(row.symbol_count)}</td>
              <td>{formatNumber(row.template_count)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
  const renderAiStage1LossWindowsTable = () => (
    <table>
      <thead>
        <tr>
          <th>Date</th>
          <th>Hour</th>
          <th>Trades</th>
          <th>Losses</th>
          <th>Wins</th>
          <th>Loss Rate</th>
          <th>Net R</th>
          <th>Symbols</th>
          <th>Families</th>
          <th>Templates</th>
        </tr>
      </thead>
      <tbody>
        {patternAiStage1LossWindowRows.map((row) => (
          <tr className="is-loss" key={`${row.trade_date}-${row.entry_hour}`}>
            <td>{formatDate(row.trade_date)}</td>
            <td>{formatHourLabel(row.entry_hour)}</td>
            <td>{formatNumber(row.trades)}</td>
            <td>{formatNumber(row.losses)}</td>
            <td>{formatNumber(row.wins)}</td>
            <td>{formatDecimal(row.loss_rate, 1)}%</td>
            <td>{formatDecimal(row.total_r, 1)}R</td>
            <td>{formatNumber(row.symbol_count)}</td>
            <td>{formatNumber(row.family_count)}</td>
            <td>{formatNumber(row.template_count)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
  const renderAiStage1TakenTradesSection = () => (
    <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-templates pattern-family-selected-simulation-chart--collapsible">
      <summary>
        <span>AI 2026 Taken Trades</span>
        <small>
          {patternAiStage1TradeData.totalRows
            ? `${formatNumber(patternAiStage1TradeData.totalRows)} trades | showing ${formatNumber(patternAiStage1TradeRows.length)}`
            : patternAiStage1TradeData.run?.multi_valid_eval_run_id || ''}
        </small>
      </summary>
      <div className="pattern-family-entry-dashboard-raw-table pattern-family-selected-simulation-raw-trades-table pattern-family-ai-taken-trades-table">
        {patternAiStage1TradeRows.length ? (
          <table>
            <colgroup>
              <col style={{ width: '60px' }} />
              <col style={{ width: '132px' }} />
              <col style={{ width: '96px' }} />
              <col style={{ width: '64px' }} />
              <col style={{ width: '92px' }} />
              <col style={{ width: '94px' }} />
              <col style={{ width: '94px' }} />
              <col style={{ width: '94px' }} />
              <col style={{ width: '104px' }} />
              <col style={{ width: '760px' }} />
              <col style={{ width: '122px' }} />
              <col style={{ width: '122px' }} />
              <col style={{ width: '122px' }} />
              <col style={{ width: '122px' }} />
              <col style={{ width: '220px' }} />
              <col style={{ width: '420px' }} />
            </colgroup>
            <thead>
              <tr>
                <th>#</th>
                <th>Time</th>
                <th>Symbol</th>
                <th>TF</th>
                <th>Side</th>
                <th>AI R</th>
                <th>Margin</th>
                <th>R</th>
                <th>Outcome</th>
                <th>Template</th>
                <th>Entry</th>
                <th>Stop</th>
                <th>Target</th>
                <th>Exit</th>
                <th>Family</th>
                <th>Pattern</th>
              </tr>
            </thead>
            <tbody>
              {patternAiStage1TradeRows.map((trade, index) => {
                const tradeKey = getSimulationRawTradeKey(trade, index);
                const resultR = Number(trade.result_r || 0);
                const outcomeClass =
                  trade.outcome === 'pass'
                    ? 'pattern-family-template-table-win'
                    : trade.outcome === 'fail'
                      ? 'pattern-family-template-table-loss'
                      : 'pattern-family-template-table-skipped';
                return (
                  <tr
                    className={[
                      resultR < 0 ? 'is-loss' : 'is-win',
                      tradeKey === selectedSimulationRawTradeKey ? 'pattern-family-selected-simulation-raw-trade-row--selected' : '',
                    ].filter(Boolean).join(' ')}
                    key={tradeKey}
                    onClick={() => handlePatternAiStage1TradeClick(trade, index)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        handlePatternAiStage1TradeClick(trade, index);
                      }
                    }}
                    role="button"
                    tabIndex={0}
                    title={`${trade.setup_id} | ${trade.template_uid} | ${trade.multi_valid_eval_run_id}`}
                  >
                    <td>{formatNumber(index + 1)}</td>
                    <td>{formatShortDateTime(trade.entry_date || trade.d_confirm_date)}</td>
                    <td>{trade.symbol || 'N/A'}</td>
                    <td>{trade.source_timeframe || 'N/A'}</td>
                    <td>{formatRouteMode(trade.trade_direction || trade.market || 'N/A')}</td>
                    <td className={Number(trade.predicted_expected_r || 0) >= 0 ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                      {formatDecimal(trade.predicted_expected_r, 3)}R
                    </td>
                    <td>{formatDecimal(trade.score_margin_top2, 3)}R</td>
                    <td className={resultR >= 0 ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                      {formatDecimal(resultR, 3)}R
                    </td>
                    <td className={outcomeClass}>{formatRouteMode(trade.outcome || trade.exit_reason || 'N/A')}</td>
                    <td className="pattern-family-ai-taken-template-cell" title={trade.template_uid || trade.template_name}>
                      {trade.template_name || trade.template_uid || 'N/A'}
                    </td>
                    <td>{formatDecimal(trade.entry_price, 4)}</td>
                    <td>{formatDecimal(trade.stop_price, 4)}</td>
                    <td title={trade.target_price === null || trade.target_price === undefined ? 'No fixed target' : ''}>
                      {formatNullableDecimal(trade.target_price, 4)}
                    </td>
                    <td>{formatDecimal(trade.exit_price, 4)}</td>
                    <td className="pattern-family-template-table-family-key" title={trade.pattern_family_key}>
                      {trade.pattern_family_key || 'N/A'}
                    </td>
                    <td className="pattern-family-template-table-run-id pattern-family-template-table-copy-cell">
                      <span>{trade.pattern_id || trade.setup_id || 'N/A'}</span>
                      <button
                        aria-label="Copy pattern ID"
                        className={[
                          'pattern-family-table-copy-button',
                          copiedPatternId === String(trade.pattern_id || trade.setup_id)
                            ? 'pattern-family-table-copy-button--copied'
                            : '',
                        ].filter(Boolean).join(' ')}
                        onClick={(event) => handleCopyPatternId(trade.pattern_id || trade.setup_id, event)}
                        title={copiedPatternId === String(trade.pattern_id || trade.setup_id) ? 'Copied' : 'Copy pattern ID'}
                        type="button"
                      >
                        <span aria-hidden="true" />
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        ) : (
          <div className="pattern-family-selected-playbook-empty">
            {isPatternAiStage1TradeLoading
              ? 'Loading AI 2026 taken trades...'
              : patternAiStage1TradeError || 'No AI 2026 taken trades loaded.'}
          </div>
        )}
      </div>
      <footer className="pattern-family-table-footer">
        <button
          disabled={isPatternAiStage1TradeLoading}
          onClick={() => loadPatternAiStage1Trades({ offset: 0, append: false })}
          type="button"
        >
          Refresh
        </button>
        <button
          disabled={isPatternAiStage1TradeLoading || !patternAiStage1TradeHasMore}
          onClick={() =>
            loadPatternAiStage1Trades({
              offset: patternAiStage1TradeRows.length,
              append: true,
            })
          }
          type="button"
        >
          {patternAiStage1TradeHasMore ? 'Load More' : 'All Loaded'}
        </button>
      </footer>
    </details>
  );
  const renderAiStage1TradesPanel = () => (
    <>
      <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-overview pattern-family-selected-simulation-chart--collapsible">
        <summary>
          <span>AI Model Test Overview</span>
          <small>
            {isPatternAiStage1TradeLoading
              ? 'Loading AI model test'
              : patternAiStage1TradeData.run?.multi_valid_eval_run_id || patternAiStage1TradeError || 'No AI model test loaded'}
          </small>
        </summary>
        {patternAiStage1TradeData.run || patternAiStage1TradeData.summary ? (
          <div className="pattern-family-selected-simulation-run-overview pattern-family-selected-simulation-run-overview--playbook">
            {[
              {
                title: 'Model Run',
                items: [
                  {
                    label: 'AI Run',
                    value: compactText(patternAiStage1TradeData.run?.multi_valid_eval_run_id || 'N/A', 22),
                    title: patternAiStage1TradeData.run?.multi_valid_eval_run_id || 'N/A',
                    wide: true,
                  },
                  {
                    label: 'Train Years',
                    value: patternAiStage1TradeData.run
                      ? `${patternAiStage1TradeData.run.train_start_year}-${patternAiStage1TradeData.run.train_end_year}`
                      : 'N/A',
                  },
                  { label: 'Test Year', value: patternAiStage1TradeData.run?.valid_year || 'N/A' },
                  { label: 'Train Setups', value: formatNumber(patternAiStage1TradeData.run?.train_setups) },
                  { label: 'Train Rows', value: formatNumber(patternAiStage1TradeData.run?.train_rows) },
                  { label: 'Valid Slots', value: patternAiStage1TradeData.run?.valid_sample_slots || 'N/A' },
                ],
              },
              {
                title: 'Trade Rules',
                items: [
                  {
                    label: 'Template',
                    value: patternAiStage1PrimaryTemplate?.template_name || patternAiStage1TradeRows[0]?.template_name || 'N/A',
                    title: patternAiStage1PrimaryTemplate?.template_uid || patternAiStage1TradeRows[0]?.template_uid || 'N/A',
                    wide: true,
                  },
                  {
                    label: 'Rule Summary',
                    value: patternAiStage1RuleSummary,
                    title: patternAiStage1RuleSummary,
                    wide: true,
                  },
                  {
                    label: 'Timeframe',
                    value: patternAiStage1Timeframes.length ? patternAiStage1Timeframes.join(', ') : 'N/A',
                  },
                  {
                    label: 'Sides',
                    value: patternAiStage1Directions.length ? patternAiStage1Directions.join(' / ') : 'N/A',
                  },
                  {
                    label: 'Target',
                    value: patternAiStage1HasFixedTargets ? 'Fixed TP present' : 'No fixed TP',
                  },
                  {
                    label: 'Exit Reasons',
                    value: patternAiStage1ExitReasons.length
                      ? patternAiStage1ExitReasons.map(formatRouteMode).join(' / ')
                      : 'N/A',
                    title: patternAiStage1ExitReasons.join(' / ') || 'N/A',
                    wide: true,
                  },
                ],
              },
              {
                title: '2026 Result',
                items: [
                  { label: 'Trades', value: formatNumber(patternAiStage1TradeData.summary?.total_trades) },
                  {
                    label: 'Win Rate',
                    value: `${formatDecimal(patternAiStage1TradeData.summary?.win_rate, 2)}%`,
                    tone: Number(patternAiStage1TradeData.summary?.win_rate || 0) >= 25 ? 'win' : 'loss',
                  },
                  {
                    label: 'Avg R',
                    value: `${formatDecimal(patternAiStage1TradeData.summary?.avg_r, 3)}R`,
                    tone: Number(patternAiStage1TradeData.summary?.avg_r || 0) >= 0 ? 'win' : 'loss',
                  },
                  {
                    label: 'Net R',
                    value: `${formatDecimal(patternAiStage1TradeData.summary?.sum_r, 1)}R`,
                    tone: Number(patternAiStage1TradeData.summary?.sum_r || 0) >= 0 ? 'win' : 'loss',
                  },
                  { label: 'Wins', value: formatNumber(patternAiStage1TradeData.summary?.wins), tone: 'win' },
                  { label: 'Losses', value: formatNumber(patternAiStage1TradeData.summary?.losses), tone: 'loss' },
                  { label: 'No Entry', value: formatNumber(patternAiStage1TradeData.summary?.no_entries) },
                  { label: 'Best R', value: `${formatDecimal(patternAiStage1TradeData.summary?.best_r, 2)}R`, tone: 'win' },
                  { label: 'Worst R', value: `${formatDecimal(patternAiStage1TradeData.summary?.worst_r, 2)}R`, tone: 'loss' },
                ],
              },
              {
                title: 'Coverage',
                items: [
                  { label: 'First Trade', value: formatDate(patternAiStage1TradeData.summary?.first_trade_date) },
                  { label: 'Last Trade', value: formatDate(patternAiStage1TradeData.summary?.last_trade_date) },
                  { label: 'Slots', value: formatNumber(patternAiStage1TradeData.summary?.slot_count) },
                  { label: 'Symbols', value: formatNumber(patternAiStage1TradeData.summary?.symbol_count) },
                  { label: 'Families', value: formatNumber(patternAiStage1TradeData.summary?.family_count) },
                  { label: 'Templates', value: formatNumber(patternAiStage1TradeData.summary?.template_count) },
                  { label: 'Avg AI R', value: `${formatDecimal(patternAiStage1TradeData.summary?.avg_predicted_expected_r, 3)}R` },
                  { label: 'Avg Margin', value: `${formatDecimal(patternAiStage1TradeData.summary?.avg_score_margin_top2, 3)}R` },
                ],
              },
              {
                title: 'Model Settings',
                items: [
                  { label: 'Features', value: patternAiStage1TradeData.run?.pre_feature_set || 'N/A' },
                  { label: 'Aggregates', value: patternAiStage1TradeData.run?.aggregate_feature_set || 'N/A' },
                  { label: 'Depth', value: formatNumber(patternAiStage1TradeData.run?.depth) },
                  { label: 'Iterations', value: formatNumber(patternAiStage1TradeData.run?.iterations) },
                  { label: 'Learning Rate', value: formatDecimal(patternAiStage1TradeData.run?.learning_rate, 3) },
                  { label: 'L2', value: formatDecimal(patternAiStage1TradeData.run?.l2_leaf_reg, 2) },
                  { label: 'Seed', value: formatNumber(patternAiStage1TradeData.run?.random_seed) },
                  {
                    label: 'Excluded Roots',
                    value: patternAiStage1TradeData.run?.excluded_roots || 'None',
                    title: patternAiStage1TradeData.run?.excluded_roots || 'None',
                    wide: true,
                  },
                ],
              },
            ].map((section) => (
              <section className="pattern-family-selected-simulation-run-section" key={`ai-overview-${section.title}`}>
                <header>
                  <span>{section.title}</span>
                </header>
                <div className="pattern-family-selected-simulation-run-grid">
                  {section.items.map((item) => (
                    <div
                      className={[
                        'pattern-family-selected-simulation-run-card',
                        item.wide ? 'pattern-family-selected-simulation-run-card--wide' : '',
                        item.tone ? `pattern-family-selected-simulation-run-card--${item.tone}` : '',
                      ].filter(Boolean).join(' ')}
                      key={`${section.title}-${item.label}`}
                    >
                      <span>{item.label}</span>
                      <strong title={item.title || item.value}>{item.value}</strong>
                    </div>
                  ))}
                </div>
              </section>
            ))}
          </div>
        ) : (
          <div className="pattern-family-selected-playbook-empty">
            {isPatternAiStage1TradeLoading
              ? 'Loading AI model test overview...'
              : patternAiStage1TradeError || 'No AI model test overview loaded.'}
          </div>
        )}
      </details>
      {renderAiStage1TakenTradesSection()}
      <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-templates pattern-family-selected-simulation-chart--collapsible">
        <summary>
          <span>Template Performance</span>
          <small>
            {isPatternAiStage1TradeLoading
              ? 'Loading templates'
              : patternAiStage1TemplatePerformanceRows.length
                ? `${formatNumber(patternAiStage1TemplatePerformanceRows.length)} templates ranked by total R`
                : patternAiStage1TradeError || 'No template performance loaded'}
          </small>
        </summary>
        {patternAiStage1TemplatePerformanceRows.length ? (
          <div className="pattern-family-selected-simulation-contribution-table">
            {renderAiStage1TemplatePerformanceTable()}
          </div>
        ) : (
          <div className="pattern-family-selected-playbook-empty">
            {isPatternAiStage1TradeLoading
              ? 'Loading template performance...'
              : patternAiStage1TradeError || 'No template performance rows loaded.'}
          </div>
        )}
      </details>
      <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-daily-r pattern-family-selected-simulation-chart--collapsible">
        <summary>
          <span>Daily R</span>
          <small>
            {isPatternAiStage1TradeLoading
              ? 'Loading daily R'
              : patternAiStage1DailyRows.length
                ? `${formatNumber(patternAiStage1DailyRows.length)} trading days`
                : patternAiStage1TradeError || 'No daily data loaded'}
          </small>
        </summary>
        {patternAiStage1DailyRows.length ? (
          <div className="pattern-family-selected-simulation-contribution-table">
            {renderAiStage1DailyTable()}
          </div>
        ) : (
          <div className="pattern-family-selected-playbook-empty">
            {isPatternAiStage1TradeLoading
              ? 'Loading daily R...'
              : patternAiStage1TradeError || 'No daily R rows loaded.'}
          </div>
        )}
      </details>
      <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-hourly pattern-family-selected-simulation-chart--collapsible">
        <summary>
          <span>Time Of Day Performance</span>
          <small>
            {isPatternAiStage1TradeLoading
              ? 'Loading hours'
              : patternAiStage1HourlyRows.length
                ? `${formatNumber(patternAiStage1HourlyRows.length)} active hours`
                : patternAiStage1TradeError || 'No hourly data loaded'}
          </small>
        </summary>
        {patternAiStage1HourlyRows.length ? (
          <div className="pattern-family-selected-simulation-contribution-table">
            {renderAiStage1HourlyTable()}
          </div>
        ) : (
          <div className="pattern-family-selected-playbook-empty">
            {isPatternAiStage1TradeLoading
              ? 'Loading time-of-day performance...'
              : patternAiStage1TradeError || 'No hourly rows loaded.'}
          </div>
        )}
      </details>
      <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-cadence pattern-family-selected-simulation-chart--collapsible">
        <summary>
          <span>Trade Cadence</span>
          <small>
            {isPatternAiStage1TradeLoading
              ? 'Loading cadence'
              : patternAiStage1TradeCadence
                ? `${formatNumber(patternAiStage1TradeCadence.trades)} trades | ${formatDecimal(
                    patternAiStage1TradeCadence.median_gap_minutes,
                    1
                  )}m median gap`
                : patternAiStage1TradeError || 'No cadence loaded'}
          </small>
        </summary>
        {patternAiStage1TradeCadence || patternAiStage1TradeWorkload ? (
          <section className="pattern-family-selected-simulation-cadence-section">
            <header>
              <span>Time Between Trades</span>
            </header>
            <div className="pattern-family-selected-simulation-pressure-summary">
              <section className="pattern-family-selected-simulation-pressure-panel pattern-family-selected-simulation-pressure-panel--spacing">
                <header>
                  <span>Overview</span>
                </header>
                <div className="pattern-family-selected-simulation-spacing-context">
                  <div>
                    <span>Measured Gaps</span>
                    <strong>{formatNumber(patternAiStage1TradeCadence?.gap_count)}</strong>
                  </div>
                  <div>
                    <span>Date Range</span>
                    <strong>
                      {formatDate(patternAiStage1TradeCadence?.first_trade_at)} -{' '}
                      {formatDate(patternAiStage1TradeCadence?.last_trade_at)}
                    </strong>
                  </div>
                </div>
                <div className="pattern-family-selected-simulation-spacing-hero">
                  <div>
                    <span>Typical Wait</span>
                    <strong>{formatGapDuration(patternAiStage1TradeCadence?.median_gap_minutes)}</strong>
                    <small>median gap between trade entries</small>
                  </div>
                  <div>
                    <span>Average Wait</span>
                    <strong>{formatGapDuration(patternAiStage1TradeCadence?.avg_gap_minutes)}</strong>
                    <small>average gap between trade entries</small>
                  </div>
                </div>
                <div className="pattern-family-selected-simulation-spacing-range">
                  <div>
                    <span>Fastest repeat</span>
                    <strong>{formatGapDuration(patternAiStage1TradeCadence?.min_gap_minutes)}</strong>
                  </div>
                  <div>
                    <span>Longest pause</span>
                    <strong>{formatGapDuration(patternAiStage1TradeCadence?.max_gap_minutes)}</strong>
                  </div>
                </div>
              </section>
              <section className="pattern-family-selected-simulation-pressure-panel pattern-family-selected-simulation-pressure-panel--buckets">
                <header>
                  <span>Gap Buckets</span>
                </header>
                <div className="pattern-family-selected-simulation-pressure-table pattern-family-selected-simulation-pressure-table--embedded">
                  <table>
                    <thead>
                      <tr>
                        <th>Gap Range</th>
                        <th>Times Seen</th>
                        <th>Share</th>
                      </tr>
                    </thead>
                    <tbody>
                      {patternAiStage1TradeCadenceBuckets.map((bucket) => (
                        <tr key={bucket.label}>
                          <td>{bucket.label}</td>
                          <td>{formatNumber(bucket.count)}</td>
                          <td>{formatDecimal(bucket.percent, 1)}%</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </section>
            </div>
            <section className="pattern-family-selected-simulation-cadence-section pattern-family-selected-simulation-cadence-section--workload">
              <header>
                <span>Trade Workload</span>
              </header>
              {patternAiStage1TradeWorkloadCards.length ? (
                <div className="pattern-family-selected-simulation-workload-board">
                  {patternAiStage1TradeWorkloadCards.map((card) => (
                    <article
                      className={`pattern-family-selected-simulation-workload-tile pattern-family-selected-simulation-workload-tile--${card.key}`}
                      key={card.key}
                    >
                      <header>
                        <span>{card.title}</span>
                      </header>
                      <div className="pattern-family-selected-simulation-workload-average">
                        <strong>{card.value}</strong>
                        <span>{card.unit}</span>
                      </div>
                      <div className="pattern-family-selected-simulation-workload-stat-list">
                        {card.stats.map((stat) => (
                          <div key={`${card.key}-${stat.label}`}>
                            <span>{stat.label}</span>
                            <strong>{stat.value}</strong>
                          </div>
                        ))}
                      </div>
                    </article>
                  ))}
                </div>
              ) : (
                <div className="pattern-family-selected-playbook-empty">
                  {isPatternAiStage1TradeLoading
                    ? 'Loading trade workload...'
                    : patternAiStage1TradeError || 'No trade workload data loaded.'}
                </div>
              )}
            </section>
          </section>
        ) : (
          <div className="pattern-family-selected-playbook-empty">
            {isPatternAiStage1TradeLoading
              ? 'Loading trade cadence...'
              : patternAiStage1TradeError || 'No trade cadence data loaded.'}
          </div>
        )}
      </details>
      <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-loss-cluster pattern-family-selected-simulation-chart--collapsible">
        <summary>
          <span>Loss Behavior</span>
          <small>
            {isPatternAiStage1TradeLoading
              ? 'Loading loss behavior'
              : patternAiStage1LossWindowRows.length
                ? `${formatNumber(patternAiStage1LossWindowRows.length)} loss-heavy hours`
                : patternAiStage1TradeError || 'No loss behavior loaded'}
          </small>
        </summary>
        {patternAiStage1LossWindowRows.length ? (
          <div className="pattern-family-selected-simulation-contribution-table">
            {renderAiStage1LossWindowsTable()}
          </div>
        ) : (
          <div className="pattern-family-selected-playbook-empty">
            {isPatternAiStage1TradeLoading
              ? 'Loading loss behavior...'
              : patternAiStage1TradeError || 'No loss behavior rows loaded.'}
          </div>
        )}
      </details>
      <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-contribution pattern-family-selected-simulation-chart--collapsible">
        <summary>
          <span>Symbol Contribution</span>
          <small>
            {isPatternAiStage1TradeLoading
              ? 'Loading symbols'
              : patternAiStage1SymbolContributionRows.length
                ? `${formatNumber(patternAiStage1SymbolContributionRows.length)} symbols`
                : patternAiStage1TradeError || 'No symbol contribution loaded'}
          </small>
        </summary>
        {patternAiStage1SymbolContributionRows.length ? (
          <>
            <div className="pattern-family-selected-simulation-equity-stats">
              {[
                {
                  label: 'Top Symbol',
                  value: `${patternAiStage1TopSymbol?.root_symbol || 'N/A'} / ${formatDecimal(
                    patternAiStage1TopSymbol?.sum_r ?? 0,
                    1
                  )}R`,
                  tone: 'win',
                },
                {
                  label: 'Worst Symbol',
                  value: `${patternAiStage1WorstSymbol?.root_symbol || 'N/A'} / ${formatDecimal(
                    patternAiStage1WorstSymbol?.sum_r ?? 0,
                    1
                  )}R`,
                  tone: Number(patternAiStage1WorstSymbol?.sum_r ?? 0) < 0 ? 'loss' : 'skipped',
                },
                {
                  label: 'Symbol Count',
                  value: formatNumber(patternAiStage1SymbolContributionRows.length),
                  tone: 'skipped',
                },
              ].map((item) => (
                <div
                  className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                  key={item.label}
                >
                  <span>{item.label}</span>
                  <strong>{item.value}</strong>
                </div>
              ))}
            </div>
            <div className="pattern-family-selected-simulation-contribution-table">
              {renderAiStage1SymbolContributionTable()}
            </div>
          </>
        ) : (
          <div className="pattern-family-selected-playbook-empty">
            {isPatternAiStage1TradeLoading
              ? 'Loading symbol contribution...'
              : patternAiStage1TradeError || 'No symbol contribution rows loaded.'}
          </div>
        )}
      </details>
      <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-contribution pattern-family-selected-simulation-contribution--family pattern-family-selected-simulation-chart--collapsible">
        <summary>
          <span>Family Contribution</span>
          <small>
            {isPatternAiStage1TradeLoading
              ? 'Loading families'
              : patternAiStage1FamilyContributionRows.length
                ? `${formatNumber(patternAiStage1FamilyContributionRows.length)} families`
                : patternAiStage1TradeError || 'No family contribution loaded'}
          </small>
        </summary>
        {patternAiStage1FamilyContributionRows.length ? (
          <>
            <div className="pattern-family-selected-simulation-equity-stats">
              {[
                {
                  label: 'Top Family',
                  value: `${compactText(patternAiStage1TopFamily?.family_key || 'N/A', 10)} / ${formatDecimal(
                    patternAiStage1TopFamily?.sum_r ?? 0,
                    1
                  )}R`,
                  tone: 'win',
                },
                {
                  label: 'Worst Family',
                  value: `${compactText(patternAiStage1WorstFamily?.family_key || 'N/A', 10)} / ${formatDecimal(
                    patternAiStage1WorstFamily?.sum_r ?? 0,
                    1
                  )}R`,
                  tone: Number(patternAiStage1WorstFamily?.sum_r ?? 0) < 0 ? 'loss' : 'skipped',
                },
                {
                  label: 'Family Count',
                  value: formatNumber(patternAiStage1FamilyContributionRows.length),
                  tone: 'skipped',
                },
              ].map((item) => (
                <div
                  className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                  key={item.label}
                >
                  <span>{item.label}</span>
                  <strong>{item.value}</strong>
                </div>
              ))}
            </div>
            <div className="pattern-family-selected-simulation-contribution-table">
              {renderAiStage1FamilyContributionTable()}
            </div>
          </>
        ) : (
          <div className="pattern-family-selected-playbook-empty">
            {isPatternAiStage1TradeLoading
              ? 'Loading family contribution...'
              : patternAiStage1TradeError || 'No family contribution rows loaded.'}
          </div>
        )}
      </details>
    </>
  );
  const renderSymbolContributionTable = (rows = selectedSimulationSymbolContributionRows) => (
    <table>
      <thead>
        <tr>
          <th>Symbol</th>
          <th>Trades</th>
          <th>WR</th>
          <th>Avg R</th>
          <th>Net R</th>
          <th>Best</th>
          <th>Worst</th>
          <th>Bad-Day</th>
          <th>Families</th>
          <th>Tests</th>
          <th>Contracts</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => {
          const netR = Number(row.sum_r || 0);
          return (
            <tr className={netR >= 0 ? 'is-win' : 'is-loss'} key={row.root_symbol}>
              <td>{row.root_symbol || 'N/A'}</td>
              <td>{formatNumber(row.trades)}</td>
              <td>{formatDecimal(row.win_rate, 1)}%</td>
              <td>{formatDecimal(row.avg_r, 3)}R</td>
              <td>{formatDecimal(netR, 1)}R</td>
              <td>{formatDecimal(row.best_r, 1)}R</td>
              <td>{formatDecimal(row.worst_r, 1)}R</td>
              <td>
                {formatNumber(row.daily_loss_day_trades)} / {formatNumber(row.daily_loss_day_count)}
              </td>
              <td>{formatNumber(row.family_count)}</td>
              <td>{formatNumber(row.template_count)}</td>
              <td>{formatNumber(row.contract_count)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
  const renderFamilyContributionTable = (rows = selectedSimulationFamilyContributionRows, { compactFamily = true } = {}) => (
    <table>
      <thead>
        <tr>
          <th>Family</th>
          <th>Trades</th>
          <th>WR</th>
          <th>Avg R</th>
          <th>Net R</th>
          <th>Best</th>
          <th>Worst</th>
          <th>Bad-Day</th>
          <th>Symbols</th>
          <th>Tests</th>
          <th>Contracts</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => {
          const netR = Number(row.sum_r || 0);
          return (
            <tr className={netR >= 0 ? 'is-win' : 'is-loss'} key={row.family_key}>
              <td title={row.family_key}>{compactFamily ? compactText(row.family_key || 'N/A', 14) : row.family_key || 'N/A'}</td>
              <td>{formatNumber(row.trades)}</td>
              <td>{formatDecimal(row.win_rate, 1)}%</td>
              <td>{formatDecimal(row.avg_r, 3)}R</td>
              <td>{formatDecimal(netR, 1)}R</td>
              <td>{formatDecimal(row.best_r, 1)}R</td>
              <td>{formatDecimal(row.worst_r, 1)}R</td>
              <td>
                {formatNumber(row.daily_loss_day_trades)} / {formatNumber(row.daily_loss_day_count)}
              </td>
              <td>{formatNumber(row.symbol_count)}</td>
              <td>{formatNumber(row.template_count)}</td>
              <td>{formatNumber(row.contract_count)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
  const renderSelectedBuildInfoTable = () => (
    <section className="pattern-family-selected-build-info">
      <header>
        <span>Build Info</span>
        <small>{selectedBuildPrimaryTemplate ? getEntryExitTemplateLabel(selectedBuildPrimaryTemplate) : 'No template loaded'}</small>
      </header>
      <div className="pattern-family-selected-build-info-table-wrap">
        <table className="pattern-family-selected-build-info-table">
          <tbody>
            {selectedBuildInfoRows.map((row) => (
              <tr
                className={[
                  row.wide ? 'pattern-family-selected-build-info-row--wide' : '',
                  row.tone ? `pattern-family-selected-build-info-row--${row.tone}` : '',
                ].filter(Boolean).join(' ')}
                key={row.label}
              >
                <th>{row.label}</th>
                <td title={row.value}>{row.value}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );

  const renderSelectedBuildPanel = () => (
    <section className="pattern-family-selected-build-panel">
      <section className="pattern-family-entry-dashboard-build-strip">
        <div className="pattern-family-entry-dashboard-build-title">
          <span>Selected Build</span>
          <div className="pattern-family-entry-dashboard-build-title-actions">
            <div className="pattern-family-entry-dashboard-build-switch" aria-label="Select build">
              {entryExitModelDatasets.length ? (
                entryExitModelDatasets.map((dataset) => (
                  <button
                    className={
                      dataset.id === selectedBuildRunId
                        ? 'pattern-family-entry-dashboard-build-button pattern-family-entry-dashboard-build-button--active'
                        : 'pattern-family-entry-dashboard-build-button'
                    }
                    key={dataset.id}
                    onClick={() => {
                      setSelectedEntryExitModelDatasetId(dataset.id);
                      setSelectedEntryExitTemplateUid(null);
                      setSelectedEntryExitRouterRunId(null);
                      setSelectedBuildCoverageExchangeKey('');
                      setEntryExitProfileTab('model');
                    }}
                    title={`${dataset.buildLabel} | ${dataset.label}`}
                    type="button"
                  >
                    {dataset.buildLabel}
                  </button>
                ))
              ) : (
                <span>
                  {entryExitBuildListError ||
                    (isEntryExitBuildListLoading ? 'Loading builds' : 'No stored builds')}
                </span>
              )}
            </div>
            <div className="pattern-family-selected-build-tabs" role="tablist" aria-label="Selected build view">
              {[
                { id: 'dashboard', label: 'Dashboard' },
                { id: 'raw', label: 'Raw Rows' },
              ].map((tab) => {
                const isActive = selectedBuildView === tab.id;
                return (
                  <button
                    className={
                      isActive
                        ? 'pattern-family-selected-build-tab pattern-family-selected-build-tab--active'
                        : 'pattern-family-selected-build-tab'
                    }
                    key={tab.id}
                    onClick={() => setSelectedBuildView(tab.id)}
                    role="tab"
                    aria-selected={isActive}
                    type="button"
                  >
                    {tab.label}
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      </section>

      {selectedBuildView === 'raw' ? (
        <section className="pattern-family-selected-playbook-used pattern-family-selected-build-raw">
          <header>
            <span>Final Build Tests</span>
            <small>{formatNumber(displayedEntryExitTemplates.length)} tests</small>
          </header>
          <div className="pattern-family-entry-dashboard-raw-table">
            {displayedEntryExitTemplates.length ? (
              <table>
                <thead>
                  <tr>
                    <th>#</th>
                    <th>Template</th>
                    <th>Rule</th>
                    <th>Dir</th>
                    <th>Entry</th>
                    <th>Risk</th>
                    <th>Target</th>
                    <th>Eval</th>
                    <th>Pass</th>
                    <th>Fail</th>
                    <th>No Entry</th>
                    <th>Avg R</th>
                  </tr>
                </thead>
                <tbody>
                  {displayedEntryExitTemplates.map((template, templateIndex) => {
                    const entryOffset = getTemplateEntryOffset(template);
                    return (
                      <tr
                        key={template.template_uid || `${template.created_from_setup_id}-${templateIndex}`}
                        title={`${template.template_name || 'Template'} | ${template.template_uid || 'N/A'}`}
                      >
                        <td>{formatNumber(templateIndex + 1)}</td>
                        <td title={template.template_uid}>
                          {getEntryExitTemplateLabel(template)}
                        </td>
                        <td title={template.template_name}>
                          {formatEntryExitTemplateRule(template)}
                        </td>
                        <td>
                          {template.direction_mode === 'inverse_pattern' ? 'Inverse' : 'Pattern'}
                        </td>
                        <td>
                          {entryOffset
                            ? `C+${entryOffset}`
                            : formatRouteMode(template.entry_kind)}
                        </td>
                        <td>{formatDecimal(template.risk_multiple, 3)} {getEntryExitTemplateRiskLabel(template)}</td>
                        <td>{formatDecimal(template.target_r, 2)}R</td>
                        <td>{formatNumber(template.eval_count)}</td>
                        <td className="is-win">{formatNumber(template.pass_count)}</td>
                        <td className="is-loss">{formatNumber(template.fail_count)}</td>
                        <td>{formatNumber(template.no_entry_count)}</td>
                        <td className={Number(template.avg_r) < 0 ? 'is-loss' : 'is-win'}>
                          {formatDecimal(template.avg_r, 3)}R
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            ) : (
              <div className="pattern-family-build-coverage-empty">
                No final tests loaded for this build.
              </div>
            )}
          </div>
        </section>
      ) : (
        <>
          {renderSelectedBuildInfoTable()}
          <section className="pattern-family-build-coverage-board pattern-family-build-coverage-board--static">
          <header className="pattern-family-build-coverage-head">
            <div>
              <span>Build Coverage</span>
              <strong>Exchange / symbol rows</strong>
            </div>
            <small>
              {selectedBuildHasStoredSummary
                ? `${formatNumber(selectedBuildCoveragePatternCount)} patterns | ${formatNumber(selectedBuildRootCount)} roots`
                : ''}
            </small>
          </header>
          {entryExitBuildExchangeSections.length ? (
            <>
              <div className="pattern-family-build-coverage-exchange-togglebar" role="tablist" aria-label="Build coverage exchange">
                {entryExitBuildExchangeSections.map((section) => {
                  const exchangeKey = getExchangeClassSuffix(section.exchange);
                  const isActive =
                    selectedBuildCoverageExchangeSection?.exchange === section.exchange;
                  return (
                    <button
                      className={[
                        'pattern-family-build-coverage-exchange-toggle',
                        `pattern-family-build-coverage-exchange-toggle--${exchangeKey}`,
                        isActive ? 'pattern-family-build-coverage-exchange-toggle--active' : '',
                      ].join(' ')}
                      key={section.exchange}
                      onClick={() => setSelectedBuildCoverageExchangeKey(exchangeKey)}
                      role="tab"
                      aria-selected={isActive}
                      type="button"
                    >
                      <span>{section.exchange}</span>
                      <strong>{formatNumber(section.pattern_count)}</strong>
                      <small>{formatNumber(section.scanned_count)} / {formatNumber(section.rows.length)}</small>
                    </button>
                  );
                })}
              </div>
              {selectedBuildCoverageExchangeSection ? (
                <div
                  className={[
                    'pattern-family-build-coverage-table-wrap',
                    `pattern-family-build-coverage-table-wrap--${getExchangeClassSuffix(
                      selectedBuildCoverageExchangeSection.exchange
                    )}`,
                  ].join(' ')}
                >
                  <table className="pattern-family-build-coverage-table">
                    <thead>
                      <tr>
                        <th>Root</th>
                        <th>Scanned</th>
                        <th>Universe</th>
                        <th>Contracts</th>
                        <th>TF</th>
                        <th>Status</th>
                      </tr>
                    </thead>
                    <tbody
                      className={[
                        'pattern-family-build-coverage-table-section',
                        `pattern-family-build-coverage-table-section--${getExchangeClassSuffix(
                          selectedBuildCoverageExchangeSection.exchange
                        )}`,
                      ].join(' ')}
                    >
                      {selectedBuildCoverageExchangeSection.rows.map((row) => (
                        <tr
                          className={[
                            `pattern-family-build-coverage-table-row--${getExchangeClassSuffix(
                              selectedBuildCoverageExchangeSection.exchange
                            )}`,
                            row.is_scanned ? '' : 'pattern-family-build-coverage-table-row--empty',
                          ].join(' ')}
                          key={`${selectedBuildCoverageExchangeSection.exchange}-${row.root_symbol}`}
                          title={`${row.root_symbol} | ${formatNumber(row.pattern_count)} scanned patterns | ${formatNumber(row.universe_pattern_count)} universe patterns | ${formatNumber(row.contract_count)} contracts | ${row.timeframe_label}`}
                        >
                          <td className="pattern-family-build-coverage-table-root">{row.root_symbol}</td>
                          <td>{formatNumber(row.pattern_count)}</td>
                          <td>{formatNumber(row.universe_pattern_count)}</td>
                          <td>{formatNumber(row.contract_count)}</td>
                          <td>{row.timeframe_label}</td>
                          <td>{row.is_scanned ? 'Scanned' : 'Not scanned'}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : null}
            </>
          ) : (
            <div className="pattern-family-build-coverage-empty">
              {isEntryExitLoading
                ? 'Loading stored build coverage rows...'
                : ''}
            </div>
          )}
          </section>
        </>
      )}
    </section>
  );
  const renderSelectedPlaybookPanel = () => (
    <aside
      className={[
        'pattern-family-selected-playbook-panel',
        selectedPlaybookView !== 'dashboard' ? 'pattern-family-selected-playbook-panel--raw' : '',
      ].filter(Boolean).join(' ')}
    >
      <header className="pattern-family-selected-playbook-head">
        <div className="pattern-family-selected-playbook-head-left">
          <span>Selected Playbook</span>
          <div
            className={[
              'pattern-family-build-playbook-link',
              `pattern-family-build-playbook-link--${selectedPlaybookLinkState}`,
            ].join(' ')}
            title={`Build ${selectedPlaybookBuildRunId || 'N/A'} | Playbook ${
              selectedEntryExitPlaybookName || 'N/A'
            }`}
          >
            <strong>{selectedPlaybookBuildLabel}</strong>
            <span aria-hidden="true">-&gt;</span>
            <strong>
              {selectedEntryExitRouterRunRow?.playbookLabel ||
                selectedEntryExitPlaybookName ||
                'P?'}
            </strong>
            <small>{selectedPlaybookLinkStatus}</small>
          </div>
        </div>
        <div className="pattern-family-selected-playbook-head-actions">
          <div className="pattern-family-entry-dashboard-header-switch" aria-label="Select playbook">
            {generatedEntryExitPlaybookTabRows.length ? (
              generatedEntryExitPlaybookTabRows.map((row) => (
                <button
                  className={
                    row.playbookKey === selectedEntryExitPlaybookKey
                      ? 'pattern-family-entry-dashboard-header-button pattern-family-entry-dashboard-header-button--active'
                      : 'pattern-family-entry-dashboard-header-button'
                  }
                  key={row.playbookName}
                  onClick={() => {
                    setSelectedEntryExitRouterRunId(row.run.router_run_id);
                    setEntryExitProfileTab('model');
                  }}
                  title={`${row.playbookLabel} | ${row.playbookName}`}
                  type="button"
                >
                  {row.playbookLabel}
                </button>
              ))
            ) : (
              <strong />
            )}
          </div>
          <div className="pattern-family-selected-simulation-tabs pattern-family-selected-playbook-view-tabs" aria-label="Selected playbook view">
            {[
              { id: 'dashboard', label: 'Dashboard' },
              { id: 'used', label: 'Used Plays' },
              { id: 'symbols', label: 'Symbols' },
              { id: 'raw', label: 'Raw Rows' },
            ].map((tab) => {
              const isActive = selectedPlaybookView === tab.id;
              return (
                <button
                  className={
                    isActive
                      ? 'pattern-family-selected-simulation-tab pattern-family-selected-simulation-tab--active'
                      : 'pattern-family-selected-simulation-tab'
                  }
                  key={tab.id}
                  onClick={() => setSelectedPlaybookView(tab.id)}
                  type="button"
                >
                  {tab.label}
                </button>
              );
            })}
          </div>
        </div>
      </header>
      {selectedPlaybookView === 'raw' ? (
        <section className="pattern-family-selected-playbook-used pattern-family-selected-playbook-raw">
          <header>
            <span>Raw Playbook Rows</span>
            <small>{formatNumber(generatedEntryExitFamilyRouterRows.length)} family/template rows</small>
          </header>
          <div className="pattern-family-entry-dashboard-raw-table">
            {generatedEntryExitFamilyRouterRows.length ? (
              <table>
                <thead>
                  <tr>
                    <th>#</th>
                    <th>Family</th>
                    <th>Template</th>
                    <th>Status</th>
                    <th>Train</th>
                    <th>Train WR</th>
                    <th>Train R</th>
                    <th>Test</th>
                    <th>Test WR</th>
                    <th>Total R</th>
                  </tr>
                </thead>
                <tbody>
                  {generatedEntryExitFamilyRouterRows.map((row) => {
                    const route = row.route;
                    return (
                      <tr key={`${route.family_key}-${route.template_uid}-${route.template_rank}`}>
                        <td>{formatNumber(row.index)}</td>
                        <td title={route.family_key}>
                          {route.harmonic_type || 'Unknown'} | {route.market || 'N/A'} | {route.family_bin || 'Bin'}
                        </td>
                        <td title={route.template_name}>{route.template_label || compactText(route.template_uid, 8)}</td>
                        <td>{route.route_status || 'N/A'}</td>
                        <td>{formatNumber(route.train_eval_count)}</td>
                        <td>{formatRatePercent(route.train_win_rate, 2)}%</td>
                        <td className={Number(route.train_avg_r) < 0 ? 'is-loss' : 'is-win'}>{formatDecimal(route.train_avg_r, 4)}R</td>
                        <td>{formatNumber(row.testEvalCount)}</td>
                        <td>{formatDecimal(row.testPassRate, 2)}%</td>
                        <td className={Number(route.test_sum_r) < 0 ? 'is-loss' : 'is-win'}>{formatDecimal(route.test_sum_r, 2)}R</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            ) : (
              <div className="pattern-family-selected-playbook-empty">No playbook rows loaded.</div>
            )}
          </div>
        </section>
      ) : selectedPlaybookView === 'used' ? (
        <section className="pattern-family-selected-playbook-used pattern-family-selected-playbook-used--view">
          <header>
            <span>Used Plays</span>
            <small>
              {hasSelectedEntryExitRouterRun
                ? `${formatNumber(selectedPlaybookUsedPlayRows.length)} entry/exit templates`
                : ''}
            </small>
          </header>
          <div className="pattern-family-selected-playbook-used-scroll">
            <div className="pattern-family-selected-playbook-list">
              {selectedPlaybookUsedPlayRows.length ? (
                selectedPlaybookUsedPlayRows.map((play) => (
                  <div
                    className="pattern-family-selected-playbook-row"
                    key={play.templateUid}
                    title={`${play.templateUid} | ${play.name}`}
                  >
                    <span>{play.label}</span>
                    <small>{play.name}</small>
                    <strong>{formatNumber(play.familyCount)} families</strong>
                  </div>
                ))
              ) : (
                <div className="pattern-family-selected-playbook-empty">No used plays loaded.</div>
              )}
            </div>
          </div>
        </section>
      ) : selectedPlaybookView === 'symbols' ? (
        <section className="pattern-family-selected-playbook-used pattern-family-selected-playbook-symbols">
          <header>
            <span>Symbols</span>
            <small>
              {selectedPlaybookSymbolRows.length
                ? `${formatNumber(selectedPlaybookTradeSymbolCount)} trade / ${formatNumber(selectedPlaybookSkippedSymbolCount)} skip`
                : 'No symbol gate rows loaded'}
            </small>
          </header>
          <div className="pattern-family-entry-dashboard-raw-table">
            {selectedPlaybookSymbolRows.length ? (
              <table>
                <thead>
                  <tr>
                    <th>Root</th>
                    <th>Status</th>
                    <th>Train Tests</th>
                    <th>Train WR</th>
                    <th>Train Avg R</th>
                    <th>Train Sum R</th>
                    <th>Reason</th>
                  </tr>
                </thead>
                <tbody>
                  {selectedPlaybookSymbolRows.map((symbol) => {
                    const isSkipped = symbol.route_status !== 'TRADE';
                    return (
                      <tr
                        key={`${symbol.router_run_id}-${symbol.root_symbol}`}
                        title={`${symbol.root_symbol} | ${symbol.status_reason || ''}`}
                      >
                        <td className="pattern-family-template-table-id">{symbol.root_symbol || 'N/A'}</td>
                        <td className={isSkipped ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                          {symbol.route_status || 'N/A'}
                        </td>
                        <td>{formatNumber(symbol.train_eval_count)}</td>
                        <td>{formatRatePercent(symbol.train_win_rate, 2)}%</td>
                        <td className={Number(symbol.train_avg_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                          {formatDecimal(symbol.train_avg_r, 4)}R
                        </td>
                        <td className={Number(symbol.train_sum_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                          {formatDecimal(symbol.train_sum_r, 2)}R
                        </td>
                        <td className="pattern-family-template-table-reason">{symbol.status_reason || 'Selected by symbol gate'}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            ) : (
              <div className="pattern-family-selected-playbook-empty">
                No symbol filter rows loaded for this playbook.
              </div>
            )}
          </div>
        </section>
      ) : (
        <div className="pattern-family-selected-playbook-dashboard-body">
          <div className="pattern-family-selected-playbook-logic-grid pattern-family-selected-playbook-dashboard-card-grid">
            {[
              {
                label: 'Plays',
                value: hasSelectedEntryExitRouterRun ? formatNumber(selectedPlaybookPlaysCount) : '',
                detail: 'assigned templates',
              },
              {
                label: 'Trade',
                value: hasSelectedEntryExitRouterRun ? formatNumber(selectedPlaybookTradeCount) : '',
                detail: 'trade families',
              },
              {
                label: 'Watch',
                value: hasSelectedEntryExitRouterRun ? formatNumber(selectedPlaybookWatchCount) : '',
                detail: 'watchlist families',
              },
              {
                label: 'Skipped',
                value: hasSelectedEntryExitRouterRun ? formatNumber(selectedPlaybookSkipCount) : '',
                detail: 'blocked families',
              },
              {
                label: 'Families',
                value: hasSelectedEntryExitRouterRun ? formatNumber(selectedPlaybookRowCount) : '',
                detail: 'playbook rows',
              },
            ].map((item) => (
              <div
                className="pattern-family-selected-playbook-logic-card"
                key={item.label}
              >
                <span>{item.label}</span>
                <strong>{item.value}</strong>
                <small>{item.detail}</small>
              </div>
            ))}
          </div>
          <section className="pattern-family-selected-playbook-assignment-chart">
            <header>
              <div>
                <span>Template Usage</span>
                <small>Build templates assigned into this playbook</small>
              </div>
              <strong>
                {selectedPlaybookTemplateTotal
                  ? `${formatNumber(selectedPlaybookAssignedTemplateCount)} / ${formatNumber(
                      selectedPlaybookTemplateTotal
                    )}`
                  : formatNumber(selectedPlaybookAssignedTemplateCount)}
              </strong>
            </header>
            <div className="pattern-family-selected-playbook-assignment-summary">
              <div className="pattern-family-selected-playbook-logic-card pattern-family-selected-playbook-assignment-summary-card">
                <span>Assigned Templates</span>
                <strong>{formatNumber(selectedPlaybookAssignedTemplateCount)}</strong>
                <small>used by trade families</small>
              </div>
              <div className="pattern-family-selected-playbook-logic-card pattern-family-selected-playbook-assignment-summary-card">
                <span>Unused Templates</span>
                <strong>{selectedPlaybookTemplateTotal ? formatNumber(selectedPlaybookUnassignedTemplateCount) : ''}</strong>
                <small>built but not selected</small>
              </div>
              <div className="pattern-family-selected-playbook-logic-card pattern-family-selected-playbook-assignment-summary-card">
                <span>Assignment Rate</span>
                <strong>{selectedPlaybookTemplateTotal ? `${formatDecimal(selectedPlaybookAssignmentRate, 1)}%` : ''}</strong>
                <small>assigned / total build</small>
              </div>
            </div>
            <div className="pattern-family-selected-playbook-assignment-meter">
              <div>
                <span>Overall Assignment Rate</span>
                <strong>{selectedPlaybookTemplateTotal ? `${formatDecimal(selectedPlaybookAssignmentRate, 1)}%` : ''}</strong>
              </div>
              <b aria-hidden="true">
                <i style={{ width: `${Math.max(0, Math.min(100, selectedPlaybookAssignmentRate))}%` }} />
              </b>
            </div>
            <div className="pattern-family-selected-playbook-assignment-bars">
              {selectedPlaybookAssignmentChartRows.length ? (
                selectedPlaybookAssignmentChartRows.map((play) => {
                  const familyCount = Number(play.familyCount || 0);
                  const barWidth = (familyCount / selectedPlaybookAssignmentMaxFamilyCount) * 100;
                  return (
                    <div
                      className="pattern-family-selected-playbook-assignment-bar-row"
                      key={play.templateUid}
                      title={`${play.label} | ${play.name} | ${formatNumber(familyCount)} families`}
                    >
                      <span>{play.label}</span>
                      <div>
                        <i style={{ width: `${Math.max(2, Math.min(100, barWidth))}%` }} />
                      </div>
                      <strong>{formatNumber(familyCount)}</strong>
                    </div>
                  );
                })
              ) : (
                <div className="pattern-family-selected-playbook-empty">No assigned templates loaded.</div>
              )}
            </div>
          </section>
          <section className="pattern-family-selected-playbook-description">
            <header>
              <span>Strategy Description</span>
              <small>{selectedEntryExitPlaybookRunRow?.run.router_run_id ?? ''}</small>
            </header>
            {selectedEntryExitPlaybookRuleSections.length ? (
              <div className="pattern-family-selected-playbook-rule-sections">
                {selectedEntryExitPlaybookRuleSections.map((section) => (
                  <article className="pattern-family-selected-playbook-rule-section" key={section.title}>
                    <header>
                      <span>{section.title}</span>
                      <small>{section.detail}</small>
                    </header>
                    <div className="pattern-family-selected-playbook-rule-grid">
                      {section.items.map((item) => (
                        <div
                          className={[
                            'pattern-family-selected-playbook-logic-card',
                            'pattern-family-selected-playbook-rule-card',
                            item.tone ? `pattern-family-selected-playbook-logic-card--${item.tone}` : '',
                          ].filter(Boolean).join(' ')}
                          key={`${section.title}-${item.label}`}
                        >
                          <span>{item.label}</span>
                          <strong title={item.value}>{item.value}</strong>
                          <small title={item.detail}>{item.detail}</small>
                        </div>
                      ))}
                    </div>
                  </article>
                ))}
              </div>
            ) : (
              <p>No playbook description available for this row yet.</p>
            )}
          </section>
          <section className="pattern-family-selected-playbook-rule-section pattern-family-selected-playbook-rule-impact">
            <header>
              <span>Rule Impact</span>
            </header>
            <div className="pattern-family-selected-playbook-rule-grid">
              {[
                {
                  label: 'Overlap Skips',
                  value: hasSelectedEntryExitLogicRun ? formatNumber(selectedEntryExitLogicRunRow?.run.skipped_overlap_patterns || 0) : '',
                  detail: hasSelectedEntryExitLogicRun ? 'trade gate' : '',
                  tone: Number(selectedEntryExitLogicRunRow?.run.skipped_overlap_patterns || 0) ? 'skipped' : '',
                },
                {
                  label: 'Symbol Gate',
                  value: selectedPlaybookSymbolGateLabel,
                  detail: selectedPlaybookSymbolGateDetail,
                  tone: selectedEntryExitLogicRunRow?.run.symbol_filter_enabled ? 'skipped' : '',
                },
                {
                  label: 'Symbol Skips',
                  value: hasSelectedEntryExitLogicRun ? formatNumber(selectedEntryExitLogicRunRow?.run.skipped_symbol_patterns || 0) : '',
                  detail: hasSelectedEntryExitLogicRun ? 'symbol gate' : '',
                  tone: Number(selectedEntryExitLogicRunRow?.run.skipped_symbol_patterns || 0) ? 'skipped' : '',
                },
              ].map((item) => (
                <div
                  className={[
                    'pattern-family-selected-playbook-logic-card',
                    'pattern-family-selected-playbook-rule-card',
                    item.tone ? `pattern-family-selected-playbook-logic-card--${item.tone}` : '',
                  ].filter(Boolean).join(' ')}
                  key={item.label}
                >
                  <span>{item.label}</span>
                  <strong title={item.value}>{item.value}</strong>
                  <small title={item.detail}>{item.detail}</small>
                </div>
              ))}
            </div>
          </section>
        </div>
      )}
    </aside>
  );

  const renderSelectedSimulationPanel = () => (
                        <aside
                          className={[
                            'pattern-family-selected-playbook-panel',
                            'pattern-family-selected-simulation-panel',
                          ].filter(Boolean).join(' ')}
                        >
                          <header className="pattern-family-selected-playbook-head">
                            <span>Simulation Testing</span>
                            <div className="pattern-family-selected-simulation-head-actions">
                              <div className="pattern-family-selected-simulation-head-row pattern-family-selected-simulation-head-row--mode">
                                <div className="pattern-family-selected-simulation-station-buttons" aria-label="Simulation mode">
                                  {[
                                    { id: 'propFirm', label: 'Prop Firm' },
                                    { id: 'dayTrading', label: 'Day Trading' },
                                  ].map((mode) => (
                                    <button
                                      className={
                                        entryExitSimulationMode === mode.id
                                          ? 'pattern-family-selected-simulation-station-button pattern-family-selected-simulation-station-button--active'
                                          : 'pattern-family-selected-simulation-station-button'
                                      }
                                      key={mode.id}
                                      onClick={() => setEntryExitSimulationMode(mode.id)}
                                      type="button"
                                    >
                                      {mode.label}
                                    </button>
                                  ))}
                                </div>
                                <div className="pattern-family-selected-simulation-year-tabs" aria-label="Simulation test year">
                                  {selectedPlaybookTestYearRows.length ? (
                                    selectedPlaybookTestYearRows.map((yearRow) => (
                                      <button
                                        className={
                                          selectedEntryExitSimulationYear === yearRow.key
                                            ? 'pattern-family-selected-simulation-year-tab pattern-family-selected-simulation-year-tab--active'
                                            : 'pattern-family-selected-simulation-year-tab'
                                        }
                                        key={yearRow.key}
                                        onClick={() => setSelectedEntryExitSimulationYearKey(yearRow.key)}
                                        title={`${yearRow.label} simulation test`}
                                        type="button"
                                      >
                                        {yearRow.label}
                                      </button>
                                    ))
                                  ) : (
                                    <span>No Year</span>
                                  )}
                                </div>
                                <div className="pattern-family-selected-simulation-tabs" aria-label="Simulation testing view">
                                  {[
                                    { id: 'overview', label: 'Overview' },
                                    { id: 'plays', label: 'Sim Plays' },
                                    { id: 'rawTrades', label: 'Raw Trades' },
                                    { id: 'aiTrades', label: 'AI Trades' },
                                  ].map((tab) => (
                                    <button
                                      className={
                                        entryExitSimulationTab === tab.id
                                          ? 'pattern-family-selected-simulation-tab pattern-family-selected-simulation-tab--active'
                                          : 'pattern-family-selected-simulation-tab'
                                      }
                                      key={tab.id}
                                      onClick={() => setEntryExitSimulationTab(tab.id)}
                                      type="button"
                                    >
                                      {tab.label}
                                    </button>
                                  ))}
                                </div>
                              </div>
                            </div>
                          </header>
                          <div className={`pattern-family-selected-simulation-body pattern-family-selected-simulation-body--${entryExitSimulationTab}`}>
                            {entryExitSimulationTab === 'overview' ? (
                              <>
                                {entryExitSimulationMode === 'dayTrading' ? (
                                  <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-day-trading pattern-family-selected-simulation-chart--collapsible">
                                    <summary>
                                      <span>Day Trading Simulation</span>
                                      <small>
                                        {isEntryExitDayTradingSimLoading
                                          ? 'Loading day trading summary'
                                          : selectedDayTradingSummary
                                            ? `${formatNumber(selectedDayTradingSummary.total_trades)} trades | ${formatDecimal(selectedDayTradingSummary.total_r, 1)}R`
                                            : entryExitDayTradingSimError || 'No day trading summary loaded'}
                                      </small>
                                    </summary>
                                    {selectedDayTradingSummary ? (
                                      <div className="pattern-family-selected-simulation-day-trading-grid">
                                        {[
                                          {
                                            label: 'Net R',
                                            value: `${formatDecimal(selectedDayTradingSummary.total_r, 1)}R`,
                                            detail: `${formatDecimal(selectedDayTradingSummary.avg_r, 3)}R avg trade`,
                                            tone: selectedDayTradingSummary.total_r >= 0 ? 'win' : 'loss',
                                          },
                                          {
                                            label: 'Win Rate',
                                            value: `${formatDecimal(selectedDayTradingSummary.win_rate, 1)}%`,
                                            detail: `${formatNumber(selectedDayTradingSummary.wins)}W / ${formatNumber(selectedDayTradingSummary.losses)}L`,
                                            tone: selectedDayTradingSummary.win_rate >= 50 ? 'win' : 'loss',
                                          },
                                          {
                                            label: 'Max Drawdown',
                                            value: `${formatDecimal(selectedDayTradingSummary.max_drawdown_r, 1)}R`,
                                            detail: `${formatDecimal(selectedDayTradingSummary.peak_equity_r, 1)}R peak`,
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Profit Factor',
                                            value: formatDecimal(selectedDayTradingSummary.profit_factor, 2),
                                            detail: `${formatDecimal(selectedDayTradingSummary.gross_profit_r, 1)}R gross win`,
                                            tone: selectedDayTradingSummary.profit_factor >= 1 ? 'win' : 'loss',
                                          },
                                          {
                                            label: 'Trading Days',
                                            value: formatNumber(selectedDayTradingSummary.trading_days),
                                            detail: `${formatNumber(selectedDayTradingSummary.profitable_days)} green / ${formatNumber(selectedDayTradingSummary.losing_days)} red`,
                                          },
                                          {
                                            label: 'Avg Day',
                                            value: `${formatDecimal(selectedDayTradingSummary.avg_day_r, 2)}R`,
                                            detail: `${formatDecimal(selectedDayTradingSummary.best_day_r, 1)}R best / ${formatDecimal(selectedDayTradingSummary.worst_day_r, 1)}R worst`,
                                            tone: selectedDayTradingSummary.avg_day_r >= 0 ? 'win' : 'loss',
                                          },
                                          {
                                            label: 'Max Trades / Day',
                                            value: formatNumber(selectedDayTradingSummary.max_trades_per_day),
                                            detail: `${formatNumber(selectedDayTradingSummary.total_trades)} total trades`,
                                          },
                                          {
                                            label: 'Trade Duration',
                                            value: formatGapDuration(selectedDayTradingSummary.median_trade_duration_minutes),
                                            detail: `${formatGapDuration(selectedDayTradingSummary.avg_trade_duration_minutes)} avg`,
                                          },
                                        ].map((item) => (
                                          <div
                                            className={[
                                              'pattern-family-selected-simulation-day-trading-card',
                                              item.tone ? `pattern-family-selected-simulation-day-trading-card--${item.tone}` : '',
                                            ].filter(Boolean).join(' ')}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                            <small>{item.detail}</small>
                                          </div>
                                        ))}
                                      </div>
                                    ) : (
                                      <div className="pattern-family-selected-playbook-empty">
                                        {isEntryExitDayTradingSimLoading
                                          ? 'Loading day trading simulation...'
                                          : entryExitDayTradingSimError || 'No day trading simulation run has been created yet.'}
                                      </div>
                                    )}
                                  </details>
                                ) : (
                                  <>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-overview pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Playbook Test Overview</span>
                                    <small>{selectedEntryExitSimulationTestId || 'No simulation test selected'}</small>
                                  </summary>
                                  {selectedSimulationRunOverviewSections.length ? (
                                    <div className="pattern-family-selected-simulation-run-overview pattern-family-selected-simulation-run-overview--playbook">
                                      {selectedSimulationRunOverviewSections.map((section) => (
                                        <section
                                          className={[
                                            'pattern-family-selected-simulation-run-section',
                                            section.layout ? `pattern-family-selected-simulation-run-section--${section.layout}` : '',
                                          ].filter(Boolean).join(' ')}
                                          key={section.title}
                                        >
                                          <header>
                                            <span>{section.title}</span>
                                          </header>
                                          <div className="pattern-family-selected-simulation-run-grid">
                                            {section.items.map((item) => (
                                              <div
                                                className={[
                                                  'pattern-family-selected-simulation-run-card',
                                                  item.wide ? 'pattern-family-selected-simulation-run-card--wide' : '',
                                                  item.tone ? `pattern-family-selected-simulation-run-card--${item.tone}` : '',
                                                ].filter(Boolean).join(' ')}
                                                key={`${section.title}-${item.label}`}
                                              >
                                                <span>{item.label}</span>
                                                <strong title={item.title || item.value}>{item.value}</strong>
                                              </div>
                                            ))}
                                          </div>
                                        </section>
                                      ))}
                                    </div>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">No simulation run row loaded.</div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-overview pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Prop Simulation Overview</span>
                                    <small>{selectedEntryExitSimulationTestId || 'No simulation test selected'}</small>
                                  </summary>
                                  {selectedPropSimulationOverviewSections.length ? (
                                    <div className="pattern-family-selected-simulation-run-overview">
                                      {selectedPropSimulationOverviewSections.map((section) => (
                                        <section className="pattern-family-selected-simulation-run-section" key={section.title}>
                                          <header>
                                            <span>{section.title}</span>
                                          </header>
                                          <div className="pattern-family-selected-simulation-run-grid">
                                            {section.items.map((item) => (
                                              <div
                                                className={[
                                                  'pattern-family-selected-simulation-run-card',
                                                  item.tone ? `pattern-family-selected-simulation-run-card--${item.tone}` : '',
                                                ].filter(Boolean).join(' ')}
                                                key={`${section.title}-${item.label}`}
                                              >
                                                <span>{item.label}</span>
                                                <strong title={item.value}>{item.value}</strong>
                                              </div>
                                            ))}
                                          </div>
                                        </section>
                                      ))}
                                      <section className="pattern-family-selected-simulation-run-section pattern-family-selected-simulation-run-section--outcomes">
                                        <header>
                                          <span>Prop Cycle Outcomes</span>
                                        </header>
                                        {selectedSimulationOutcomeSegments.length ? (
                                          <>
                                            <div
                                              className="pattern-family-selected-simulation-outcome-bar"
                                              aria-label="Prop cycle outcome distribution"
                                            >
                                              {selectedSimulationOutcomeSegments.map((segment) => (
                                                <div
                                                  className={`pattern-family-selected-simulation-outcome-segment pattern-family-selected-simulation-outcome-segment--${segment.tone}`}
                                                  key={segment.label}
                                                  style={{ width: `${Math.max(segment.percent, segment.value ? 3 : 0)}%` }}
                                                  title={`${segment.label}: ${formatNumber(segment.value)} (${formatDecimal(segment.percent, 1)}%)`}
                                                />
                                              ))}
                                            </div>
                                            <div className="pattern-family-selected-simulation-outcome-grid">
                                              <div className="pattern-family-selected-simulation-outcome-card">
                                                <span>Total Cycles</span>
                                                <strong>{formatNumber(selectedSimulationOutcomeTotal)}</strong>
                                                <small>100.0%</small>
                                              </div>
                                              {selectedSimulationOutcomeSegments.map((segment) => (
                                                <div
                                                  className={`pattern-family-selected-simulation-outcome-card pattern-family-selected-simulation-outcome-card--${segment.tone}`}
                                                  key={segment.label}
                                                >
                                                  <span>{segment.label}</span>
                                                  <strong>{formatNumber(segment.value)}</strong>
                                                  <small>{formatDecimal(segment.percent, 1)}%</small>
                                                </div>
                                              ))}
                                            </div>
                                          </>
                                        ) : (
                                          <div className="pattern-family-selected-playbook-empty">No prop cycle outcome data loaded.</div>
                                        )}
                                      </section>
                                    </div>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">No prop simulation summary loaded.</div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-templates pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Template Performance</span>
                                    <small>
                                      {hasSelectedEntryExitRouterRun
                                        ? `${formatNumber(selectedSimulationTemplatePerformanceRows.length)} templates ranked by total R`
                                        : ''}
                                    </small>
                                  </summary>
                                  <div className="pattern-family-selected-simulation-template-table">
                                    {selectedSimulationTemplatePerformanceRows.length ? (
                                      <table>
                                        <thead>
                                          <tr>
                                            <th>Template</th>
                                            <th>Rule</th>
                                            <th>Tests</th>
                                            <th>W</th>
                                            <th>L</th>
                                            <th>No Entry</th>
                                            <th>Win %</th>
                                            <th>Avg R</th>
                                            <th>Total R</th>
                                            <th>Families</th>
                                          </tr>
                                        </thead>
                                        <tbody>
                                          {selectedSimulationTemplatePerformanceRows.map((play) => (
                                            <tr key={play.templateUid} title={`${play.templateUid} | ${play.name}`}>
                                              <td>{play.label}</td>
                                              <td>{play.name}</td>
                                              <td>{formatNumber(play.evalCount)}</td>
                                              <td className="is-win">{formatNumber(play.passCount)}</td>
                                              <td className="is-loss">{formatNumber(play.failCount)}</td>
                                              <td>{formatNumber(play.noEntryCount)}</td>
                                              <td className={Number(play.winRate) >= 50 ? 'is-win' : 'is-warning'}>
                                                {formatDecimal(play.winRate, 1)}%
                                              </td>
                                              <td className={Number(play.avgR) < 0 ? 'is-loss' : 'is-win'}>
                                                {formatDecimal(play.avgR, 3)}R
                                              </td>
                                              <td className={Number(play.sumR) < 0 ? 'is-loss' : 'is-win'}>
                                                {formatDecimal(play.sumR, 2)}R
                                              </td>
                                              <td>{formatNumber(play.familyCount)}</td>
                                            </tr>
                                          ))}
                                        </tbody>
                                      </table>
                                    ) : (
                                      <div className="pattern-family-selected-playbook-empty">No template performance loaded.</div>
                                    )}
                                  </div>
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-equity pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Equity Curve In R</span>
                                    <small>
                                      {isEntryExitSimEquityLoading
                                        ? 'Loading curve'
                                        : selectedSimulationEquityPoints.length
                                          ? `${formatNumber(selectedSimulationEquityPoints.length)} points`
                                          : entryExitSimEquityError || 'No curve loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationEquityPoints.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Current R',
                                            value: `${formatDecimal(selectedSimulationEquityLast?.cumulative_r ?? 0, 1)}R`,
                                            tone: Number(selectedSimulationEquityLast?.cumulative_r ?? 0) < 0 ? 'loss' : 'win',
                                          },
                                          {
                                            label: 'Peak R',
                                            value: `${formatDecimal(selectedSimulationEquityPeak, 1)}R`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Worst DD',
                                            value: `${formatDecimal(selectedSimulationEquityWorstDrawdown, 1)}R`,
                                            tone: 'loss',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <svg
                                        className="pattern-family-selected-simulation-equity-svg"
                                        viewBox={`0 0 ${selectedSimulationEquityWidth} ${selectedSimulationEquityHeight}`}
                                        role="img"
                                        aria-label="Cumulative R equity curve"
                                        preserveAspectRatio="none"
                                      >
                                        <line
                                          className="pattern-family-selected-simulation-equity-zero"
                                          x1={selectedSimulationEquityPadding.left}
                                          x2={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y1={selectedSimulationEquityZeroY}
                                          y2={selectedSimulationEquityZeroY}
                                        />
                                        <path
                                          className="pattern-family-selected-simulation-equity-line"
                                          d={selectedSimulationEquityPath}
                                        />
                                        <circle
                                          className="pattern-family-selected-simulation-equity-dot"
                                          cx={getSelectedSimulationEquityX(selectedSimulationEquityPoints.length - 1)}
                                          cy={getSelectedSimulationEquityY(selectedSimulationEquityLast?.cumulative_r ?? 0)}
                                          r="4"
                                        />
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityPadding.top + 4}
                                          textAnchor="end"
                                        >
                                          {formatNumber(Math.round(selectedSimulationEquityMax))}R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityHeight - selectedSimulationEquityPadding.bottom + 4}
                                          textAnchor="end"
                                        >
                                          {formatNumber(Math.round(selectedSimulationEquityMin))}R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityPadding.left}
                                          y={selectedSimulationEquityHeight - 8}
                                        >
                                          {selectedSimulationEquityStartLabel}
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y={selectedSimulationEquityHeight - 8}
                                          textAnchor="end"
                                        >
                                          {selectedSimulationEquityEndLabel}
                                        </text>
                                      </svg>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimEquityLoading
                                        ? 'Loading equity curve...'
                                        : entryExitSimEquityError || 'No equity curve data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-daily-r pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Daily R Bars</span>
                                    <small>
                                      {isEntryExitSimDailyRLoading
                                        ? 'Loading daily bars'
                                        : selectedSimulationDailyRows.length
                                          ? `${formatNumber(selectedSimulationDailyRows.length)} days | -${formatDecimal(
                                              selectedSimulationDailyLossLimit,
                                              0
                                            )}R limit`
                                          : entryExitSimDailyRError || 'No daily R loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationDailyRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Best Day',
                                            value: `${formatDecimal(selectedSimulationBestDay?.total_r ?? 0, 1)}R`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Worst Day',
                                            value: `${formatDecimal(selectedSimulationWorstDay?.total_r ?? 0, 1)}R`,
                                            tone:
                                              Number(selectedSimulationWorstDay?.worst_intraday_r ?? 0) <=
                                              -selectedSimulationDailyLossLimit
                                                ? 'loss'
                                                : 'skipped',
                                          },
                                          {
                                            label: 'Daily Hits',
                                            value: formatNumber(selectedSimulationDailyLossHitRows.length),
                                            tone: selectedSimulationDailyLossHitRows.length ? 'loss' : 'win',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <svg
                                        className="pattern-family-selected-simulation-equity-svg pattern-family-selected-simulation-daily-r-svg"
                                        viewBox={`0 0 ${selectedSimulationEquityWidth} ${selectedSimulationEquityHeight}`}
                                        role="img"
                                        aria-label="Daily net R bars"
                                        preserveAspectRatio="none"
                                      >
                                        <line
                                          className="pattern-family-selected-simulation-equity-zero"
                                          x1={selectedSimulationEquityPadding.left}
                                          x2={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y1={selectedSimulationDailyZeroY}
                                          y2={selectedSimulationDailyZeroY}
                                        />
                                        <line
                                          className="pattern-family-selected-simulation-daily-r-limit"
                                          x1={selectedSimulationEquityPadding.left}
                                          x2={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y1={selectedSimulationDailyLossLimitY}
                                          y2={selectedSimulationDailyLossLimitY}
                                        />
                                        {selectedSimulationDailyRows.map((day, index) => {
                                          const totalR = Number(day.total_r || 0);
                                          const isPositive = totalR >= 0;
                                          const barValueY = getSelectedSimulationDailyY(totalR);
                                          const barY = isPositive ? barValueY : selectedSimulationDailyZeroY;
                                          const barHeight = Math.max(1, Math.abs(selectedSimulationDailyZeroY - barValueY));
                                          const hitDailyLoss =
                                            day.hit_daily_loss ||
                                            Number(day.worst_intraday_r || 0) <= -selectedSimulationDailyLossLimit;
                                          const tradeDate = String(day.trade_date || '').slice(0, 10);
                                          const isSelectedDay = tradeDate === selectedSimulationDailyDate;

                                          return (
                                            <g key={`${tradeDate}-${index}`}>
                                              <rect
                                                className={[
                                                  'pattern-family-selected-simulation-daily-r-bar',
                                                  isPositive
                                                    ? 'pattern-family-selected-simulation-daily-r-bar--win'
                                                    : 'pattern-family-selected-simulation-daily-r-bar--loss',
                                                  hitDailyLoss
                                                    ? 'pattern-family-selected-simulation-daily-r-bar--hit'
                                                    : '',
                                                  isSelectedDay
                                                    ? 'pattern-family-selected-simulation-daily-r-bar--selected'
                                                    : '',
                                                ]
                                                  .filter(Boolean)
                                                  .join(' ')}
                                                x={getSelectedSimulationDailyX(index)}
                                                y={barY}
                                                width={selectedSimulationDailyBarWidth}
                                                height={barHeight}
                                                onClick={() => setSelectedSimulationDailyDate(tradeDate)}
                                              >
                                                <title>
                                                  {`${tradeDate} | Net ${formatDecimal(totalR, 2)}R | ${formatNumber(
                                                    day.wins
                                                  )}W / ${formatNumber(day.losses)}L | Trades ${formatNumber(
                                                    day.trades
                                                  )} | Best ${formatDecimal(day.best_trade_r, 2)}R | Worst ${formatDecimal(
                                                    day.worst_trade_r,
                                                    2
                                                  )}R | Intraday low ${formatDecimal(day.worst_intraday_r, 2)}R | TP level ${formatDecimal(
                                                    day.tp_progress_pct,
                                                    1
                                                  )}%`}
                                                </title>
                                              </rect>
                                              {hitDailyLoss ? (
                                                <circle
                                                  className="pattern-family-selected-simulation-daily-r-hit-dot"
                                                  cx={getSelectedSimulationDailyX(index) + selectedSimulationDailyBarWidth / 2}
                                                  cy={selectedSimulationDailyLossLimitY}
                                                  r="3"
                                                />
                                              ) : null}
                                            </g>
                                          );
                                        })}
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityPadding.top + 4}
                                          textAnchor="end"
                                        >
                                          {formatDecimal(selectedSimulationDailyMax, 0)}R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-daily-r-limit-label"
                                          x={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y={selectedSimulationDailyLossLimitY - 8}
                                          textAnchor="end"
                                        >
                                          -10R daily loss
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityHeight - selectedSimulationEquityPadding.bottom + 4}
                                          textAnchor="end"
                                        >
                                          {formatDecimal(selectedSimulationDailyMin, 0)}R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityPadding.left}
                                          y={selectedSimulationEquityHeight - 8}
                                        >
                                          {selectedSimulationDailyStartLabel}
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y={selectedSimulationEquityHeight - 8}
                                          textAnchor="end"
                                        >
                                          {selectedSimulationDailyEndLabel}
                                        </text>
                                      </svg>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimDailyRLoading
                                        ? 'Loading daily R bars...'
                                        : entryExitSimDailyRError || 'No daily R data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-drawdown pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Daily Drawdown Bars</span>
                                    <small>
                                      {isEntryExitSimDailyRLoading
                                        ? 'Loading drawdown bars'
                                        : selectedSimulationDailyRows.length
                                          ? `${formatNumber(selectedSimulationDailyRows.length)} days | ${formatDecimal(
                                              selectedSimulationDailyLossLimit,
                                              0
                                            )}R daily limit`
                                          : entryExitSimDailyRError || 'No drawdown loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationDailyRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Current DD',
                                            value: `${formatDecimal(selectedSimulationDrawdownCurrent, 1)}R`,
                                            tone: selectedSimulationDrawdownCurrent >= selectedSimulationDrawdownLimit ? 'loss' : 'skipped',
                                          },
                                          {
                                            label: 'Worst Day DD',
                                            value: `${formatDecimal(
                                              Math.max(0, -Number(selectedSimulationWorstDrawdownDay?.worst_intraday_r || 0)),
                                              1
                                            )}R`,
                                            tone:
                                              Math.max(0, -Number(selectedSimulationWorstDrawdownDay?.worst_intraday_r || 0)) >=
                                              selectedSimulationDailyLossLimit
                                                ? 'loss'
                                                : 'skipped',
                                          },
                                          {
                                            label: 'Daily Hits',
                                            value: formatNumber(selectedSimulationDailyLossHitRows.length),
                                            tone: selectedSimulationDailyLossHitRows.length ? 'loss' : 'win',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <svg
                                        className="pattern-family-selected-simulation-equity-svg pattern-family-selected-simulation-daily-r-svg pattern-family-selected-simulation-drawdown-svg"
                                        viewBox={`0 0 ${selectedSimulationEquityWidth} ${selectedSimulationEquityHeight}`}
                                        role="img"
                                        aria-label="Daily drawdown bars in R"
                                        preserveAspectRatio="none"
                                      >
                                        <line
                                          className="pattern-family-selected-simulation-equity-zero"
                                          x1={selectedSimulationEquityPadding.left}
                                          x2={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y1={selectedSimulationDailyDrawdownZeroY}
                                          y2={selectedSimulationDailyDrawdownZeroY}
                                        />
                                        <line
                                          className="pattern-family-selected-simulation-daily-r-limit"
                                          x1={selectedSimulationEquityPadding.left}
                                          x2={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y1={selectedSimulationDailyDrawdownBarLimitY}
                                          y2={selectedSimulationDailyDrawdownBarLimitY}
                                        />
                                        {selectedSimulationDailyRows.map((day, index) => {
                                          const drawdownR = Math.max(0, -Number(day.worst_intraday_r || 0));
                                          const barValueY = getSelectedSimulationDailyDrawdownY(drawdownR);
                                          const barHeight = Math.max(1, Math.abs(selectedSimulationDailyDrawdownZeroY - barValueY));
                                          const hitDailyLoss = day.hit_daily_loss || drawdownR >= selectedSimulationDailyLossLimit;
                                          const tradeDate = String(day.trade_date || '').slice(0, 10);
                                          const isSelectedDay = tradeDate === selectedSimulationDailyDate;

                                          return (
                                            <g key={`${tradeDate}-drawdown-${index}`}>
                                              <rect
                                                className={[
                                                  'pattern-family-selected-simulation-daily-r-bar',
                                                  'pattern-family-selected-simulation-daily-r-bar--loss',
                                                  hitDailyLoss ? 'pattern-family-selected-simulation-daily-r-bar--hit' : '',
                                                  isSelectedDay ? 'pattern-family-selected-simulation-daily-r-bar--selected' : '',
                                                ].filter(Boolean).join(' ')}
                                                x={getSelectedSimulationDailyX(index)}
                                                y={selectedSimulationDailyDrawdownZeroY}
                                                width={selectedSimulationDailyBarWidth}
                                                height={barHeight}
                                                role="button"
                                                tabIndex="0"
                                                onClick={() => setSelectedSimulationDailyDate(tradeDate)}
                                                onKeyDown={(event) => {
                                                  if (event.key === 'Enter' || event.key === ' ') {
                                                    event.preventDefault();
                                                    setSelectedSimulationDailyDate(tradeDate);
                                                  }
                                                }}
                                              >
                                                <title>
                                                  {`${tradeDate} | worst drawdown ${formatDecimal(drawdownR, 2)}R | net ${formatDecimal(
                                                    day.total_r,
                                                    2
                                                  )}R | DD level ${formatDecimal(day.drawdown_progress_pct, 1)}%`}
                                                </title>
                                              </rect>
                                              {hitDailyLoss ? (
                                                <circle
                                                  className="pattern-family-selected-simulation-daily-r-hit-dot"
                                                  cx={getSelectedSimulationDailyX(index) + selectedSimulationDailyBarWidth / 2}
                                                  cy={selectedSimulationDailyDrawdownBarLimitY}
                                                  r="3"
                                                />
                                              ) : null}
                                            </g>
                                          );
                                        })}
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityPadding.top + 4}
                                          textAnchor="end"
                                        >
                                          0R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-daily-r-limit-label"
                                          x={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y={selectedSimulationDailyDrawdownBarLimitY - 8}
                                          textAnchor="end"
                                        >
                                          10R daily loss
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityHeight - selectedSimulationEquityPadding.bottom + 4}
                                          textAnchor="end"
                                        >
                                          {formatDecimal(selectedSimulationDailyDrawdownChartMax, 0)}R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityPadding.left}
                                          y={selectedSimulationEquityHeight - 8}
                                        >
                                          {selectedSimulationDailyStartLabel}
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y={selectedSimulationEquityHeight - 8}
                                          textAnchor="end"
                                        >
                                          {selectedSimulationDailyEndLabel}
                                        </text>
                                      </svg>
                                      <div className="pattern-family-selected-simulation-drawdown-legend">
                                        <span>
                                          <i className="pattern-family-selected-simulation-drawdown-legend-dot pattern-family-selected-simulation-drawdown-legend-dot--drawdown" />
                                          Worst Intraday
                                        </span>
                                        <span>
                                          <i className="pattern-family-selected-simulation-drawdown-legend-dot pattern-family-selected-simulation-drawdown-legend-dot--daily" />
                                          Daily Loss
                                        </span>
                                      </div>
                                      <div className="pattern-family-selected-simulation-daily-trades">
                                        <header>
                                          <span>
                                            {selectedSimulationDailyDate
                                              ? `${selectedSimulationDailyDate} Trades`
                                              : 'Daily Trade Detail'}
                                          </span>
                                          <small>
                                            {selectedSimulationDailyDate
                                              ? `${formatNumber(selectedSimulationDailyTradeRows.length)} trades | ${formatDecimal(
                                                  selectedSimulationDailyRow?.total_r ?? selectedSimulationDailyTradeNetR,
                                                  1
                                                )}R`
                                              : 'No day selected'}
                                          </small>
                                        </header>
                                        {selectedSimulationDailyDate ? (
                                          selectedSimulationDailyTradeRows.length ? (
                                            <div className="pattern-family-selected-simulation-daily-trades-table">
                                              <table>
                                                <thead>
                                                  <tr>
                                                    <th>Time</th>
                                                    <th>Symbol</th>
                                                    <th>Family</th>
                                                    <th>Template</th>
                                                    <th>Dir</th>
                                                    <th>Result</th>
                                                    <th>TP After</th>
                                                    <th>TP Chg</th>
                                                    <th>DD After</th>
                                                    <th>DD Chg</th>
                                                    <th>Exit</th>
                                                  </tr>
                                                </thead>
                                                <tbody>
                                                  {selectedSimulationDailyTradeRows.map((trade) => {
                                                    const resultR = Number(trade.result_r || 0);
                                                    const direction = String(trade.trade_direction || '').toUpperCase();
                                                    return (
                                                      <tr
                                                        className={resultR >= 0 ? 'is-win' : 'is-loss'}
                                                        key={`${trade.id}-${trade.setup_id}`}
                                                      >
                                                        <td>{formatTime(trade.entry_date ?? trade.d_confirm_date)}</td>
                                                        <td>{trade.symbol || 'N/A'}</td>
                                                        <td title={trade.family_key}>{compactText(trade.family_key || 'N/A', 12)}</td>
                                                        <td title={trade.template_uid}>
                                                          {trade.template_label || compactText(trade.template_uid || 'N/A', 8)}
                                                        </td>
                                                        <td>{direction || 'N/A'}</td>
                                                        <td>{formatDecimal(resultR, 2)}R</td>
                                                        <td title={`${formatDecimal(trade.cycle_equity_r_before ?? 0, 2)}R -> ${formatDecimal(trade.cycle_equity_r_after ?? 0, 2)}R`}>
                                                          {trade.tp_progress_pct_after === null || trade.tp_progress_pct_after === undefined
                                                            ? ''
                                                            : `${formatDecimal(trade.tp_progress_pct_after, 1)}%`}
                                                        </td>
                                                        <td>
                                                          {trade.tp_progress_pct_delta === null || trade.tp_progress_pct_delta === undefined
                                                            ? ''
                                                            : formatSignedPercent(trade.tp_progress_pct_delta)}
                                                        </td>
                                                        <td title={`${formatDecimal(trade.cycle_drawdown_r_before ?? 0, 2)}R -> ${formatDecimal(trade.cycle_drawdown_r_after ?? 0, 2)}R`}>
                                                          {trade.drawdown_progress_pct_after === null || trade.drawdown_progress_pct_after === undefined
                                                            ? ''
                                                            : `${formatDecimal(trade.drawdown_progress_pct_after, 1)}%`}
                                                        </td>
                                                        <td>
                                                          {trade.drawdown_progress_pct_delta === null ||
                                                          trade.drawdown_progress_pct_delta === undefined
                                                            ? ''
                                                            : formatSignedPercent(trade.drawdown_progress_pct_delta)}
                                                        </td>
                                                        <td>{formatRouteMode(trade.exit_reason || trade.outcome || 'N/A')}</td>
                                                      </tr>
                                                    );
                                                  })}
                                                </tbody>
                                              </table>
                                            </div>
                                          ) : (
                                            <div className="pattern-family-selected-playbook-empty">
                                              {isEntryExitSimDailyTradesLoading
                                                ? 'Loading daily trades...'
                                                : entryExitSimDailyTradesError || 'No trades loaded for selected day.'}
                                            </div>
                                          )
                                        ) : (
                                          <div className="pattern-family-selected-playbook-empty">No day selected.</div>
                                        )}
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimDailyRLoading
                                        ? 'Loading drawdown bars...'
                                        : entryExitSimDailyRError || 'No drawdown bar data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-hourly pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Time-of-Day Performance</span>
                                    <small>
                                      {isEntryExitSimHourlyLoading
                                        ? 'Loading hours'
                                        : selectedSimulationHourlyRows.length
                                          ? `${formatNumber(selectedSimulationHourlyRows.length)} active hours`
                                          : entryExitSimHourlyError || 'No hourly data loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationHourlyRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Best Hour',
                                            value: `${formatHourLabel(selectedSimulationBestHour?.entry_hour)} / ${formatDecimal(
                                              selectedSimulationBestHour?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Worst Hour',
                                            value: `${formatHourLabel(selectedSimulationWorstHour?.entry_hour)} / ${formatDecimal(
                                              selectedSimulationWorstHour?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Most Bad-Day Trades',
                                            value: `${formatHourLabel(selectedSimulationMostDangerHour?.entry_hour)} / ${formatNumber(
                                              selectedSimulationMostDangerHour?.daily_loss_day_trades ?? 0
                                            )}`,
                                            tone: 'skipped',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-hourly-table">
                                        <table>
                                          <thead>
                                            <tr>
                                              <th>Hour</th>
                                              <th>Trades</th>
                                              <th>WR</th>
                                              <th>Avg R</th>
                                              <th>Net R</th>
                                              <th>Best</th>
                                              <th>Worst</th>
                                              <th>Bad-Day</th>
                                            </tr>
                                          </thead>
                                          <tbody>
                                            {selectedSimulationHourlyRows.map((hour) => {
                                              const netR = Number(hour.sum_r || 0);
                                              return (
                                                <tr className={netR >= 0 ? 'is-win' : 'is-loss'} key={hour.entry_hour}>
                                                  <td>{formatHourLabel(hour.entry_hour)}</td>
                                                  <td>{formatNumber(hour.trades)}</td>
                                                  <td>{formatDecimal(hour.win_rate, 1)}%</td>
                                                  <td>{formatDecimal(hour.avg_r, 3)}R</td>
                                                  <td>{formatDecimal(netR, 1)}R</td>
                                                  <td>{formatDecimal(hour.best_r, 1)}R</td>
                                                  <td>{formatDecimal(hour.worst_r, 1)}R</td>
                                                  <td>
                                                    {formatNumber(hour.daily_loss_day_trades)} /{' '}
                                                    {formatNumber(hour.daily_loss_day_count)}
                                                  </td>
                                                </tr>
                                              );
                                            })}
                                          </tbody>
                                        </table>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimHourlyLoading
                                        ? 'Loading time-of-day performance...'
                                        : entryExitSimHourlyError || 'No hourly performance data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-cadence pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Trade Cadence</span>
                                    <small>
                                      {isEntryExitSimTradeCadenceLoading
                                        ? 'Loading cadence'
                                        : selectedSimulationTradeCadence
                                          ? `${formatNumber(selectedSimulationTradeCadence.trades)} trades | ${formatDecimal(
                                              selectedSimulationTradeCadence.median_gap_minutes,
                                              1
                                            )}m median gap`
                                          : entryExitSimTradeCadenceError || 'No cadence loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationTradeCadence ? (
                                    <>
                                      <section className="pattern-family-selected-simulation-cadence-section">
                                        <header>
                                          <span>Time Between Trades</span>
                                        </header>
                                        <div className="pattern-family-selected-simulation-pressure-summary">
                                          <section className="pattern-family-selected-simulation-pressure-panel pattern-family-selected-simulation-pressure-panel--spacing">
                                            <header>
                                              <span>Overview</span>
                                            </header>
                                            <div className="pattern-family-selected-simulation-spacing-context">
                                              <div>
                                                <span>Measured Gaps</span>
                                                <strong>{formatNumber(selectedSimulationTradeCadence.gap_count)}</strong>
                                              </div>
                                              <div>
                                                <span>Date Range</span>
                                                <strong>
                                                  {formatDate(selectedSimulationTradeCadence.first_trade_at)} -{' '}
                                                  {formatDate(selectedSimulationTradeCadence.last_trade_at)}
                                                </strong>
                                              </div>
                                            </div>
                                            <div className="pattern-family-selected-simulation-spacing-hero">
                                              <div>
                                                <span>Typical Wait</span>
                                                <strong>{formatGapDuration(selectedSimulationTradeCadence.median_gap_minutes)}</strong>
                                                <small>median gap between trade entries</small>
                                              </div>
                                              <div>
                                                <span>Average Wait</span>
                                                <strong>{formatGapDuration(selectedSimulationTradeCadence.avg_gap_minutes)}</strong>
                                                <small>pulled higher by long quiet periods</small>
                                              </div>
                                            </div>
                                            <div className="pattern-family-selected-simulation-spacing-range">
                                              <div>
                                                <span>Fastest repeat</span>
                                                <strong>{formatGapDuration(selectedSimulationTradeCadence.min_gap_minutes)}</strong>
                                              </div>
                                              <div>
                                                <span>Longest pause</span>
                                                <strong>{formatGapDuration(selectedSimulationTradeCadence.max_gap_minutes)}</strong>
                                              </div>
                                            </div>
                                          </section>
                                          <section className="pattern-family-selected-simulation-pressure-panel pattern-family-selected-simulation-pressure-panel--buckets">
                                            <header>
                                              <span>Gap Buckets</span>
                                            </header>
                                            <div className="pattern-family-selected-simulation-pressure-table pattern-family-selected-simulation-pressure-table--embedded">
                                              <table>
                                                <thead>
                                                  <tr>
                                                    <th>Gap Range</th>
                                                    <th>Times Seen</th>
                                                    <th>Share</th>
                                                  </tr>
                                                </thead>
                                                <tbody>
                                                  {selectedSimulationTradeCadenceBuckets.map((bucket) => (
                                                    <tr key={bucket.label}>
                                                      <td>{bucket.label}</td>
                                                      <td>{formatNumber(bucket.count)}</td>
                                                      <td>{formatDecimal(bucket.percent, 1)}%</td>
                                                    </tr>
                                                  ))}
                                                </tbody>
                                              </table>
                                            </div>
                                          </section>
                                        </div>
                                        <div className="pattern-family-selected-simulation-timeline-workload-row">
                                          <div className="pattern-family-selected-simulation-timeline-slot">
                                            {selectedSimulationTradeGapRows.length ? (
                                              <div className="pattern-family-selected-simulation-gap-timeline">
                                                <header>
                                                  <span>Vertical Trade Timeline</span>
                                                  <small>Linear time scale. Empty vertical space represents minutes with no trades.</small>
                                                </header>
                                                <div className="pattern-family-selected-simulation-gap-vertical">
                                                  {selectedSimulationTradeTimelinePoints.slice(0, 600).map((point, index, points) => {
                                                    const gapMinutes = Number(point.gap_minutes || 0);
                                                    const spacerHeight =
                                                      index === 0 ? 0 : Math.max(2, Math.round(gapMinutes * 0.6));
                                                    const tradeDate = formatDate(point.event_at);
                                                    const previousDate = index > 0 ? formatDate(points[index - 1]?.event_at) : '';
                                                    const showDayDivider = index === 0 || tradeDate !== previousDate;
                                                    const showCycleDivider = index > 0 && point.starts_new_cycle;
                                                    return (
                                                      <React.Fragment key={`${point.sequence_number}-${point.event_at}`}>
                                                        {showDayDivider ? (
                                                          <div
                                                            className="pattern-family-selected-simulation-gap-vertical-day"
                                                            style={{ marginTop: index === 0 ? 0 : spacerHeight }}
                                                          >
                                                            <span>{tradeDate}</span>
                                                          </div>
                                                        ) : null}
                                                        {showCycleDivider ? (
                                                          <div
                                                            className="pattern-family-selected-simulation-gap-vertical-cycle"
                                                            style={{ marginTop: showDayDivider ? 0 : spacerHeight }}
                                                          >
                                                            <span>
                                                              End test {formatNumber(point.previous_cycle_number)} / Start test{' '}
                                                              {formatNumber(point.cycle_number)}
                                                            </span>
                                                          </div>
                                                        ) : null}
                                                        <div
                                                          className="pattern-family-selected-simulation-gap-vertical-item"
                                                          style={{ marginTop: showDayDivider || showCycleDivider ? 0 : spacerHeight }}
                                                        >
                                                          <span className="pattern-family-selected-simulation-gap-vertical-time">
                                                            {formatTime(point.event_at)}
                                                          </span>
                                                          <span className="pattern-family-selected-simulation-gap-vertical-dot" />
                                                          <span className="pattern-family-selected-simulation-gap-vertical-gap">
                                                            {point.gap_minutes === null
                                                              ? 'first trade'
                                                              : `${formatGapDuration(gapMinutes)} since previous`}
                                                          </span>
                                                        </div>
                                                      </React.Fragment>
                                                    );
                                                  })}
                                                </div>
                                                <div className="pattern-family-selected-simulation-gap-timeline-legend">
                                                  <span><i /> trade</span>
                                                  <span>scale: 1 minute = 0.6px</span>
                                                  {selectedSimulationTradeTimelinePoints.length > 600 ? (
                                                    <span>showing first 600 of {formatNumber(selectedSimulationTradeTimelinePoints.length)}</span>
                                                  ) : null}
                                                </div>
                                              </div>
                                            ) : (
                                              <div className="pattern-family-selected-playbook-empty">
                                                {isEntryExitSimTradeGapLoading
                                                  ? 'Loading trade gap timeline...'
                                                  : entryExitSimTradeGapError || 'No trade gap timeline data loaded.'}
                                              </div>
                                            )}
                                          </div>
                                          <section className="pattern-family-selected-simulation-cadence-section pattern-family-selected-simulation-cadence-section--workload">
                                            <header>
                                              <span>Trade Workload</span>
                                            </header>
                                            {selectedSimulationTradeWorkloadCards.length ? (
                                              <div className="pattern-family-selected-simulation-workload-board">
                                                {selectedSimulationTradeWorkloadCards.map((card) => (
                                                  <article className={`pattern-family-selected-simulation-workload-tile pattern-family-selected-simulation-workload-tile--${card.key}`} key={card.key}>
                                                    <header>
                                                      <span>{card.title}</span>
                                                    </header>
                                                    <div className="pattern-family-selected-simulation-workload-average">
                                                      <strong>{card.value}</strong>
                                                      <span>{card.unit}</span>
                                                    </div>
                                                    <div className="pattern-family-selected-simulation-workload-stat-list">
                                                      {card.stats.map((stat) => (
                                                        <div key={`${card.key}-${stat.label}`}>
                                                          <span>{stat.label}</span>
                                                          <strong>{stat.value}</strong>
                                                        </div>
                                                      ))}
                                                    </div>
                                                  </article>
                                                ))}
                                              </div>
                                            ) : (
                                              <div className="pattern-family-selected-playbook-empty">
                                                {isEntryExitSimTradeWorkloadLoading
                                                  ? 'Loading trade workload...'
                                                  : entryExitSimTradeWorkloadError || 'No trade workload data loaded.'}
                                              </div>
                                            )}
                                          </section>
                                        </div>
                                      </section>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimTradeCadenceLoading
                                        ? 'Loading trade cadence...'
                                        : entryExitSimTradeCadenceError || 'No trade cadence data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-test-frequency pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Test Frequency</span>
                                    <small>
                                      {isEntryExitSimTestFrequencyLoading
                                        ? 'Loading test frequency'
                                        : selectedSimulationTestFrequencyRows.length
                                          ? `${formatNumber(selectedSimulationTestFrequencyRows.length)} tests | ${formatDecimal(
                                              selectedSimulationAvgTradesPerTest,
                                              1
                                            )} avg trades / test`
                                          : entryExitSimTestFrequencyError || 'No test frequency data loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationTestFrequencyRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Avg Trades / Test',
                                            value: formatDecimal(selectedSimulationAvgTradesPerTest, 1),
                                            tone: 'skipped',
                                          },
                                          {
                                            label: 'Max Trades / Test',
                                            value: formatNumber(selectedSimulationMaxTestTrades),
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Avg Test Length',
                                            value: formatGapDuration(selectedSimulationAvgTestDurationMinutes),
                                            tone: 'skipped',
                                          },
                                          {
                                            label: 'Longest Test',
                                            value: formatGapDuration(selectedSimulationLongestTestMinutes),
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Closed Tests',
                                            value: `${formatNumber(selectedSimulationTestFrequencyCompletedRows.length)} / ${formatNumber(
                                              selectedSimulationTestFrequencyRows.length
                                            )}`,
                                            tone: 'win',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-frequency-table">
                                        <div className="pattern-family-selected-simulation-frequency-scroll" tabIndex={0}>
                                          <table>
                                            <thead>
                                              <tr>
                                                <th>Test</th>
                                                <th>Outcome</th>
                                                <th>Start</th>
                                                <th>End</th>
                                                <th>Length</th>
                                                <th>Days</th>
                                                <th>Trade Days</th>
                                                <th>Trades</th>
                                                <th>Events</th>
                                                <th>Avg / Day</th>
                                                <th>Avg / Hr</th>
                                                <th>Net R</th>
                                                <th>Max DD</th>
                                              </tr>
                                            </thead>
                                            <tbody>
                                              {selectedSimulationTestFrequencyRows.map((test) => {
                                                const netR = Number(test.sum_r || 0);
                                                return (
                                                  <tr className={netR >= 0 ? 'is-win' : 'is-loss'} key={test.cycle_number}>
                                                    <td>{formatNumber(test.cycle_number)}</td>
                                                    <td>{formatTrendLabel(test.outcome)}</td>
                                                    <td>{formatShortDateTime(test.start_at)}</td>
                                                    <td>{formatShortDateTime(test.end_at)}</td>
                                                    <td>{formatGapDuration(test.duration_minutes)}</td>
                                                    <td>{formatNumber(test.calendar_days)}</td>
                                                    <td>{formatNumber(test.active_trade_days)}</td>
                                                    <td>{formatNumber(test.trades)}</td>
                                                    <td>{formatNumber(test.events)}</td>
                                                    <td>{formatDecimal(test.avg_trades_per_active_day, 1)}</td>
                                                    <td>{formatDecimal(test.avg_trades_per_hour, 1)}</td>
                                                    <td>{formatDecimal(netR, 1)}R</td>
                                                    <td>{formatDecimal(test.max_drawdown_r, 1)}R</td>
                                                  </tr>
                                                );
                                              })}
                                            </tbody>
                                          </table>
                                        </div>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimTestFrequencyLoading
                                        ? 'Loading test frequency...'
                                        : entryExitSimTestFrequencyError || 'No test frequency rows loaded for this sim yet.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-loss-cluster pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Loss Behavior</span>
                                    <small>
                                      {isEntryExitSimLossClusterLoading
                                        ? 'Loading loss clustering'
                                        : selectedSimulationLossSummary
                                          ? `${formatNumber(selectedSimulationLossSummary.losses)} losses | ${formatDecimal(
                                              selectedSimulationLossSummary.clustered_60m_rate,
                                              1
                                            )}% within 60m`
                                          : entryExitSimLossClusterError || 'No loss behavior loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationLossSummary ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Max Loss Streak',
                                            value: `${formatNumber(selectedSimulationLossSummary.max_loss_streak)}L`,
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Median Loss Gap',
                                            value: `${formatDecimal(
                                              selectedSimulationLossSummary.median_loss_gap_minutes,
                                              1
                                            )}m`,
                                            tone:
                                              Number(selectedSimulationLossSummary.median_loss_gap_minutes || 0) <= 60
                                                ? 'loss'
                                                : 'win',
                                          },
                                          {
                                            label: 'Same-Hour Pairs',
                                            value: `${formatNumber(
                                              selectedSimulationLossSummary.clustered_60m_loss_pairs
                                            )} / ${formatDecimal(selectedSimulationLossSummary.clustered_60m_rate, 1)}%`,
                                            tone:
                                              Number(selectedSimulationLossSummary.clustered_60m_rate || 0) >= 50
                                                ? 'loss'
                                                : 'skipped',
                                          },
                                          {
                                            label: '5+ Loss Days',
                                            value: `${formatNumber(selectedSimulationLossSummary.loss_days_5_plus)} / ${formatNumber(
                                              selectedSimulationLossSummary.loss_days
                                            )}`,
                                            tone: selectedSimulationLossSummary.loss_days_5_plus ? 'loss' : 'win',
                                          },
                                          {
                                            label: 'Worst Loss Day',
                                            value: `${formatDate(selectedSimulationLossSummary.worst_loss_day)} / ${formatNumber(
                                              selectedSimulationLossSummary.worst_loss_day_losses
                                            )}L`,
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Worst Hour',
                                            value: `${formatDate(selectedSimulationLossSummary.worst_loss_hour_date)} ${formatHourLabel(
                                              selectedSimulationLossSummary.worst_loss_hour ?? 0
                                            )} / ${formatNumber(selectedSimulationLossSummary.worst_loss_hour_losses)}L`,
                                            tone: 'loss',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-cadence-buckets">
                                        {selectedSimulationLossBuckets.map((bucket) => (
                                          <div className="pattern-family-selected-simulation-cadence-bucket" key={bucket.bucket_key}>
                                            <span>{bucket.bucket_label}</span>
                                            <div>
                                              <i style={{ width: `${bucket.gap_count ? Math.max(3, bucket.gap_percent) : 0}%` }} />
                                            </div>
                                            <strong>
                                              {formatNumber(bucket.gap_count)} / {formatDecimal(bucket.gap_percent, 1)}%
                                            </strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-contribution-table">
                                        <table>
                                          <thead>
                                            <tr>
                                              <th>Date</th>
                                              <th>Hour</th>
                                              <th>Losses</th>
                                              <th>Wins</th>
                                              <th>Loss Rate</th>
                                              <th>Net R</th>
                                              <th>Top Symbol</th>
                                              <th>Top Family</th>
                                              <th>Top Test</th>
                                            </tr>
                                          </thead>
                                          <tbody>
                                            {selectedSimulationLossWindows.slice(0, 18).map((row) => (
                                              <tr className="is-loss" key={`${row.trade_date}-${row.entry_hour}`}>
                                                <td>{formatDate(row.trade_date)}</td>
                                                <td>{formatHourLabel(row.entry_hour)}</td>
                                                <td>{formatNumber(row.losses)}</td>
                                                <td>{formatNumber(row.wins)}</td>
                                                <td>{formatDecimal(row.loss_rate, 1)}%</td>
                                                <td>{formatDecimal(row.total_r, 1)}R</td>
                                                <td>{row.top_root_symbol || 'N/A'}</td>
                                                <td title={row.top_family_key}>{compactText(row.top_family_key || 'N/A', 14)}</td>
                                                <td title={row.top_template_uid}>{compactText(row.top_template_uid || 'N/A', 12)}</td>
                                              </tr>
                                            ))}
                                          </tbody>
                                        </table>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimLossClusterLoading
                                        ? 'Loading loss behavior...'
                                        : entryExitSimLossClusterError || 'No loss clustering data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-contribution pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Symbol Contribution</span>
                                    <small>
                                      {isEntryExitSimSymbolContributionLoading
                                        ? 'Loading symbols'
                                        : selectedSimulationSymbolContributionRows.length
                                          ? `${formatNumber(selectedSimulationSymbolContributionRows.length)} symbols`
                                          : entryExitSimSymbolContributionError || 'No symbol contribution loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationSymbolContributionRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Top Symbol',
                                            value: `${selectedSimulationTopSymbol?.root_symbol || 'N/A'} / ${formatDecimal(
                                              selectedSimulationTopSymbol?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Worst Symbol',
                                            value: `${selectedSimulationWorstSymbol?.root_symbol || 'N/A'} / ${formatDecimal(
                                              selectedSimulationWorstSymbol?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: Number(selectedSimulationWorstSymbol?.sum_r ?? 0) < 0 ? 'loss' : 'skipped',
                                          },
                                          {
                                            label: 'Bad-Day Leader',
                                            value: `${selectedSimulationMostDangerSymbol?.root_symbol || 'N/A'} / ${formatNumber(
                                              selectedSimulationMostDangerSymbol?.daily_loss_day_trades ?? 0
                                            )}`,
                                            tone: 'skipped',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-family-chart">
                                        <header>
                                          <span>Top Families By Net R</span>
                                          <small>{formatNumber(selectedSimulationFamilyPerformanceRows.length)} shown</small>
                                        </header>
                                        {selectedSimulationFamilyPerformanceRows.map((row) => {
                                          const barPercent = Math.max(
                                            0,
                                            (Number(row.netR || 0) / selectedSimulationFamilyPerformanceMaxR) * 100
                                          );
                                          return (
                                            <div
                                              className="pattern-family-selected-simulation-family-bar-row"
                                              key={row.family_key}
                                              title={`${row.family_key} | ${row.label}`}
                                            >
                                              <div className="pattern-family-selected-simulation-family-bar-head">
                                                <span>
                                                  #{String(row.rank).padStart(2, '0')} {row.label}
                                                </span>
                                                <strong>{formatDecimal(row.netR, 1)}R</strong>
                                              </div>
                                              <div className="pattern-family-selected-simulation-family-bar-track">
                                                <i style={{ width: `${barPercent ? Math.max(4, barPercent) : 0}%` }} />
                                              </div>
                                              <small>
                                                {formatNumber(row.trades)} trades | {formatDecimal(row.winRate, 1)}% WR |{' '}
                                                {formatDecimal(row.avgR, 3)}R avg
                                              </small>
                                            </div>
                                          );
                                        })}
                                      </div>
                                      <div className="pattern-family-selected-simulation-contribution-table">
                                        <div className="pattern-family-selected-simulation-table-actions">
                                          <span>Symbol Table</span>
                                          <button
                                            type="button"
                                            onClick={() => setExpandedSimulationTable('symbolContribution')}
                                          >
                                            Expand
                                          </button>
                                        </div>
                                        {renderSymbolContributionTable()}
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimSymbolContributionLoading
                                        ? 'Loading symbol contribution...'
                                        : entryExitSimSymbolContributionError || 'No symbol contribution data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-contribution pattern-family-selected-simulation-contribution--family pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Family Contribution</span>
                                    <small>
                                      {isEntryExitSimFamilyContributionLoading
                                        ? 'Loading families'
                                        : selectedSimulationFamilyContributionRows.length
                                          ? `${formatNumber(selectedSimulationFamilyContributionRows.length)} families`
                                          : entryExitSimFamilyContributionError || 'No family contribution loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationFamilyContributionRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Top Family',
                                            value: `${compactText(selectedSimulationTopFamily?.family_key || 'N/A', 10)} / ${formatDecimal(
                                              selectedSimulationTopFamily?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Worst Family',
                                            value: `${compactText(selectedSimulationWorstFamily?.family_key || 'N/A', 10)} / ${formatDecimal(
                                              selectedSimulationWorstFamily?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: Number(selectedSimulationWorstFamily?.sum_r ?? 0) < 0 ? 'loss' : 'skipped',
                                          },
                                          {
                                            label: 'Bad-Day Leader',
                                            value: `${compactText(selectedSimulationMostDangerFamily?.family_key || 'N/A', 10)} / ${formatNumber(
                                              selectedSimulationMostDangerFamily?.daily_loss_day_trades ?? 0
                                            )}`,
                                            tone: 'skipped',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-contribution-table">
                                        <div className="pattern-family-selected-simulation-table-actions">
                                          <span>Family Table</span>
                                          <button
                                            type="button"
                                            onClick={() => setExpandedSimulationTable('familyContribution')}
                                          >
                                            Expand
                                          </button>
                                        </div>
                                        {renderFamilyContributionTable()}
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimFamilyContributionLoading
                                        ? 'Loading family contribution...'
                                        : entryExitSimFamilyContributionError || 'No family contribution data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-streaks pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Win/Loss Streaks</span>
                                    <small>
                                      {isEntryExitSimStreakLoading
                                        ? 'Loading streaks'
                                        : selectedSimulationStreakRows.length
                                          ? `${formatNumber(selectedSimulationStreakRows.length)} streaks`
                                          : entryExitSimStreakError || 'No streaks loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationStreakRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Largest Loss',
                                            value: `${formatNumber(selectedSimulationLargestLossStreak?.streak_length || 0)}L`,
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Largest Win',
                                            value: `${formatNumber(selectedSimulationLargestWinStreak?.streak_length || 0)}W`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Loss Runs',
                                            value: formatNumber(selectedSimulationLossStreakRows.length),
                                            tone: 'skipped',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-streak-chart">
                                        {selectedSimulationStreakDistribution.map((bucket) => (
                                          <div className="pattern-family-selected-simulation-streak-bucket" key={bucket.length}>
                                            <span>{bucket.length}</span>
                                            <div className="pattern-family-selected-simulation-streak-bars">
                                              <div
                                                className="pattern-family-selected-simulation-streak-bar pattern-family-selected-simulation-streak-bar--win"
                                                style={{
                                                  height: `${Math.max(
                                                    bucket.wins
                                                      ? (bucket.wins / selectedSimulationMaxStreakBucketCount) * 100
                                                      : 0,
                                                    bucket.wins ? 8 : 0
                                                  )}%`,
                                                }}
                                                title={`${formatNumber(bucket.wins)} win streaks of length ${bucket.length}`}
                                              />
                                              <div
                                                className="pattern-family-selected-simulation-streak-bar pattern-family-selected-simulation-streak-bar--loss"
                                                style={{
                                                  height: `${Math.max(
                                                    bucket.losses
                                                      ? (bucket.losses / selectedSimulationMaxStreakBucketCount) * 100
                                                      : 0,
                                                    bucket.losses ? 8 : 0
                                                  )}%`,
                                                }}
                                                title={`${formatNumber(bucket.losses)} loss streaks of length ${bucket.length}`}
                                              />
                                            </div>
                                            <small>
                                              {formatNumber(bucket.wins)} / {formatNumber(bucket.losses)}
                                            </small>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-streak-legend">
                                        <span><i className="pattern-family-selected-simulation-streak-dot pattern-family-selected-simulation-streak-dot--win" />Wins</span>
                                        <span><i className="pattern-family-selected-simulation-streak-dot pattern-family-selected-simulation-streak-dot--loss" />Losses</span>
                                        <small>Bucket label = streak length</small>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimStreakLoading
                                        ? 'Loading streak chart...'
                                        : entryExitSimStreakError || 'No streak data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-market-trends pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Market Trend Snapshot</span>
                                    <small>
                                      {isEntryExitSimMarketTrendLoading
                                        ? 'Loading market trends'
                                        : selectedSimulationMarketTrendPerformanceRows.length
                                          ? `${formatNumber(selectedSimulationMarketTrendPerformanceRows.length)} trend buckets | ${formatNumber(
                                              selectedSimulationMarketTrendAlignmentRows.length
                                            )} alignments`
                                          : entryExitSimMarketTrendError || 'No trend snapshot loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationMarketTrendPerformanceRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Best Trend',
                                            value: selectedSimulationBestMarketTrend
                                              ? `${selectedSimulationBestMarketTrend.timeframe} ${formatTrendLabel(
                                                  selectedSimulationBestMarketTrend.trend_label
                                                )}`
                                              : 'N/A',
                                            detail: selectedSimulationBestMarketTrend
                                              ? `${formatDecimal(selectedSimulationBestMarketTrend.sum_r, 1)}R net`
                                              : '',
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Best Alignment',
                                            value: selectedSimulationBestMarketTrendAlignment
                                              ? `${formatTrendLabel(
                                                  selectedSimulationBestMarketTrendAlignment.trend_5m
                                                )} / ${formatTrendLabel(
                                                  selectedSimulationBestMarketTrendAlignment.trend_15m
                                                )} / ${formatTrendLabel(
                                                  selectedSimulationBestMarketTrendAlignment.trend_1h
                                                )}`
                                              : 'N/A',
                                            detail: selectedSimulationBestMarketTrendAlignment
                                              ? `${formatDecimal(selectedSimulationBestMarketTrendAlignment.sum_r, 1)}R net`
                                              : '',
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Trend Samples',
                                            value: formatNumber(
                                              selectedSimulationMarketTrendPerformanceRows.reduce(
                                                (total, row) => total + Number(row.trades || 0),
                                                0
                                              )
                                            ),
                                            detail: '3 timeframes per trade',
                                            tone: 'skipped',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                            title={item.detail}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                            {item.detail ? <small>{item.detail}</small> : null}
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-market-trend-grid">
                                        <div className="pattern-family-selected-simulation-contribution-table pattern-family-selected-simulation-market-trend-table">
                                          <table>
                                            <thead>
                                              <tr>
                                                <th>Timeframe</th>
                                                <th>Trend</th>
                                                <th>Trades</th>
                                                <th>WR</th>
                                                <th>Avg R</th>
                                                <th>Net R</th>
                                                <th>Strength</th>
                                                <th>Symbols</th>
                                              </tr>
                                            </thead>
                                            <tbody>
                                              {selectedSimulationMarketTrendPerformanceRows.map((row) => {
                                                const netR = Number(row.sum_r || 0);
                                                return (
                                                  <tr
                                                    className={netR >= 0 ? 'is-win' : 'is-loss'}
                                                    key={`${row.timeframe}-${row.trend_label}`}
                                                  >
                                                    <td>{row.timeframe}</td>
                                                    <td>{formatTrendLabel(row.trend_label)}</td>
                                                    <td>{formatNumber(row.trades)}</td>
                                                    <td>{formatDecimal(row.win_rate, 1)}%</td>
                                                    <td>{formatDecimal(row.avg_r, 3)}R</td>
                                                    <td>{formatDecimal(netR, 1)}R</td>
                                                    <td>{formatDecimal(row.avg_strength_pct, 1)}%</td>
                                                    <td>{formatNumber(row.symbol_count)}</td>
                                                  </tr>
                                                );
                                              })}
                                            </tbody>
                                          </table>
                                        </div>
                                        <div className="pattern-family-selected-simulation-contribution-table pattern-family-selected-simulation-market-trend-table">
                                          <table>
                                            <thead>
                                              <tr>
                                                <th>5m</th>
                                                <th>15m</th>
                                                <th>1h</th>
                                                <th>Trades</th>
                                                <th>WR</th>
                                                <th>Avg R</th>
                                                <th>Net R</th>
                                                <th>Best</th>
                                                <th>Worst</th>
                                              </tr>
                                            </thead>
                                            <tbody>
                                              {selectedSimulationMarketTrendAlignmentRows.map((row) => {
                                                const netR = Number(row.sum_r || 0);
                                                return (
                                                  <tr
                                                    className={netR >= 0 ? 'is-win' : 'is-loss'}
                                                    key={`${row.trend_5m}-${row.trend_15m}-${row.trend_1h}`}
                                                  >
                                                    <td>{formatTrendLabel(row.trend_5m)}</td>
                                                    <td>{formatTrendLabel(row.trend_15m)}</td>
                                                    <td>{formatTrendLabel(row.trend_1h)}</td>
                                                    <td>{formatNumber(row.trades)}</td>
                                                    <td>{formatDecimal(row.win_rate, 1)}%</td>
                                                    <td>{formatDecimal(row.avg_r, 3)}R</td>
                                                    <td>{formatDecimal(netR, 1)}R</td>
                                                    <td>{formatDecimal(row.best_r, 1)}R</td>
                                                    <td>{formatDecimal(row.worst_r, 1)}R</td>
                                                  </tr>
                                                );
                                              })}
                                            </tbody>
                                          </table>
                                        </div>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimMarketTrendLoading
                                        ? 'Loading market trend snapshot...'
                                        : entryExitSimMarketTrendError || 'No market trend data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-direction-trends pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Trade Direction vs HTF Trend</span>
                                    <small>
                                      {isEntryExitSimMarketTrendLoading
                                        ? 'Loading direction trend data'
                                        : selectedSimulationMarketTrendDirectionRows.length
                                          ? `${formatNumber(selectedSimulationMarketTrendDirectionRows.length)} direction buckets`
                                          : entryExitSimMarketTrendError || 'No direction trend data loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationMarketTrendDirectionRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {selectedSimulationMarketTrendDirectionSummaryRows.map((row) => {
                                          const tone =
                                            row.trend_alignment === 'against_trend'
                                              ? 'loss'
                                              : row.trend_alignment === 'with_trend'
                                                ? 'win'
                                                : 'skipped';
                                          return (
                                            <div
                                              className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${tone}`}
                                              key={row.trend_alignment}
                                            >
                                              <span>{formatTrendAlignmentLabel(row.trend_alignment)}</span>
                                              <strong>{formatDecimal(row.avg_r, 3)}R</strong>
                                              <small>
                                                {formatDecimal(row.win_rate, 1)}% WR | {formatDecimal(row.sum_r, 1)}R
                                              </small>
                                            </div>
                                          );
                                        })}
                                      </div>
                                      <div className="pattern-family-selected-simulation-contribution-table pattern-family-selected-simulation-direction-trend-table">
                                        <table>
                                          <thead>
                                            <tr>
                                              <th>Timeframe</th>
                                              <th>Side</th>
                                              <th>Trend</th>
                                              <th>Alignment</th>
                                              <th>Trades</th>
                                              <th>Pass</th>
                                              <th>Fail</th>
                                              <th>WR</th>
                                              <th>Avg R</th>
                                              <th>Net R</th>
                                            </tr>
                                          </thead>
                                          <tbody>
                                            {selectedSimulationMarketTrendDirectionRows.map((row) => {
                                              const netR = Number(row.sum_r || 0);
                                              return (
                                                <tr
                                                  className={netR >= 0 ? 'is-win' : 'is-loss'}
                                                  key={`${row.timeframe}-${row.trade_direction}-${row.trend_label}-${row.trend_alignment}`}
                                                >
                                                  <td>{row.timeframe}</td>
                                                  <td>{row.trade_direction}</td>
                                                  <td>{formatTrendLabel(row.trend_label)}</td>
                                                  <td>{formatTrendAlignmentLabel(row.trend_alignment)}</td>
                                                  <td>{formatNumber(row.trades)}</td>
                                                  <td>{formatNumber(row.wins)}</td>
                                                  <td>{formatNumber(row.losses)}</td>
                                                  <td>{formatDecimal(row.win_rate, 1)}%</td>
                                                  <td>{formatDecimal(row.avg_r, 3)}R</td>
                                                  <td>{formatDecimal(netR, 1)}R</td>
                                                </tr>
                                              );
                                            })}
                                          </tbody>
                                        </table>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimMarketTrendLoading
                                        ? 'Loading direction trend data...'
                                        : entryExitSimMarketTrendError || 'No direction trend data loaded.'}
                                    </div>
                                  )}
                                </details>
                                  </>
                                )}
                              </>
                            ) : entryExitSimulationTab === 'aiTrades' ? (
                              renderAiStage1TradesPanel()
                            ) : entryExitSimulationTab === 'rawTrades' ? (
                              <section className="pattern-family-selected-playbook-used pattern-family-selected-simulation-raw-trades">
                                <header>
                                  <span>Raw Trades</span>
                                  <small>
                                    {selectedSimulationRawTradeTotal
                                      ? `${formatNumber(selectedSimulationRawTradeTotal)} trades | showing ${formatNumber(selectedSimulationRawTradeRows.length)}`
                                      : selectedEntryExitSimulationTestId || ''}
                                  </small>
                                </header>
                                <div className="pattern-family-entry-dashboard-raw-table pattern-family-selected-simulation-raw-trades-table">
                                  {selectedSimulationRawTradeRows.length ? (
                                    <table>
                                      <thead>
                                        <tr>
                                          <th>#</th>
                                          <th>Time</th>
                                          <th>Symbol</th>
                                          <th>TF</th>
                                          <th>Family</th>
                                          <th>Play</th>
                                          <th>Dir</th>
                                          <th>R</th>
                                          <th>Outcome</th>
                                          <th>Exit</th>
                                          <th>Duration</th>
                                          <th>Entry</th>
                                          <th>Stop</th>
                                          <th>Target</th>
                                          <th>Exit Px</th>
                                          <th>TP %</th>
                                          <th>DD %</th>
                                        </tr>
                                      </thead>
                                      <tbody>
                                        {selectedSimulationRawTradeRows.map((trade, index) =>
                                          renderSimulationRawTradeRow(trade, index)
                                        )}
                                      </tbody>
                                    </table>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimRawTradesLoading
                                        ? 'Loading raw trades...'
                                        : entryExitSimRawTradesError || 'No raw trades loaded for this simulation.'}
                                    </div>
                                  )}
                                </div>
                              </section>
                            ) : (
                              <section className="pattern-family-selected-playbook-used pattern-family-selected-simulation-plays">
                                <header>
                                  <span>Sim Plays</span>
                                  <small>
                                    {hasSelectedEntryExitRouterRun
                                      ? `${formatNumber(selectedSimulationUsedPlayRows.length)} evaluated templates`
                                      : ''}
                                  </small>
                                </header>
                                <div className="pattern-family-selected-playbook-list">
                                  {selectedSimulationUsedPlayRows.length ? (
                                    selectedSimulationUsedPlayRows.map((play) => (
                                      <div
                                        className="pattern-family-selected-playbook-row"
                                        key={play.templateUid}
                                        title={`${play.templateUid} | ${play.name} | ${formatNumber(play.passCount)}W / ${formatNumber(
                                          play.failCount
                                        )}L / ${formatNumber(play.noEntryCount)} no entry`}
                                      >
                                        <span>{play.label}</span>
                                        <small>
                                          {formatDecimal(play.winRate, 1)}% WR | {formatDecimal(play.avgR, 3)}R |{' '}
                                          {formatNumber(play.familyCount)} families
                                        </small>
                                        <strong>{formatNumber(play.evalCount)} tests</strong>
                                      </div>
                                    ))
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">No simulation plays loaded.</div>
                                  )}
                                </div>
                              </section>
                            )}
                          </div>
                        </aside>
  );

  return (
    <div
      className={[
        'pattern-family-page',
        'pattern-family-terminal',
        CANVAS_ONLY_MODE ? 'pattern-family-page--live-cockpit' : '',
        isEntryExitStandalone ? 'pattern-family-page--flat' : '',
      ].filter(Boolean).join(' ')}
    >
      {!isEntryExitStandalone && !CANVAS_ONLY_MODE ? (
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
            <span>{selectedSourceLabel} / {selectedTimeframeLabel}</span>
            <strong>{yearFilter === 'All' ? 'ALL YEARS' : yearFilter}</strong>
          </div>
        </header>
      ) : null}

      {error ? <div className="pattern-family-error">{error}</div> : null}

      <div
        className={[
          'pattern-family-workspace',
          'pattern-family-one-page',
          isEntryExitStandalone ? 'pattern-family-workspace--entry-exit-only' : '',
          isInspectorCollapsed ? 'pattern-family-one-page--inspector-collapsed' : '',
          CANVAS_ONLY_MODE ? 'pattern-family-workspace--canvas-only' : '',
        ].filter(Boolean).join(' ')}
      >
        <main
          className={[
            'pattern-family-table-stack',
            'pattern-family-table-stack--selection-hidden',
            isSelectedDataFullyCollapsed
              ? 'pattern-family-table-stack--selected-minimized'
              : isSelectedDataCollapsed
                ? 'pattern-family-table-stack--selected-collapsed'
                : '',
          ].filter(Boolean).join(' ')}
        >
          {false ? (
          <section
            className={[
              'pattern-family-selected-shell',
              isSelectedDataCollapsed ? 'pattern-family-selected-shell--collapsed' : '',
              isSelectedDataFullyCollapsed ? 'pattern-family-selected-shell--minimized' : '',
            ].filter(Boolean).join(' ')}
          >
            <div className="pattern-family-section-bar pattern-family-selected-collapse-bar">
              <span className="pattern-family-section-bar-title">Selection Deck</span>
              <span className="pattern-family-section-bar-line" aria-hidden="true" />
              <button
                aria-label={selectionDeckToggleLabel}
                title={selectionDeckToggleLabel}
                type="button"
                onClick={() => setSelectedDataCollapseLevel((current) => (current + 1) % 3)}
              >
                <span
                  className={
                    isSelectedDataFullyCollapsed
                      ? 'pattern-family-selected-collapse-arrow pattern-family-selected-collapse-arrow--down'
                      : 'pattern-family-selected-collapse-arrow pattern-family-selected-collapse-arrow--up'
                  }
                />
              </button>
            </div>
            <div className="pattern-family-selected-columns">
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
                className={isPatternCardHovered ? 'pattern-family-selected-card--keyboard-hover' : ''}
                emptyText={familyPatternsError || (selectedFamily ? 'No family patterns loaded.' : 'Select a family before choosing a pattern.')}
                index={2}
                isLoading={isFamilyPatternsLoading}
                label="Pattern"
                loadingText="Loading family patterns..."
                metrics={selectedPatternRowMetrics}
                onMouseEnter={() => setPatternCardHovered(true)}
                onMouseLeave={() => setPatternCardHovered(false)}
                onAction={() => {
                  if (patternBrowseScope === 'selected') {
                    setPatternBrowseRows(familyPatterns);
                    setPatternBrowseMeta({ totalCount: familyPatterns.length, hasMore: false });
                  }
                  setPatternBrowseError('');
                  setBrowsePanel('patterns');
                }}
                status={
                  familyPatternsError ||
                  (appliedPatternNavigation
                    ? `Nav ${formatNumber(patternNavigationRows.length)} patterns`
                    : `${formatNumber(familyPatterns.length)} patterns`)
                }
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
                  selectedTradeSource
                    ? `${formatTradeDirection(selectedTradeSource)} / ${formatDate(selectedTradeEntryDate)} to ${formatDate(selectedTradeExitDate)}`
                    : 'Choose a stored trade'
                }
                title={selectedTradeIdDisplay === 'Replay only' ? null : selectedTradeIdDisplay}
                tone={selectedTradeIsLoss ? 'loss' : selectedTradeSource ? 'win' : 'trade'}
              />
            </div>
          </section>
          ) : null}

          <section className="pattern-family-sim-panel">
            <div className="pattern-family-section-bar pattern-family-data-center-bar">
              <span className="pattern-family-section-bar-title">Data Center</span>
              <span className="pattern-family-section-bar-line" aria-hidden="true" />
              <strong
                className="pattern-family-section-bar-context"
                title="Pattern Library"
              >
                Pattern Library
              </strong>
            </div>
            <div className="pattern-family-sim-body">
              {testOverviewTab === 'patterns' ? (
                <div className="pattern-family-test-overview">
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
                  {testOverviewTab === 'build' ? (
                    <div className="pattern-family-test-overview-build">
                      {renderSelectedBuildPanel()}
                    </div>
                  ) : testOverviewTab === 'playbook' ? (
                    <div className="pattern-family-test-overview-build pattern-family-test-overview-playbook">
                      {renderSelectedPlaybookPanel()}
                    </div>
                  ) : testOverviewTab === 'simTesting' ? (
                    <div className="pattern-family-test-overview-build pattern-family-test-overview-sim-testing">
                      {renderSelectedSimulationPanel()}
                    </div>
                  ) : (
                    <div
                      className={[
                        'pattern-family-test-overview-grid',
                        testOverviewTab === 'entryExit' ? 'pattern-family-test-overview-grid--entry-exit' : '',
                      ].filter(Boolean).join(' ')}
                    >
                      {testOverviewTab === 'entryExit' && (isEntryExitLoading || isEntryExitRouterLoading) ? (
                        <div className="pattern-family-data-loading-popover">
                          {isEntryExitLoading
                            ? 'Loading build data...'
                            : 'Loading playbook data...'}
                        </div>
                      ) : null}
                      {activeTestOverviewSections.map((section) => {
                        const usesNativeDetails = ['patternXaOutcomeTable', 'patternXaFamilyTable', 'patternReversalAiTable', 'patternReversalAiBuckets', 'patternReversalAiThresholds', 'patternAiStage1TradeTable', 'patternAiExitModelTradeTable'].includes(section.variant);
                        const sectionCollapseKey = `${testOverviewTab}:${section.variant || 'cards'}:${section.title}`;
                        const isDataCenterSectionCollapsed = dataCenterCollapsedSections[sectionCollapseKey] !== false;
                        const dataCenterSectionStatus =
                          section.variant === 'patternXaOutcomeTable'
                            ? isPatternXaOutcomeLoading
                              ? 'Loading rows'
                              : section.tableRows?.length
                                ? `${formatNumber(section.tableRows.length)} loaded / ${formatNumber(patternXaOutcomeData.totalRows)} total`
                                : patternXaOutcomeError || 'No rows loaded'
                            : section.variant === 'patternXaFamilyTable'
                              ? isPatternXaOutcomeLoading
                                ? 'Loading families'
                                : section.tableRows?.length
                                  ? `${formatNumber(section.tableRows.length)} families`
                                  : patternXaOutcomeError || 'No family rows loaded'
                              : section.variant === 'patternReversalAiThresholds'
                                ? isPatternReversalAiLoading
                                  ? 'Loading thresholds'
                                  : section.tableRows?.length
                                    ? `${formatNumber(section.tableRows.length)} cutoffs`
                                    : patternReversalAiError || 'No threshold rows loaded'
                                : section.variant === 'patternReversalAiBuckets'
                                  ? isPatternReversalAiLoading
                                    ? 'Loading buckets'
                                    : section.tableRows?.length
                                      ? `${formatNumber(section.tableRows.length)} buckets`
                                      : patternReversalAiError || 'No bucket rows loaded'
                                  : section.variant === 'patternReversalAiTable'
                                    ? isPatternReversalAiLoading
                                      ? 'Loading rows'
                                      : section.tableRows?.length
                                        ? `${formatNumber(section.tableRows.length)} loaded / ${formatNumber(patternReversalAiData.totalRows)} total`
                                        : patternReversalAiError || 'No AI rows loaded'
                                    : section.variant === 'patternAiStage1TradeTable'
                                      ? isPatternAiExitModelTakenTradeLoading
                                        ? 'Loading trades'
                                        : section.tableRows?.length
                                          ? `${formatNumber(section.tableRows.length)} loaded / ${formatNumber(patternAiExitModelTakenTradeData.totalRows)} total`
                                          : patternAiExitModelTakenTradeError || 'No 180-bar AI exit trade rows loaded'
                                    : section.variant === 'patternAiExitModelTradeTable'
                                      ? isPatternAiExitModelTradeLoading
                                        ? 'Loading exit audit'
                                        : section.tableRows?.length
                                          ? `${formatNumber(section.tableRows.length)} changed / ${formatNumber(patternAiExitModelTradeData.totalRows)} total`
                                          : patternAiExitModelTradeError || 'No exit model rows loaded'
                                    : '';
                        return (
                        <section
                          className={[
                            'pattern-family-test-overview-section',
                            isDataCenterSectionCollapsed ? 'pattern-family-test-overview-section--collapsed' : '',
                            section.wide ? 'pattern-family-test-overview-section--wide' : '',
                            ['templateTable', 'routerRunTable', 'familyRouterTable'].includes(section.variant) ? 'pattern-family-test-overview-section--table' : '',
                            ['patternXaOutcomeTable', 'patternXaFamilyTable', 'patternReversalAiTable', 'patternReversalAiBuckets', 'patternReversalAiThresholds', 'patternAiStage1TradeTable', 'patternAiExitModelTradeTable'].includes(section.variant) ? 'pattern-family-test-overview-section--table pattern-family-test-overview-section--collapsible' : '',
                            ['patternXaOutcomeTable', 'patternXaFamilyTable', 'patternReversalAiTable', 'patternReversalAiBuckets', 'patternReversalAiThresholds', 'patternAiStage1TradeTable', 'patternAiExitModelTradeTable'].includes(section.variant) ? 'pattern-family-test-overview-section--summary-only' : '',
                            ['patternXaOutcomeTable', 'patternXaFamilyTable', 'patternReversalAiTable', 'patternReversalAiBuckets', 'patternReversalAiThresholds', 'patternAiStage1TradeTable', 'patternAiExitModelTradeTable'].includes(section.variant) ? 'pattern-family-test-overview-section--xa-expanded-table' : '',
                            section.variant === 'templateTable' ? 'pattern-family-test-overview-section--scan' : '',
                            section.variant === 'familyRouterTable' ? 'pattern-family-test-overview-section--playbook' : '',
                          ].filter(Boolean).join(' ')}
                          key={section.title}
                        >
                        <header>
                          <button
                            aria-expanded={!isDataCenterSectionCollapsed}
                            className="pattern-family-test-overview-section-toggle"
                            onClick={() => toggleDataCenterSection(sectionCollapseKey)}
                            type="button"
                          >
                            <span>{section.title}</span>
                          </button>
                          {usesNativeDetails ? (
                            <small className="pattern-family-test-overview-section-status">
                              {dataCenterSectionStatus}
                            </small>
                          ) : null}
                          {section.variant === 'templateTable' ? (
                            <div className="pattern-family-model-dataset-tabs pattern-family-build-header-tabs" aria-label="Entry / Exit builds">
                              {entryExitModelDatasets.length ? (
                                entryExitModelDatasets.map((dataset) => {
                                  const isDatasetSelected = dataset.id === selectedBuildRunId;
                                  return (
                                    <button
                                      className={[
                                        'pattern-family-model-dataset-tab',
                                        isDatasetSelected ? 'pattern-family-model-dataset-tab--active' : '',
                                      ].filter(Boolean).join(' ')}
                                      key={dataset.id}
                                      title={`${dataset.buildLabel} | ${dataset.label} | ${dataset.detail}`}
                                      onClick={() => {
                                        setSelectedEntryExitModelDatasetId(dataset.id);
                                        setSelectedEntryExitTemplateUid(null);
                                        setSelectedEntryExitRouterRunId(null);
                                        setEntryExitProfileTab('model');
                                      }}
                                      type="button"
                                    >
                                      <strong>{dataset.buildLabel}</strong>
                                    </button>
                                  );
                                })
                              ) : (
                                <span>
                                  {entryExitBuildListError ||
                                    (isEntryExitBuildListLoading ? 'Loading builds' : 'No stored builds')}
                                </span>
                              )}
                            </div>
                          ) : null}
                          {section.variant === 'familyRouterTable' ? (
                            <div className="pattern-family-playbook-tabs pattern-family-playbook-header-tabs" aria-label="Playbooks used">
                              {generatedEntryExitPlaybookTabRows.length ? (
                                generatedEntryExitPlaybookTabRows.map((row) => (
                                  <button
                                    className={[
                                      'pattern-family-playbook-tab',
                                      row.playbookName === selectedEntryExitRouterRunRow?.playbookName ? 'pattern-family-playbook-tab--active' : '',
                                    ].filter(Boolean).join(' ')}
                                    key={row.playbookName}
                                    onClick={() => {
                                      setSelectedEntryExitRouterRunId(row.run.router_run_id);
                                      setEntryExitProfileTab('model');
                                      setSelectedDataCollapseLevel(2);
                                      setInspectorCollapsed(true);
                                    }}
                                    title={`${row.playbookLabel} | ${row.playbookName} | ${row.run.router_run_id}`}
                                    type="button"
                                  >
                                    <strong>{row.playbookLabel}</strong>
                                  </button>
                                ))
                              ) : (
                                <div className="pattern-family-playbook-tabs-empty">No playbooks loaded</div>
                              )}
                            </div>
                          ) : null}
                        </header>
                        {isDataCenterSectionCollapsed ? null : (
                        <div className="pattern-family-test-overview-section-body">
                        {section.variant === 'emptyPanel' ? (
                          <div className="pattern-family-test-overview-empty">
                            {section.emptyText ?? 'No data loaded yet.'}
                          </div>
                        ) : section.variant === 'patternXaOutcomeTable' ? (
                          <details className="pattern-family-test-overview-collapsible-section" open>
                            <summary>
                              <span>Pattern Table</span>
                              <small>
                                {isPatternXaOutcomeLoading
                                  ? 'Loading rows'
                                  : section.tableRows?.length
                                    ? `${formatNumber(section.tableRows.length)} loaded / ${formatNumber(patternXaOutcomeData.totalRows)} total`
                                    : patternXaOutcomeError || 'No rows loaded'}
                              </small>
                            </summary>
                            <div
                              className="pattern-family-template-table-wrap pattern-family-template-table-wrap--xa-pattern"
                              onKeyDown={handlePatternXaOutcomeTableKeyDown}
                              onMouseDown={(event) => {
                                if (!event.target.closest('button')) {
                                  event.currentTarget.focus();
                                }
                              }}
                              ref={patternXaOutcomeTableWrapRef}
                              tabIndex={0}
                            >
                              {section.tableRows?.length ? (
                                <table className="pattern-family-template-table pattern-family-template-table--router">
                                  <thead>
                                    <tr>
                                      <th>#</th>
                                      <th>Time</th>
                                      <th>Symbol</th>
                                      <th>Side</th>
                                      <th>Outcome</th>
                                      <th>Hit</th>
                                      <th>Bars</th>
                                      <th>Minutes</th>
                                      <th>D</th>
                                      <th>XA</th>
                                      <th>Rev Target</th>
                                      <th>Cont Target</th>
                                      <th>Rev Max</th>
                                      <th>Cont Max</th>
                                      <th>Family</th>
                                      <th>Trade ID</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    {section.tableRows.map((row) => {
                                      const rowKey = getPatternXaOutcomeRowKey(row);
                                      const outcomeClass =
                                        row.outcome === 'reversal_xa'
                                          ? 'pattern-family-template-table-win'
                                          : row.outcome === 'continuation_xa'
                                            ? 'pattern-family-template-table-loss'
                                            : 'pattern-family-template-table-skipped';
                                      return (
                                        <tr
                                          className={
                                            rowKey === selectedPatternXaOutcomeRowKey
                                              ? 'pattern-family-template-table-row--selected'
                                              : ''
                                          }
                                          key={rowKey}
                                          onClick={() => loadPatternXaOutcomeRow(row)}
                                          ref={(element) => {
                                            if (element) {
                                              patternXaOutcomeRowRefs.current.set(rowKey, element);
                                            } else {
                                              patternXaOutcomeRowRefs.current.delete(rowKey);
                                            }
                                          }}
                                          title={`${row.setup_id} | ${row.pattern_group_id}`}
                                        >
                                          <td>{formatDate(row.d_confirm_date)}</td>
                                          <td>{row.symbol}</td>
                                          <td>{row.market}</td>
                                          <td className={outcomeClass}>{row.outcome || 'N/A'}</td>
                                          <td>{formatDate(row.hit_date)}</td>
                                          <td>{formatOptionalNumber(row.bars_to_hit)}</td>
                                          <td>{formatOptionalNumber(row.minutes_to_hit)}</td>
                                          <td>{formatDecimal(row.d_price, 4)}</td>
                                          <td>{formatDecimal(row.xa_distance, 4)}</td>
                                          <td>{formatDecimal(row.reversal_target_price, 4)}</td>
                                          <td>{formatDecimal(row.continuation_target_price, 4)}</td>
                                          <td>{formatDecimal(row.max_reversal_excursion, 4)}</td>
                                          <td>{formatDecimal(row.max_continuation_excursion, 4)}</td>
                                          <td>{compactText(row.pattern_family_key, 16)}</td>
                                          <td className="pattern-family-template-table-run-id pattern-family-template-table-copy-cell">
                                            <span>{compactText(row.pattern_id || row.setup_id, 16)}</span>
                                            <button
                                              aria-label="Copy pattern ID"
                                              className={[
                                                'pattern-family-table-copy-button',
                                                copiedPatternId === String(row.pattern_id || row.setup_id)
                                                  ? 'pattern-family-table-copy-button--copied'
                                                  : '',
                                              ].filter(Boolean).join(' ')}
                                              onClick={(event) => handleCopyPatternId(row.pattern_id || row.setup_id, event)}
                                              title={copiedPatternId === String(row.pattern_id || row.setup_id) ? 'Copied' : 'Copy pattern ID'}
                                              type="button"
                                            >
                                              <span aria-hidden="true" />
                                            </button>
                                          </td>
                                        </tr>
                                      );
                                    })}
                                  </tbody>
                                </table>
                              ) : isPatternXaOutcomeLoading ? (
                                <div className="pattern-family-test-overview-empty">Loading reversal pattern rows...</div>
                              ) : patternXaOutcomeError ? (
                                <div className="pattern-family-test-overview-empty pattern-family-test-overview-empty--error">
                                  {patternXaOutcomeError}
                                </div>
                              ) : (
                                <div className="pattern-family-test-overview-empty">
                                  No XA reversal outcome rows loaded yet.
                                </div>
                              )}
                            </div>
                            <footer className="pattern-family-table-footer">
                              <button
                                disabled={isPatternXaOutcomeLoading}
                                onClick={() => loadPatternXaOutcomes({ offset: 0, append: false })}
                                type="button"
                              >
                                Refresh
                              </button>
                              <button
                                disabled={isPatternXaOutcomeLoading || !patternXaOutcomeHasMore}
                                onClick={() =>
                                  loadPatternXaOutcomes({
                                    offset: patternXaOutcomeRows.length,
                                    append: true,
                                  })
                                }
                                type="button"
                              >
                                {patternXaOutcomeHasMore ? 'Load More' : 'All Loaded'}
                              </button>
                            </footer>
                          </details>
                        ) : section.variant === 'patternXaFamilyTable' ? (
                          <details className="pattern-family-test-overview-collapsible-section" open>
                            <summary>
                              <span>Family Reversal Data</span>
                              <small>
                                {isPatternXaOutcomeLoading
                                  ? 'Loading families'
                                  : section.tableRows?.length
                                    ? `${formatNumber(section.tableRows.length)} families`
                                    : patternXaOutcomeError || 'No family rows loaded'}
                              </small>
                            </summary>
                            <div className="pattern-family-template-table-wrap pattern-family-template-table-wrap--xa-family">
                              {section.tableRows?.length ? (
                                <table className="pattern-family-template-table pattern-family-template-table--router pattern-family-template-table--xa-family">
                                  <thead>
                                    <tr>
                                      <th>#</th>
                                      <th>Family ID</th>
                                      <th>Features</th>
                                      <th>Total</th>
                                      <th>Reversal</th>
                                      <th>Rev %</th>
                                      <th>Continuation</th>
                                      <th>Cont %</th>
                                      <th>None</th>
                                      <th>Ambiguous</th>
                                      <th>Avg Bars</th>
                                      <th>Avg Time</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    {section.tableRows.map((row, rowIndex) => {
                                      const total = Number(row.total_count || 0);
                                      const reversalRate = total ? (Number(row.reversal_count || 0) / total) * 100 : 0;
                                      const continuationRate = total ? (Number(row.continuation_count || 0) / total) * 100 : 0;
                                      const familyFeatures = [
                                        row.harmonic_type || 'Unknown',
                                        row.family_bin || 'Unknown',
                                        row.family_size_bucket || 'Unknown',
                                        row.family_time_bin || 'Unknown',
                                        row.family_x_strictness || 'Unknown',
                                      ].join(' | ');
                                      return (
                                        <tr key={row.pattern_family_key || 'unknown-family'}>
                                          <td className="pattern-family-template-table-count">{formatNumber(rowIndex + 1)}</td>
                                          <td className="pattern-family-template-table-run-id" title={row.pattern_family_key || 'Unknown'}>
                                            {row.pattern_family_key || 'Unknown'}
                                          </td>
                                          <td className="pattern-family-template-table-features" title={familyFeatures}>{familyFeatures}</td>
                                          <td>{formatNumber(row.total_count)}</td>
                                          <td className="pattern-family-template-table-win">{formatNumber(row.reversal_count)}</td>
                                          <td className={reversalRate >= 55 ? 'pattern-family-template-table-win' : reversalRate < 50 ? 'pattern-family-template-table-loss' : ''}>
                                            {formatDecimal(reversalRate, 1)}%
                                          </td>
                                          <td className="pattern-family-template-table-loss">{formatNumber(row.continuation_count)}</td>
                                          <td>{formatDecimal(continuationRate, 1)}%</td>
                                          <td>{formatNumber(row.none_count)}</td>
                                          <td>{formatNumber(row.ambiguous_count)}</td>
                                          <td>{formatOptionalNumber(row.avg_bars_to_hit)}</td>
                                          <td>{formatGapDuration(row.avg_minutes_to_hit)}</td>
                                        </tr>
                                      );
                                    })}
                                  </tbody>
                                </table>
                              ) : isPatternXaOutcomeLoading ? (
                                <div className="pattern-family-test-overview-empty">Loading reversal family data...</div>
                              ) : patternXaOutcomeError ? (
                                <div className="pattern-family-test-overview-empty pattern-family-test-overview-empty--error">
                                  {patternXaOutcomeError}
                                </div>
                              ) : (
                                <div className="pattern-family-test-overview-empty">
                                  No XA reversal family rows loaded yet.
                                </div>
                              )}
                            </div>
                          </details>
                        ) : section.variant === 'patternReversalAiThresholds' ? (
                          <details className="pattern-family-test-overview-collapsible-section" open>
                            <summary>
                              <span>AI Threshold Tests</span>
                              <small>
                                {isPatternReversalAiLoading
                                  ? 'Loading thresholds'
                                  : section.tableRows?.length
                                    ? `${formatNumber(section.tableRows.length)} cutoffs`
                                    : patternReversalAiError || 'No threshold rows loaded'}
                              </small>
                            </summary>
                            <div className="pattern-family-template-table-wrap pattern-family-template-table-wrap--xa-family">
                              {section.tableRows?.length ? (
                                <table className="pattern-family-template-table pattern-family-template-table--router pattern-family-template-table--ai-thresholds">
                                  <thead>
                                    <tr>
                                      <th>Rule Cutoff</th>
                                      <th>Patterns Taken</th>
                                      <th>Actual Reversals</th>
                                      <th>Actual Rev %</th>
                                      <th>Avg AI Rev %</th>
                                      <th>Lift</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    {section.tableRows.map((row) => (
                                      <tr key={`ai-threshold-${row.threshold_value}`}>
                                        <td>{`AI >= ${formatDecimal(Number(row.threshold_value || 0) * 100, 0)}%`}</td>
                                        <td>{formatNumber(row.row_count)}</td>
                                        <td className="pattern-family-template-table-win">{formatNumber(row.actual_reversal_count)}</td>
                                        <td className={Number(row.actual_reversal_rate || 0) >= Number(patternReversalAiRun?.baseline_reversal_rate || 0) ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                                          {formatDecimal(Number(row.actual_reversal_rate || 0) * 100, 2)}%
                                        </td>
                                        <td>{formatDecimal(Number(row.avg_predicted_reversal_probability || 0) * 100, 2)}%</td>
                                        <td className={Number(row.lift_vs_baseline || 0) >= 0 ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                                          {formatDecimal(Number(row.lift_vs_baseline || 0) * 100, 2)}pp
                                        </td>
                                      </tr>
                                    ))}
                                  </tbody>
                                </table>
                              ) : isPatternReversalAiLoading ? (
                                <div className="pattern-family-test-overview-empty">Loading AI threshold tests...</div>
                              ) : patternReversalAiError ? (
                                <div className="pattern-family-test-overview-empty pattern-family-test-overview-empty--error">
                                  {patternReversalAiError}
                                </div>
                              ) : (
                                <div className="pattern-family-test-overview-empty">
                                  No AI threshold rows loaded yet.
                                </div>
                              )}
                            </div>
                          </details>
                        ) : section.variant === 'patternReversalAiBuckets' ? (
                          <details className="pattern-family-test-overview-collapsible-section" open>
                            <summary>
                              <span>AI Reversal Buckets</span>
                              <small>
                                {isPatternReversalAiLoading
                                  ? 'Loading buckets'
                                  : section.tableRows?.length
                                    ? `${formatNumber(section.tableRows.length)} buckets`
                                    : patternReversalAiError || 'No bucket rows loaded'}
                              </small>
                            </summary>
                            <div className="pattern-family-template-table-wrap pattern-family-template-table-wrap--xa-family">
                              {section.tableRows?.length ? (
                                <table className="pattern-family-template-table pattern-family-template-table--router pattern-family-template-table--ai-buckets">
                                  <thead>
                                    <tr>
                                      <th>Bucket</th>
                                      <th>Rows</th>
                                      <th>Actual Reversed</th>
                                      <th>Actual Rev %</th>
                                      <th>Avg AI Rev %</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    {section.tableRows.map((row) => (
                                      <tr key={row.confidence_bucket || 'unknown-bucket'}>
                                        <td>{row.confidence_bucket || 'Unknown'}</td>
                                        <td>{formatNumber(row.row_count)}</td>
                                        <td className="pattern-family-template-table-win">{formatNumber(row.actual_reversal_count)}</td>
                                        <td className={Number(row.actual_reversal_rate || 0) >= 0.6 ? 'pattern-family-template-table-win' : ''}>
                                          {formatDecimal(Number(row.actual_reversal_rate || 0) * 100, 2)}%
                                        </td>
                                        <td>{formatDecimal(Number(row.avg_predicted_reversal_probability || 0) * 100, 2)}%</td>
                                      </tr>
                                    ))}
                                  </tbody>
                                </table>
                              ) : isPatternReversalAiLoading ? (
                                <div className="pattern-family-test-overview-empty">Loading AI reversal buckets...</div>
                              ) : patternReversalAiError ? (
                                <div className="pattern-family-test-overview-empty pattern-family-test-overview-empty--error">
                                  {patternReversalAiError}
                                </div>
                              ) : (
                                <div className="pattern-family-test-overview-empty">
                                  No AI reversal bucket rows loaded yet.
                                </div>
                              )}
                            </div>
                          </details>
                        ) : section.variant === 'patternReversalAiTable' ? (
                          <details className="pattern-family-test-overview-collapsible-section" open>
                            <summary>
                              <span>AI Reversal Pattern Rows</span>
                              <small>
                                {isPatternReversalAiLoading
                                  ? 'Loading rows'
                                  : section.tableRows?.length
                                    ? `${formatNumber(section.tableRows.length)} loaded / ${formatNumber(patternReversalAiData.totalRows)} total`
                                    : patternReversalAiError || 'No AI rows loaded'}
                              </small>
                            </summary>
                            <div className="pattern-family-template-table-wrap pattern-family-template-table-wrap--xa-pattern pattern-family-ai-taken-trades-table">
                              {section.tableRows?.length ? (
                                <table className="pattern-family-template-table pattern-family-template-table--router pattern-family-template-table--ai-patterns">
                                  <thead>
                                    <tr>
                                      <th>Time</th>
                                      <th>Symbol</th>
                                      <th>Side</th>
                                      <th>AI Rev %</th>
                                      <th>Decision</th>
                                      <th>Actual</th>
                                      <th>Correct</th>
                                      <th>Bucket</th>
                                      <th>Family</th>
                                      <th>Features</th>
                                      <th>Pattern</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    {section.tableRows.map((row) => {
                                      const featureText = [
                                        row.harmonic_type || 'Unknown',
                                        row.family_bin || 'Unknown',
                                        row.family_size_bucket || 'Unknown',
                                        row.family_time_bin || 'Unknown',
                                        row.family_x_strictness || 'Unknown',
                                      ].join(' | ');
                                      const correct = Number(row.was_correct || 0) === 1;
                                      return (
                                        <tr key={row.setup_id || row.pattern_id || `${row.pattern_group_id}-${row.d_confirm_date}`}>
                                          <td>{formatDate(row.d_confirm_date)}</td>
                                          <td>{row.symbol || 'N/A'}</td>
                                          <td>{row.market || 'N/A'}</td>
                                          <td className={Number(row.predicted_reversal_probability || 0) >= 0.6 ? 'pattern-family-template-table-win' : ''}>
                                            {formatDecimal(Number(row.predicted_reversal_probability || 0) * 100, 2)}%
                                          </td>
                                          <td>{formatRouteMode(row.ai_decision || 'N/A')}</td>
                                          <td className={row.actual_outcome === 'reversal_xa' ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                                            {formatRouteMode(row.actual_outcome || 'N/A')}
                                          </td>
                                          <td className={correct ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                                            {correct ? 'Yes' : 'No'}
                                          </td>
                                          <td>{row.confidence_bucket || 'N/A'}</td>
                                          <td className="pattern-family-template-table-family-key" title={row.pattern_family_key}>
                                            {row.pattern_family_key || 'N/A'}
                                          </td>
                                          <td className="pattern-family-template-table-features" title={featureText}>{featureText}</td>
                                          <td className="pattern-family-template-table-run-id" title={row.pattern_id || row.setup_id}>
                                            {compactText(row.pattern_id || row.setup_id || 'N/A', 16)}
                                          </td>
                                        </tr>
                                      );
                                    })}
                                  </tbody>
                                </table>
                              ) : isPatternReversalAiLoading ? (
                                <div className="pattern-family-test-overview-empty">Loading AI reversal pattern rows...</div>
                              ) : patternReversalAiError ? (
                                <div className="pattern-family-test-overview-empty pattern-family-test-overview-empty--error">
                                  {patternReversalAiError}
                                </div>
                              ) : (
                                <div className="pattern-family-test-overview-empty">
                                  No AI reversal score rows loaded yet.
                                </div>
                              )}
                            </div>
                            <footer className="pattern-family-table-footer">
                              <button
                                disabled={isPatternReversalAiLoading}
                                onClick={() => loadPatternReversalAiScores({ offset: 0, append: false })}
                                type="button"
                              >
                                Refresh
                              </button>
                              <button
                                disabled={isPatternReversalAiLoading || !patternReversalAiHasMore}
                                onClick={() =>
                                  loadPatternReversalAiScores({
                                    offset: patternReversalAiRows.length,
                                    append: true,
                                  })
                                }
                                type="button"
                              >
                                {patternReversalAiHasMore ? 'Load More' : 'All Loaded'}
                              </button>
                            </footer>
                          </details>
                        ) : section.variant === 'patternAiExitModelTradeTable' ? (
                          <details className="pattern-family-test-overview-collapsible-section" open>
                            <summary>
                              <span>AI Exit Main 180-Bar Changed Trades</span>
                              <small>
                                {isPatternAiExitModelTradeLoading
                                  ? 'Loading exit audit'
                                  : section.tableRows?.length
                                    ? `${formatNumber(section.tableRows.length)} changed rows`
                                    : patternAiExitModelTradeError || 'No changed rows loaded'}
                              </small>
                            </summary>
                            <div className="pattern-family-template-table-wrap pattern-family-template-table-wrap--xa-pattern pattern-family-ai-taken-trades-table">
                              {section.tableRows?.length ? (
                                <table className="pattern-family-template-table pattern-family-template-table--router pattern-family-template-table--ai-stage1-trades">
                                  <colgroup>
                                    <col style={{ width: '60px' }} />
                                    <col style={{ width: '132px' }} />
                                    <col style={{ width: '96px' }} />
                                    <col style={{ width: '64px' }} />
                                    <col style={{ width: '86px' }} />
                                    <col style={{ width: '94px' }} />
                                    <col style={{ width: '94px' }} />
                                    <col style={{ width: '94px' }} />
                                    <col style={{ width: '112px' }} />
                                    <col style={{ width: '112px' }} />
                                    <col style={{ width: '112px' }} />
                                    <col style={{ width: '112px' }} />
                                    <col style={{ width: '122px' }} />
                                    <col style={{ width: '122px' }} />
                                    <col style={{ width: '122px' }} />
                                    <col style={{ width: '140px' }} />
                                    <col style={{ width: '420px' }} />
                                  </colgroup>
                                  <thead>
                                    <tr>
                                      <th>#</th>
                                      <th>Entry</th>
                                      <th>Symbol</th>
                                      <th>TF</th>
                                      <th>Side</th>
                                      <th>Source R</th>
                                      <th>AI R</th>
                                      <th>Delta</th>
                                      <th>Source Exit</th>
                                      <th>AI Exit</th>
                                      <th>Source Hold</th>
                                      <th>AI Hold</th>
                                      <th>Entry Px</th>
                                      <th>SL</th>
                                      <th>AI Exit Px</th>
                                      <th>Change</th>
                                      <th>Trade ID</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    {section.tableRows.map((row, rowIndex) => {
                                      const rowKey = getSimulationRawTradeKey(row, rowIndex);
                                      const deltaR = Number(row.delta_r || 0);
                                      const resultR = Number(row.result_r || 0);
                                      const changeClass =
                                        row.exit_change === 'held_longer'
                                          ? 'pattern-family-template-table-win'
                                          : row.exit_change === 'early_exit'
                                            ? 'pattern-family-template-table-skipped'
                                            : '';
                                      const auditTradeId =
                                        getAiStage1TradeUid(row) ||
                                        row.trade_uid ||
                                        row.candidate_uid ||
                                        row.pattern_id ||
                                        'N/A';
                                      return (
                                        <tr
                                          className={
                                            rowKey === selectedSimulationRawTradeKey
                                              ? 'pattern-family-template-table-row--selected'
                                              : ''
                                          }
                                          key={rowKey}
                                          onClick={() => handlePatternAiStage1TradeClick(row, rowIndex)}
                                          title={auditTradeId}
                                        >
                                          <td>{formatNumber(rowIndex + 1)}</td>
                                          <td>{formatShortDateTime(row.entry_date || row.d_confirm_date)}</td>
                                          <td>{row.symbol || 'N/A'}</td>
                                          <td>{row.source_timeframe || 'N/A'}</td>
                                          <td>{formatRouteMode(row.trade_direction || 'N/A')}</td>
                                          <td className={Number(row.baseline_result_r || 0) >= 0 ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                                            {formatDecimal(row.baseline_result_r, 3)}R
                                          </td>
                                          <td className={resultR >= 0 ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                                            {formatDecimal(resultR, 3)}R
                                          </td>
                                          <td className={deltaR >= 0 ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                                            {deltaR >= 0 ? '+' : ''}{formatDecimal(deltaR, 3)}R
                                          </td>
                                          <td>{formatShortDateTime(row.baseline_exit_date)}</td>
                                          <td>{formatShortDateTime(row.exit_date)}</td>
                                          <td>{formatGapDuration(row.baseline_hold_minutes)}</td>
                                          <td>{formatGapDuration(row.model_hold_minutes)}</td>
                                          <td>{formatDecimal(row.entry_price, 4)}</td>
                                          <td>{formatDecimal(row.stop_price, 4)}</td>
                                          <td>{formatDecimal(row.exit_price, 4)}</td>
                                          <td className={changeClass}>{formatRouteMode(row.exit_change || row.exit_reason || 'N/A')}</td>
                                          <td className="pattern-family-template-table-run-id pattern-family-template-table-copy-cell">
                                            <span>{auditTradeId}</span>
                                            <button
                                              aria-label="Copy trade ID"
                                              className={[
                                                'pattern-family-table-copy-button',
                                                copiedPatternId === String(auditTradeId)
                                                  ? 'pattern-family-table-copy-button--copied'
                                                  : '',
                                              ].filter(Boolean).join(' ')}
                                              onClick={(event) => handleCopyPatternId(auditTradeId, event)}
                                              title={copiedPatternId === String(auditTradeId) ? 'Copied' : 'Copy trade ID'}
                                              type="button"
                                            >
                                              <span aria-hidden="true" />
                                            </button>
                                          </td>
                                        </tr>
                                      );
                                    })}
                                  </tbody>
                                </table>
                              ) : isPatternAiExitModelTradeLoading ? (
                                <div className="pattern-family-test-overview-empty">Loading AI exit model rows...</div>
                              ) : patternAiExitModelTradeError ? (
                                <div className="pattern-family-test-overview-empty pattern-family-test-overview-empty--error">
                                  {patternAiExitModelTradeError}
                                </div>
                              ) : (
                                <div className="pattern-family-test-overview-empty">
                                  No changed exit rows loaded yet.
                                </div>
                              )}
                            </div>
                            <footer className="pattern-family-table-footer">
                              <button
                                disabled={isPatternAiExitModelTradeLoading}
                                onClick={() => loadPatternAiExitModelTrades({ offset: 0, append: false })}
                                type="button"
                              >
                                Refresh
                              </button>
                              <button
                                disabled={
                                  isPatternAiExitModelTradeLoading ||
                                  patternAiExitModelTradeRows.length >= Number(patternAiExitModelTradeData.totalRows || 0)
                                }
                                onClick={() =>
                                  loadPatternAiExitModelTrades({
                                    offset: patternAiExitModelTradeRows.length,
                                    append: true,
                                  })
                                }
                                type="button"
                              >
                                {patternAiExitModelTradeRows.length < Number(patternAiExitModelTradeData.totalRows || 0) ? 'Load More' : 'All Loaded'}
                              </button>
                            </footer>
                          </details>
                        ) : section.variant === 'patternAiStage1TradeTable' ? (
                          <details className="pattern-family-test-overview-collapsible-section" open>
                            <summary>
                              <span>AI 2026 Taken Trades - 180 Exit</span>
                              <small>
                                {isPatternAiExitModelTakenTradeLoading
                                  ? 'Loading trades'
                                  : section.tableRows?.length
                                    ? `${formatNumber(section.tableRows.length)} loaded / ${formatNumber(patternAiExitModelTakenTradeData.totalRows)} total`
                                    : patternAiExitModelTakenTradeError || 'No 180-bar AI exit trade rows loaded'}
                              </small>
                            </summary>
                            <div className="pattern-family-template-table-wrap pattern-family-template-table-wrap--xa-pattern pattern-family-ai-taken-trades-table">
                              {section.tableRows?.length ? (
                                <table className="pattern-family-template-table pattern-family-template-table--router pattern-family-template-table--ai-stage1-trades">
                                  <colgroup>
                                    <col style={{ width: '72px' }} />
                                    <col style={{ width: '146px' }} />
                                    <col style={{ width: '112px' }} />
                                    <col style={{ width: '72px' }} />
                                    <col style={{ width: '112px' }} />
                                    <col style={{ width: '112px' }} />
                                    <col style={{ width: '112px' }} />
                                    <col style={{ width: '112px' }} />
                                    <col style={{ width: '110px' }} />
                                    <col style={{ width: '148px' }} />
                                    <col style={{ width: '250px' }} />
                                    <col style={{ width: '132px' }} />
                                    <col style={{ width: '132px' }} />
                                    <col style={{ width: '132px' }} />
                                    <col style={{ width: '132px' }} />
                                    <col style={{ width: '420px' }} />
                                    <col style={{ width: '520px' }} />
                                    <col style={{ width: '760px' }} />
                                  </colgroup>
                                  <thead>
                                    <tr>
                                      <th>#</th>
                                      <th>Time</th>
                                      <th>Symbol</th>
                                      <th>TF</th>
                                      <th>Side</th>
                                      <th>Source R</th>
                                      <th>180 R</th>
                                      <th>Delta</th>
                                      <th>Outcome</th>
                                      <th>Exit Change</th>
                                      <th>Template</th>
                                      <th>Entry</th>
                                      <th>SL</th>
                                      <th>Target</th>
                                      <th>Exit</th>
                                      <th>Family</th>
                                      <th>Features</th>
                                      <th>Trade ID</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    {section.tableRows.map((row, rowIndex) => {
                                      const rowKey = getSimulationRawTradeKey(row, rowIndex);
                                      const resultR = Number(row.result_r || 0);
                                      const deltaR = Number(row.delta_r || 0);
                                      const featureText = [
                                        row.harmonic_type || 'Unknown',
                                        row.market || 'N/A',
                                        row.family_bin || 'Unknown',
                                        row.family_size_bucket || 'Unknown',
                                        row.family_time_bin || 'Unknown',
                                        row.family_x_strictness || 'Unknown',
                                      ].join(' | ');
                                      const outcomeClass =
                                        row.outcome === 'pass'
                                          ? 'pattern-family-template-table-win'
                                          : row.outcome === 'fail'
                                            ? 'pattern-family-template-table-loss'
                                            : 'pattern-family-template-table-skipped';
                                      const auditTradeId =
                                        getAiStage1TradeUid(row) ||
                                        row.trade_uid ||
                                        row.pattern_id ||
                                        row.setup_id ||
                                        'N/A';
                                      return (
                                        <tr
                                          className={
                                            rowKey === selectedSimulationRawTradeKey
                                              ? 'pattern-family-template-table-row--selected'
                                              : ''
                                          }
                                          key={rowKey}
                                          onClick={() => handlePatternAiStage1TradeClick(row, rowIndex)}
                                          title={`${row.setup_id} | ${row.template_uid} | ${row.multi_valid_eval_run_id}`}
                                        >
                                          <td>{formatNumber(rowIndex + 1)}</td>
                                          <td>{formatShortDateTime(row.entry_date || row.d_confirm_date)}</td>
                                          <td>{row.symbol || 'N/A'}</td>
                                          <td>{row.source_timeframe || 'N/A'}</td>
                                          <td>{formatRouteMode(row.trade_direction || row.market || 'N/A')}</td>
                                          <td className={Number(row.baseline_result_r || 0) >= 0 ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                                            {formatDecimal(row.baseline_result_r, 3)}R
                                          </td>
                                          <td className={resultR >= 0 ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                                            {formatDecimal(resultR, 3)}R
                                          </td>
                                          <td className={deltaR >= 0 ? 'pattern-family-template-table-win' : 'pattern-family-template-table-loss'}>
                                            {deltaR >= 0 ? '+' : ''}{formatDecimal(deltaR, 3)}R
                                          </td>
                                          <td className={outcomeClass}>{formatRouteMode(row.outcome || row.exit_reason || 'N/A')}</td>
                                          <td>{formatRouteMode(row.exit_change || 'Unchanged')}</td>
                                          <td className="pattern-family-template-table-run-id pattern-family-ai-taken-template-cell" title={row.template_uid || row.template_name}>
                                            {row.template_name || row.template_uid || 'N/A'}
                                          </td>
                                          <td>{formatDecimal(row.entry_price, 4)}</td>
                                          <td>{formatDecimal(row.stop_price, 4)}</td>
                                          <td title={row.target_price === null || row.target_price === undefined ? 'No fixed target' : ''}>
                                            {formatNullableDecimal(row.target_price, 4)}
                                          </td>
                                          <td>{formatDecimal(row.exit_price, 4)}</td>
                                          <td className="pattern-family-template-table-family-key" title={row.pattern_family_key}>
                                            {row.pattern_family_key || 'N/A'}
                                          </td>
                                          <td className="pattern-family-template-table-features" title={featureText}>{featureText}</td>
                                          <td className="pattern-family-template-table-run-id pattern-family-template-table-copy-cell">
                                            <span>{auditTradeId}</span>
                                            <button
                                              aria-label="Copy trade ID"
                                              className={[
                                                'pattern-family-table-copy-button',
                                                copiedPatternId === String(auditTradeId)
                                                  ? 'pattern-family-table-copy-button--copied'
                                                  : '',
                                              ].filter(Boolean).join(' ')}
                                              onClick={(event) => handleCopyPatternId(auditTradeId, event)}
                                              title={copiedPatternId === String(auditTradeId) ? 'Copied' : 'Copy trade ID'}
                                              type="button"
                                            >
                                              <span aria-hidden="true" />
                                            </button>
                                          </td>
                                        </tr>
                                      );
                                    })}
                                  </tbody>
                                </table>
                              ) : isPatternAiExitModelTakenTradeLoading ? (
                                <div className="pattern-family-test-overview-empty">Loading 180-bar AI exit taken trade rows...</div>
                              ) : patternAiExitModelTakenTradeError ? (
                                <div className="pattern-family-test-overview-empty pattern-family-test-overview-empty--error">
                                  {patternAiExitModelTakenTradeError}
                                </div>
                              ) : (
                                <div className="pattern-family-test-overview-empty">
                                  No 180-bar AI exit trade rows loaded yet.
                                </div>
                              )}
                            </div>
                            <footer className="pattern-family-table-footer">
                              <button
                                disabled={isPatternAiExitModelTakenTradeLoading}
                                onClick={() => loadPatternAiExitModelTakenTrades({ offset: 0, append: false })}
                                type="button"
                              >
                                Refresh
                              </button>
                              <button
                                disabled={isPatternAiExitModelTakenTradeLoading || !patternAiExitModelTakenTradeHasMore}
                                onClick={() =>
                                  loadPatternAiExitModelTakenTrades({
                                    offset: patternAiExitModelTakenTradeRows.length,
                                    append: true,
                                  })
                                }
                                type="button"
                              >
                                {patternAiExitModelTakenTradeHasMore ? 'Load More' : 'All Loaded'}
                              </button>
                            </footer>
                          </details>
                        ) : section.variant === 'templateTable' ? (
                          <div className="pattern-family-template-table-wrap">
                            {section.tableRows?.length ? (
                              <table className="pattern-family-template-table">
                                <thead>
                                  <tr>
                                    <th>Template</th>
                                    <th>Rule</th>
                                    <th>Total</th>
                                    <th>WR</th>
                                    <th>Avg R</th>
                                    <th>Bull Tests</th>
                                    <th>Bull WR</th>
                                    <th>Bull R</th>
                                    <th>Bear Tests</th>
                                    <th>Bear WR</th>
                                    <th>Bear R</th>
                                    <th>Edge</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  {section.tableRows.map((row) => (
                                    <tr
                                      className={row.isSelected ? 'pattern-family-template-table-row--selected' : ''}
                                      key={row.template.template_uid}
                                      onClick={() => {
                                        setSelectedEntryExitTemplateUid(row.template.template_uid);
                                        setEntryExitProfileTab('all');
                                      }}
                                      title={`${row.template.template_uid} | ${row.template.template_name}`}
                                    >
                                      <td className="pattern-family-template-table-id">{row.label}</td>
                                      <td>{row.rule}</td>
                                      <td>{formatNumber(row.evalCount)}</td>
                                      <td>{formatDecimal(row.passRate, 1)}%</td>
                                      <td className={Number(row.template.avg_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                        {formatDecimal(row.template.avg_r, 3)}R
                                      </td>
                                      <td>{formatNumber(row.bullishEvalCount)}</td>
                                      <td>{formatDecimal(row.bullishPassRate, 1)}%</td>
                                      <td className={Number(row.template.bullish_avg_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                        {formatDecimal(row.template.bullish_avg_r, 3)}R
                                      </td>
                                      <td>{formatNumber(row.bearishEvalCount)}</td>
                                      <td>{formatDecimal(row.bearishPassRate, 1)}%</td>
                                      <td className={Number(row.template.bearish_avg_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                        {formatDecimal(row.template.bearish_avg_r, 3)}R
                                      </td>
                                      <td>
                                        {row.template.market_edge_label || 'Flat'} / {formatDecimal(row.template.market_edge_score, 3)}
                                      </td>
                                    </tr>
                                  ))}
                                </tbody>
                              </table>
                            ) : isEntryExitLoading ? (
                              <div className="pattern-family-test-overview-empty">
                                Loading Entry / Exit templates...
                              </div>
                            ) : entryExitError ? (
                              <div className="pattern-family-test-overview-empty pattern-family-test-overview-empty--error">
                                {entryExitError}
                              </div>
                            ) : (
                              <div className="pattern-family-test-overview-empty">
                                No generated templates loaded yet.
                              </div>
                            )}
                          </div>
                        ) : section.variant === 'familyRouterTable' ? (
                          <div className="pattern-family-template-table-wrap">
                            {section.tableRows?.length ? (
                              <table className="pattern-family-template-table pattern-family-template-table--family-router">
                                <thead>
                                  <tr>
                                    <th>#</th>
                                    <th>Family ID</th>
                                    <th>Family</th>
                                    <th>Template</th>
                                    <th>Status</th>
                                    <th>Train Tests</th>
                                    <th>Train WR</th>
                                    <th>Train Avg R</th>
                                    <th>Test Trades</th>
                                    <th>Test WR</th>
                                    <th>Test Avg R</th>
                                    <th>Total R</th>
                                    <th>Score</th>
                                    <th>Reason</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  {section.tableRows.map((row) => {
                                    const route = row.route;
                                    const statusClass =
                                      route.route_status === 'TRADE'
                                        ? 'pattern-family-template-table-win'
                                        : route.route_status === 'WATCHLIST'
                                          ? 'pattern-family-template-table-skipped'
                                          : 'pattern-family-template-table-loss';

                                    return (
                                      <tr
                                        className={row.isSelected ? 'pattern-family-template-table-row--selected' : ''}
                                        key={`${route.family_key}-${route.template_uid}-${route.template_rank}`}
                                        onClick={() => {
                                          setSelectedEntryExitTemplateUid(route.template_uid);
                                          setEntryExitProfileTab('all');
                                        }}
                                        title={`${route.family_key} | ${route.template_uid} | ${route.status_reason}`}
                                      >
                                        <td>{formatNumber(row.index)}</td>
                                        <td>{compactText(route.family_key, 16)}</td>
                                        <td>
                                          {route.harmonic_type || 'Unknown'} | {route.market || 'N/A'} | {route.family_bin || 'Bin'} | {route.family_size_bucket || 'Size'} | {route.family_time_bin || 'Time'}
                                        </td>
                                        <td className="pattern-family-template-table-id">
                                          {route.template_label || compactText(route.template_uid, 8)}
                                          <span className="pattern-family-template-table-subtext">
                                            {route.template_name || 'Template'}
                                          </span>
                                        </td>
                                        <td className={statusClass}>{route.route_status || 'N/A'}</td>
                                        <td>{formatNumber(route.train_eval_count)}</td>
                                        <td>{formatRatePercent(route.train_win_rate, 2)}%</td>
                                        <td className={Number(route.train_avg_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                          {formatDecimal(route.train_avg_r, 4)}R
                                        </td>
                                        <td>{formatNumber(row.testEvalCount)}</td>
                                        <td>{formatDecimal(row.testPassRate, 2)}%</td>
                                        <td className={Number(route.test_avg_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                          {formatDecimal(route.test_avg_r, 4)}R
                                        </td>
                                        <td className={Number(route.test_sum_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                          {formatDecimal(route.test_sum_r, 2)}R
                                        </td>
                                        <td>{formatDecimal(route.score, 2)}</td>
                                        <td className="pattern-family-template-table-reason">{route.status_reason || 'Selected by family playbook'}</td>
                                      </tr>
                                    );
                                  })}
                                </tbody>
                              </table>
                            ) : isEntryExitRouterLoading ? (
                              <div className="pattern-family-test-overview-empty">
                                Loading family playbook...
                              </div>
                            ) : entryExitRouterError ? (
                              <div className="pattern-family-test-overview-empty pattern-family-test-overview-empty--error">
                                {entryExitRouterError}
                              </div>
                            ) : (
                              <div className="pattern-family-test-overview-empty">
                                No family playbook rows loaded yet.
                              </div>
                            )}
                          </div>
                        ) : section.variant === 'routerRunTable' ? (
                          <div className="pattern-family-template-table-wrap">
                            {section.tableRows?.length ? (
                              <table className="pattern-family-template-table pattern-family-template-table--router">
                                <thead>
                                  <tr>
                                    <th>Playbook</th>
                                    <th>Year</th>
                                    <th>TF</th>
                                    <th>Patterns</th>
                                    <th>Trades</th>
                                    <th>Trade WR</th>
                                    <th>Avg R</th>
                                    <th>Total R</th>
                                    <th>Prop WR</th>
                                    <th>Prop Total</th>
                                    <th>Passed</th>
                                    <th>Failed</th>
                                    <th>Daily Loss</th>
                                    <th>Drawdown</th>
                                    <th>Open</th>
                                    <th>Families</th>
                                    <th>Manual</th>
                                    <th>Symbols</th>
                                    <th>Overlap</th>
                                    <th>Simulation Test ID</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  {section.tableRows.map((row) => (
                                    <tr
                                      className={row.isSelected ? 'pattern-family-template-table-row--selected' : ''}
                                      key={row.run.router_run_id}
                                      onClick={() => {
                                        setSelectedEntryExitRouterRunId(row.run.router_run_id);
                                        setEntryExitProfileTab('model');
                                        setSelectedDataCollapseLevel(2);
                                        setInspectorCollapsed(true);
                                      }}
                                      title={`${row.playbookLabel} | ${row.playbookName} | ${row.run.router_run_id}`}
                                    >
                                      <td className="pattern-family-template-table-id">{row.playbookLabel}</td>
                                      <td>{row.run.test_year || 'All'}</td>
                                      <td>{row.run.source_timeframe ?? row.run.test_timeframe ?? ''}</td>
                                      <td>{formatNumber(row.run.patterns_scanned)}</td>
                                      <td>{formatNumber(row.tradeCount)}</td>
                                      <td>{formatDecimal(row.winRate, 2)}%</td>
                                      <td className={Number(row.run.avg_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                        {formatDecimal(row.run.avg_r, 4)}R
                                      </td>
                                      <td className={Number(row.run.sum_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                        {formatDecimal(row.run.sum_r, 2)}R
                                      </td>
                                      <td className={Number(row.propClosedPassRate) >= 80 ? 'pattern-family-template-table-win' : 'pattern-family-template-table-skipped'}>
                                        {formatDecimal(row.propClosedPassRate, 2)}%
                                      </td>
                                      <td title={`${formatNumber(row.propClosedTotal)} closed / ${formatNumber(row.propTotal)} total cycles`}>
                                        {formatNumber(row.propTotal)}
                                      </td>
                                      <td className="pattern-family-template-table-win">{formatNumber(row.propPassed)}</td>
                                      <td className="pattern-family-template-table-loss">{formatNumber(row.propFailed)}</td>
                                      <td className="pattern-family-template-table-skipped">{formatNumber(row.propDailyFails)}</td>
                                      <td className="pattern-family-template-table-loss">{formatNumber(row.propDrawdownFails)}</td>
                                      <td>{formatNumber(row.propIncomplete)}</td>
                                      <td>{formatNumber(row.run.trade_choices)}T / {formatNumber(row.run.watchlist_choices)}W / {formatNumber(row.run.skip_choices)}S</td>
                                      <td className={row.manualFamilyBansApplied ? 'pattern-family-template-table-skipped' : ''}>
                                        {formatNumber(row.manualFamilyBansApplied)} applied
                                      </td>
                                      <td>{formatNumber(row.run.symbol_trade_roots)}T / {formatNumber(row.run.symbol_skip_roots)}S</td>
                                      <td>
                                        {[
                                          row.run.one_trade_at_a_time
                                            ? '1 total'
                                            : row.run.one_trade_per_root_symbol
                                              ? '1/root'
                                              : null,
                                          getEntryExitCooldownShortLabel(row.run),
                                        ].filter(Boolean).join(' + ') || 'Off'} / {formatNumber(row.run.skipped_overlap_patterns)}
                                      </td>
                                      <td className="pattern-family-template-table-run-id">{row.run.router_run_id}</td>
                                    </tr>
                                  ))}
                                </tbody>
                              </table>
                            ) : isEntryExitRouterLoading ? (
                              <div className="pattern-family-test-overview-empty">
                                Loading simulation tests...
                              </div>
                            ) : entryExitRouterError ? (
                              <div className="pattern-family-test-overview-empty pattern-family-test-overview-empty--error">
                                {entryExitRouterError}
                              </div>
                            ) : (
                              <div className="pattern-family-test-overview-empty">
                                No simulation test runs loaded yet.
                              </div>
                            )}
                          </div>
                        ) : (
                          <div className="pattern-family-test-overview-cells">
                          {section.items.length ? (
                            section.items.map((item) => {
                              const cellClassName = [
                                'pattern-family-test-overview-cell',
                                item.wide ? 'pattern-family-test-overview-cell--wide' : '',
                                item.compact ? 'pattern-family-test-overview-cell--compact' : '',
                                item.narrative ? 'pattern-family-test-overview-cell--narrative' : '',
                                item.onClick ? 'pattern-family-test-overview-cell--clickable' : '',
                                item.selected ? 'pattern-family-test-overview-cell--selected' : '',
                                item.tone ? `pattern-family-test-overview-cell--${item.tone}` : '',
                              ].filter(Boolean).join(' ');
                              const content = (
                                <>
                                  <span>{item.label}</span>
                                  <strong title={item.title ?? item.value}>{item.value}</strong>
                                </>
                              );

                              return item.onClick ? (
                                <button
                                  className={cellClassName}
                                  key={`${section.title}-${item.label}`}
                                  onClick={item.onClick}
                                  type="button"
                                >
                                  {content}
                                </button>
                              ) : (
                                <div
                                  className={cellClassName}
                                  key={`${section.title}-${item.label}`}
                                >
                                  {content}
                                </div>
                              );
                            })
                          ) : (
                            <div className="pattern-family-test-overview-empty">
                              No trade breakdown data loaded yet.
                            </div>
                          )}
                          </div>
                        )}
                        </div>
                        )}
                        </section>
                        );
                      })}
                    </div>
                  )}
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

        <CanvasCollapseWrapper {...canvasCollapseWrapperProps}>
          {testOverviewTab === 'entryExit' ? (
            <section className="pattern-family-side-data-profile pattern-family-side-data-profile--blank">
              <section className="pattern-family-entry-dashboard-page pattern-family-entry-dashboard-stage" aria-label="Data dashboard">
                  {(isEntryExitLoading || isEntryExitRouterLoading) && !selectedBuildRunIsLoaded ? (
                    <div className="pattern-family-entry-dashboard-loading">
                      <span>Loading Dashboard</span>
                      <strong>{selectedBuildLabel}</strong>
                      <small>Loading stored build summary and coverage rows...</small>
                    </div>
                  ) : null}

                  {!((isEntryExitLoading || isEntryExitRouterLoading) && !selectedBuildRunIsLoaded) ? (
                    <section className="pattern-family-entry-dashboard-static-context" aria-label="Selected build and coverage summary">
                      <div className="pattern-family-entry-dashboard-context-grid">
                        <section className="pattern-family-selected-build-panel">
                          <section className="pattern-family-entry-dashboard-build-strip">
                            <div className="pattern-family-entry-dashboard-build-title">
                              <span>Selected Build</span>
                              <div className="pattern-family-entry-dashboard-build-title-actions">
                                <div className="pattern-family-entry-dashboard-build-switch" aria-label="Select build">
                                  {entryExitModelDatasets.length ? (
                                    entryExitModelDatasets.map((dataset) => (
                                      <button
                                        className={
                                          dataset.id === selectedBuildRunId
                                            ? 'pattern-family-entry-dashboard-build-button pattern-family-entry-dashboard-build-button--active'
                                            : 'pattern-family-entry-dashboard-build-button'
                                        }
                                        key={dataset.id}
                                        onClick={() => {
                                          setSelectedEntryExitModelDatasetId(dataset.id);
                                          setSelectedEntryExitTemplateUid(null);
                                          setSelectedEntryExitRouterRunId(null);
                                          setSelectedBuildCoverageExchangeKey('');
                                          setEntryExitProfileTab('model');
                                        }}
                                        title={`${dataset.buildLabel} | ${dataset.label}`}
                                        type="button"
                                      >
                                        {dataset.buildLabel}
                                      </button>
                                    ))
                                  ) : (
                                    <span>
                                      {entryExitBuildListError ||
                                        (isEntryExitBuildListLoading ? 'Loading builds' : 'No stored builds')}
                                    </span>
                                  )}
                                </div>
                                <div className="pattern-family-selected-build-tabs" role="tablist" aria-label="Selected build view">
                                  {[
                                    { id: 'dashboard', label: 'Dashboard' },
                                    { id: 'raw', label: 'Raw Rows' },
                                  ].map((tab) => {
                                    const isActive = selectedBuildView === tab.id;
                                    return (
                                      <button
                                        className={
                                          isActive
                                            ? 'pattern-family-selected-build-tab pattern-family-selected-build-tab--active'
                                            : 'pattern-family-selected-build-tab'
                                        }
                                        key={tab.id}
                                        onClick={() => setSelectedBuildView(tab.id)}
                                        role="tab"
                                        aria-selected={isActive}
                                        type="button"
                                      >
                                        {tab.label}
                                      </button>
                                    );
                                  })}
                                </div>
                              </div>
                            </div>
                          </section>

                          {selectedBuildView === 'raw' ? (
                            <section className="pattern-family-selected-playbook-used pattern-family-selected-build-raw">
                              <header>
                                <span>Final Build Tests</span>
                                <small>{formatNumber(displayedEntryExitTemplates.length)} tests</small>
                              </header>
                              <div className="pattern-family-entry-dashboard-raw-table">
                                {displayedEntryExitTemplates.length ? (
                                  <table>
                                    <thead>
                                      <tr>
                                        <th>#</th>
                                        <th>Template</th>
                                        <th>Rule</th>
                                        <th>Dir</th>
                                        <th>Entry</th>
                                        <th>Risk</th>
                                        <th>Target</th>
                                        <th>Eval</th>
                                        <th>Pass</th>
                                        <th>Fail</th>
                                        <th>No Entry</th>
                                        <th>Avg R</th>
                                      </tr>
                                    </thead>
                                    <tbody>
                                      {displayedEntryExitTemplates.map((template, templateIndex) => {
                                        const entryOffset = getTemplateEntryOffset(template);
                                        return (
                                          <tr
                                            key={template.template_uid || `${template.created_from_setup_id}-${templateIndex}`}
                                            title={`${template.template_name || 'Template'} | ${template.template_uid || 'N/A'}`}
                                          >
                                            <td>{formatNumber(templateIndex + 1)}</td>
                                            <td title={template.template_uid}>
                                              {getEntryExitTemplateLabel(template)}
                                            </td>
                                            <td title={template.template_name}>
                                              {formatEntryExitTemplateRule(template)}
                                            </td>
                                            <td>
                                              {template.direction_mode === 'inverse_pattern' ? 'Inverse' : 'Pattern'}
                                            </td>
                                            <td>
                                              {entryOffset
                                                ? `C+${entryOffset}`
                                                : formatRouteMode(template.entry_kind)}
                                            </td>
                                            <td>{formatDecimal(template.risk_multiple, 3)} {getEntryExitTemplateRiskLabel(template)}</td>
                                            <td>{formatDecimal(template.target_r, 2)}R</td>
                                            <td>{formatNumber(template.eval_count)}</td>
                                            <td className="is-win">{formatNumber(template.pass_count)}</td>
                                            <td className="is-loss">{formatNumber(template.fail_count)}</td>
                                            <td>{formatNumber(template.no_entry_count)}</td>
                                            <td className={Number(template.avg_r) < 0 ? 'is-loss' : 'is-win'}>
                                              {formatDecimal(template.avg_r, 3)}R
                                            </td>
                                          </tr>
                                        );
                                      })}
                                    </tbody>
                                  </table>
                                ) : (
                                  <div className="pattern-family-build-coverage-empty">
                                    No final tests loaded for this build.
                                  </div>
                                )}
                              </div>
                            </section>
                          ) : (
                        <>
                        {renderSelectedBuildInfoTable()}
                        <section className="pattern-family-build-coverage-board pattern-family-build-coverage-board--static">
                          <header className="pattern-family-build-coverage-head">
                            <div>
                              <span>Build Coverage</span>
                              <strong>Exchange / symbol rows</strong>
                            </div>
                            <small>
                              {selectedBuildHasStoredSummary
                                ? `${formatNumber(selectedBuildCoveragePatternCount)} patterns | ${formatNumber(selectedBuildRootCount)} roots`
                                : ''}
                            </small>
                          </header>
                          {entryExitBuildExchangeSections.length ? (
                            <>
                              <div className="pattern-family-build-coverage-exchange-togglebar" role="tablist" aria-label="Build coverage exchange">
                                {entryExitBuildExchangeSections.map((section) => {
                                  const exchangeKey = getExchangeClassSuffix(section.exchange);
                                  const isActive =
                                    selectedBuildCoverageExchangeSection?.exchange === section.exchange;
                                  return (
                                    <button
                                      className={[
                                        'pattern-family-build-coverage-exchange-toggle',
                                        `pattern-family-build-coverage-exchange-toggle--${exchangeKey}`,
                                        isActive ? 'pattern-family-build-coverage-exchange-toggle--active' : '',
                                      ].join(' ')}
                                      key={section.exchange}
                                      onClick={() => setSelectedBuildCoverageExchangeKey(exchangeKey)}
                                      role="tab"
                                      aria-selected={isActive}
                                      type="button"
                                    >
                                      <span>{section.exchange}</span>
                                      <strong>{formatNumber(section.pattern_count)}</strong>
                                      <small>{formatNumber(section.scanned_count)} / {formatNumber(section.rows.length)}</small>
                                    </button>
                                  );
                                })}
                              </div>
                              {selectedBuildCoverageExchangeSection ? (
                                <div
                                  className={[
                                    'pattern-family-build-coverage-table-wrap',
                                    `pattern-family-build-coverage-table-wrap--${getExchangeClassSuffix(
                                      selectedBuildCoverageExchangeSection.exchange
                                    )}`,
                                  ].join(' ')}
                                >
                                  <table className="pattern-family-build-coverage-table">
                                    <thead>
                                      <tr>
                                        <th>Root</th>
                                        <th>Scanned</th>
                                        <th>Universe</th>
                                        <th>Contracts</th>
                                        <th>TF</th>
                                        <th>Status</th>
                                      </tr>
                                    </thead>
                                    <tbody
                                      className={[
                                        'pattern-family-build-coverage-table-section',
                                        `pattern-family-build-coverage-table-section--${getExchangeClassSuffix(
                                          selectedBuildCoverageExchangeSection.exchange
                                        )}`,
                                      ].join(' ')}
                                    >
                                      {selectedBuildCoverageExchangeSection.rows.map((row) => (
                                        <tr
                                          className={[
                                            `pattern-family-build-coverage-table-row--${getExchangeClassSuffix(
                                              selectedBuildCoverageExchangeSection.exchange
                                            )}`,
                                            row.is_scanned ? '' : 'pattern-family-build-coverage-table-row--empty',
                                          ].join(' ')}
                                          key={`${selectedBuildCoverageExchangeSection.exchange}-${row.root_symbol}`}
                                          title={`${row.root_symbol} | ${formatNumber(row.pattern_count)} scanned patterns | ${formatNumber(row.universe_pattern_count)} universe patterns | ${formatNumber(row.contract_count)} contracts | ${row.timeframe_label}`}
                                        >
                                          <td className="pattern-family-build-coverage-table-root">{row.root_symbol}</td>
                                          <td>{formatNumber(row.pattern_count)}</td>
                                          <td>{formatNumber(row.universe_pattern_count)}</td>
                                          <td>{formatNumber(row.contract_count)}</td>
                                          <td>{row.timeframe_label}</td>
                                          <td>{row.is_scanned ? 'Scanned' : 'Not scanned'}</td>
                                        </tr>
                                      ))}
                                    </tbody>
                                  </table>
                                </div>
                              ) : null}
                            </>
                          ) : (
                            <div className="pattern-family-build-coverage-empty">
                              {isEntryExitLoading
                                ? 'Loading stored build coverage rows...'
                                : ''}
                            </div>
                          )}
                        </section>
                        </>
                          )}
                        </section>

                        <aside
                          className={[
                            'pattern-family-selected-playbook-panel',
                            selectedPlaybookView !== 'dashboard' ? 'pattern-family-selected-playbook-panel--raw' : '',
                          ].filter(Boolean).join(' ')}
                        >
                          <header className="pattern-family-selected-playbook-head">
                            <div className="pattern-family-selected-playbook-head-left">
                              <span>Selected Playbook</span>
                              <div
                                className={[
                                  'pattern-family-build-playbook-link',
                                  `pattern-family-build-playbook-link--${selectedPlaybookLinkState}`,
                                ].join(' ')}
                                title={`Build ${selectedPlaybookBuildRunId || 'N/A'} | Playbook ${
                                  selectedEntryExitPlaybookName || 'N/A'
                                }`}
                              >
                                <strong>{selectedPlaybookBuildLabel}</strong>
                                <span aria-hidden="true">-&gt;</span>
                                <strong>
                                  {selectedEntryExitRouterRunRow?.playbookLabel ||
                                    selectedEntryExitPlaybookName ||
                                    'P?'}
                                </strong>
                                <small>{selectedPlaybookLinkStatus}</small>
                              </div>
                            </div>
                            <div className="pattern-family-selected-playbook-head-actions">
                              <div className="pattern-family-entry-dashboard-header-switch" aria-label="Select playbook">
                                {generatedEntryExitPlaybookTabRows.length ? (
                                  generatedEntryExitPlaybookTabRows.map((row) => (
                                    <button
                                      className={
                                        row.playbookKey === selectedEntryExitPlaybookKey
                                          ? 'pattern-family-entry-dashboard-header-button pattern-family-entry-dashboard-header-button--active'
                                          : 'pattern-family-entry-dashboard-header-button'
                                      }
                                      key={row.playbookName}
                                      onClick={() => {
                                        setSelectedEntryExitRouterRunId(row.run.router_run_id);
                                        setEntryExitProfileTab('model');
                                      }}
                                      title={`${row.playbookLabel} | ${row.playbookName}`}
                                      type="button"
                                    >
                                      {row.playbookLabel}
                                    </button>
                                  ))
                                ) : (
                                  <strong />
                                )}
                              </div>
                              <div className="pattern-family-selected-simulation-tabs pattern-family-selected-playbook-view-tabs" aria-label="Selected playbook view">
                                {[
                                  { id: 'dashboard', label: 'Dashboard' },
                                  { id: 'used', label: 'Used Plays' },
                                  { id: 'symbols', label: 'Symbols' },
                                  { id: 'raw', label: 'Raw Rows' },
                                ].map((tab) => {
                                  const isActive = selectedPlaybookView === tab.id;
                                  return (
                                    <button
                                      className={
                                        isActive
                                          ? 'pattern-family-selected-simulation-tab pattern-family-selected-simulation-tab--active'
                                          : 'pattern-family-selected-simulation-tab'
                                      }
                                      key={tab.id}
                                      onClick={() => setSelectedPlaybookView(tab.id)}
                                      type="button"
                                    >
                                      {tab.label}
                                    </button>
                                  );
                                })}
                              </div>
                            </div>
                          </header>
                          {selectedPlaybookView === 'raw' ? (
                            <section className="pattern-family-selected-playbook-used pattern-family-selected-playbook-raw">
                              <header>
                                <span>Raw Playbook Rows</span>
                                <small>{formatNumber(generatedEntryExitFamilyRouterRows.length)} family/template rows</small>
                              </header>
                              <div className="pattern-family-entry-dashboard-raw-table">
                                {generatedEntryExitFamilyRouterRows.length ? (
                                  <table>
                                    <thead>
                                      <tr>
                                        <th>#</th>
                                        <th>Family</th>
                                        <th>Template</th>
                                        <th>Status</th>
                                        <th>Train</th>
                                        <th>Train WR</th>
                                        <th>Train R</th>
                                        <th>Test</th>
                                        <th>Test WR</th>
                                        <th>Total R</th>
                                      </tr>
                                    </thead>
                                    <tbody>
                                      {generatedEntryExitFamilyRouterRows.map((row) => {
                                        const route = row.route;
                                        return (
                                          <tr key={`${route.family_key}-${route.template_uid}-${route.template_rank}`}>
                                            <td>{formatNumber(row.index)}</td>
                                            <td title={route.family_key}>
                                              {route.harmonic_type || 'Unknown'} | {route.market || 'N/A'} | {route.family_bin || 'Bin'}
                                            </td>
                                            <td title={route.template_name}>{route.template_label || compactText(route.template_uid, 8)}</td>
                                            <td>{route.route_status || 'N/A'}</td>
                                            <td>{formatNumber(route.train_eval_count)}</td>
                                            <td>{formatRatePercent(route.train_win_rate, 2)}%</td>
                                            <td className={Number(route.train_avg_r) < 0 ? 'is-loss' : 'is-win'}>{formatDecimal(route.train_avg_r, 4)}R</td>
                                            <td>{formatNumber(row.testEvalCount)}</td>
                                            <td>{formatDecimal(row.testPassRate, 2)}%</td>
                                            <td className={Number(route.test_sum_r) < 0 ? 'is-loss' : 'is-win'}>{formatDecimal(route.test_sum_r, 2)}R</td>
                                          </tr>
                                        );
                                      })}
                                    </tbody>
                                  </table>
                                ) : (
                                  <div className="pattern-family-selected-playbook-empty">No playbook rows loaded.</div>
                                )}
                              </div>
                            </section>
                          ) : selectedPlaybookView === 'used' ? (
                            <section className="pattern-family-selected-playbook-used pattern-family-selected-playbook-used--view">
                              <header>
                                <span>Used Plays</span>
                                <small>
                                  {hasSelectedEntryExitRouterRun
                                    ? `${formatNumber(selectedPlaybookUsedPlayRows.length)} entry/exit templates`
                                    : ''}
                                </small>
                              </header>
                              <div className="pattern-family-selected-playbook-used-scroll">
                                <div className="pattern-family-selected-playbook-list">
                                  {selectedPlaybookUsedPlayRows.length ? (
                                    selectedPlaybookUsedPlayRows.map((play) => (
                                      <div
                                        className="pattern-family-selected-playbook-row"
                                        key={play.templateUid}
                                        title={`${play.templateUid} | ${play.name}`}
                                      >
                                        <span>{play.label}</span>
                                        <small>{play.name}</small>
                                        <strong>{formatNumber(play.familyCount)} families</strong>
                                      </div>
                                    ))
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">No used plays loaded.</div>
                                  )}
                                </div>
                              </div>
                            </section>
                          ) : selectedPlaybookView === 'symbols' ? (
                            <section className="pattern-family-selected-playbook-used pattern-family-selected-playbook-symbols">
                              <header>
                                <span>Symbols</span>
                                <small>
                                  {selectedPlaybookSymbolRows.length
                                    ? `${formatNumber(selectedPlaybookTradeSymbolCount)} trade / ${formatNumber(selectedPlaybookSkippedSymbolCount)} skip`
                                    : 'No symbol gate rows loaded'}
                                </small>
                              </header>
                              <div className="pattern-family-entry-dashboard-raw-table">
                                {selectedPlaybookSymbolRows.length ? (
                                  <table>
                                    <thead>
                                      <tr>
                                        <th>Root</th>
                                        <th>Status</th>
                                        <th>Train Tests</th>
                                        <th>Train WR</th>
                                        <th>Train Avg R</th>
                                        <th>Train Sum R</th>
                                        <th>Reason</th>
                                      </tr>
                                    </thead>
                                    <tbody>
                                      {selectedPlaybookSymbolRows.map((symbol) => {
                                        const isSkipped = symbol.route_status !== 'TRADE';
                                        return (
                                          <tr
                                            key={`${symbol.router_run_id}-${symbol.root_symbol}`}
                                            title={`${symbol.root_symbol} | ${symbol.status_reason || ''}`}
                                          >
                                            <td className="pattern-family-template-table-id">{symbol.root_symbol || 'N/A'}</td>
                                            <td className={isSkipped ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                              {symbol.route_status || 'N/A'}
                                            </td>
                                            <td>{formatNumber(symbol.train_eval_count)}</td>
                                            <td>{formatRatePercent(symbol.train_win_rate, 2)}%</td>
                                            <td className={Number(symbol.train_avg_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                              {formatDecimal(symbol.train_avg_r, 4)}R
                                            </td>
                                            <td className={Number(symbol.train_sum_r) < 0 ? 'pattern-family-template-table-loss' : 'pattern-family-template-table-win'}>
                                              {formatDecimal(symbol.train_sum_r, 2)}R
                                            </td>
                                            <td className="pattern-family-template-table-reason">{symbol.status_reason || 'Selected by symbol gate'}</td>
                                          </tr>
                                        );
                                      })}
                                    </tbody>
                                  </table>
                                ) : (
                                  <div className="pattern-family-selected-playbook-empty">
                                    No symbol filter rows loaded for this playbook.
                                  </div>
                                )}
                              </div>
                            </section>
                          ) : (
                            <div className="pattern-family-selected-playbook-dashboard-body">
                          <div className="pattern-family-selected-playbook-logic-grid pattern-family-selected-playbook-dashboard-card-grid">
                            {[
                              {
                                label: 'Plays',
                                value: hasSelectedEntryExitRouterRun ? formatNumber(selectedPlaybookPlaysCount) : '',
                                detail: 'assigned templates',
                              },
                              {
                                label: 'Trade',
                                value: hasSelectedEntryExitRouterRun ? formatNumber(selectedPlaybookTradeCount) : '',
                                detail: 'trade families',
                              },
                              {
                                label: 'Watch',
                                value: hasSelectedEntryExitRouterRun ? formatNumber(selectedPlaybookWatchCount) : '',
                                detail: 'watchlist families',
                              },
                              {
                                label: 'Skipped',
                                value: hasSelectedEntryExitRouterRun ? formatNumber(selectedPlaybookSkipCount) : '',
                                detail: 'blocked families',
                              },
                              {
                                label: 'Families',
                                value: hasSelectedEntryExitRouterRun ? formatNumber(selectedPlaybookRowCount) : '',
                                detail: 'playbook rows',
                              },
                            ].map((item) => (
                              <div
                                className="pattern-family-selected-playbook-logic-card"
                                key={item.label}
                              >
                                <span>{item.label}</span>
                                <strong>{item.value}</strong>
                                <small>{item.detail}</small>
                              </div>
                            ))}
                          </div>
                          <section className="pattern-family-selected-playbook-assignment-chart">
                            <header>
                              <div>
                                <span>Template Usage</span>
                                <small>Build templates assigned into this playbook</small>
                              </div>
                              <strong>
                                {selectedPlaybookTemplateTotal
                                  ? `${formatNumber(selectedPlaybookAssignedTemplateCount)} / ${formatNumber(
                                      selectedPlaybookTemplateTotal
                                    )}`
                                  : formatNumber(selectedPlaybookAssignedTemplateCount)}
                              </strong>
                            </header>
                            <div className="pattern-family-selected-playbook-assignment-summary">
                              <div className="pattern-family-selected-playbook-logic-card pattern-family-selected-playbook-assignment-summary-card">
                                <span>Assigned Templates</span>
                                <strong>{formatNumber(selectedPlaybookAssignedTemplateCount)}</strong>
                                <small>used by trade families</small>
                              </div>
                              <div className="pattern-family-selected-playbook-logic-card pattern-family-selected-playbook-assignment-summary-card">
                                <span>Unused Templates</span>
                                <strong>{selectedPlaybookTemplateTotal ? formatNumber(selectedPlaybookUnassignedTemplateCount) : ''}</strong>
                                <small>built but not selected</small>
                              </div>
                              <div className="pattern-family-selected-playbook-logic-card pattern-family-selected-playbook-assignment-summary-card">
                                <span>Assignment Rate</span>
                                <strong>{selectedPlaybookTemplateTotal ? `${formatDecimal(selectedPlaybookAssignmentRate, 1)}%` : ''}</strong>
                                <small>assigned / total build</small>
                              </div>
                            </div>
                            <div className="pattern-family-selected-playbook-assignment-meter">
                              <div>
                                <span>Overall Assignment Rate</span>
                                <strong>{selectedPlaybookTemplateTotal ? `${formatDecimal(selectedPlaybookAssignmentRate, 1)}%` : ''}</strong>
                              </div>
                              <b aria-hidden="true">
                                <i style={{ width: `${Math.max(0, Math.min(100, selectedPlaybookAssignmentRate))}%` }} />
                              </b>
                            </div>
                            <div className="pattern-family-selected-playbook-assignment-bars">
                              {selectedPlaybookAssignmentChartRows.length ? (
                                selectedPlaybookAssignmentChartRows.map((play) => {
                                  const familyCount = Number(play.familyCount || 0);
                                  const barWidth = (familyCount / selectedPlaybookAssignmentMaxFamilyCount) * 100;
                                  return (
                                    <div
                                      className="pattern-family-selected-playbook-assignment-bar-row"
                                      key={play.templateUid}
                                      title={`${play.label} | ${play.name} | ${formatNumber(familyCount)} families`}
                                    >
                                      <span>{play.label}</span>
                                      <div>
                                        <i style={{ width: `${Math.max(2, Math.min(100, barWidth))}%` }} />
                                      </div>
                                      <strong>{formatNumber(familyCount)}</strong>
                                    </div>
                                  );
                                })
                              ) : (
                                <div className="pattern-family-selected-playbook-empty">No assigned templates loaded.</div>
                              )}
                            </div>
                          </section>
                          <section className="pattern-family-selected-playbook-description">
                            <header>
                              <span>Strategy Description</span>
                              <small>{selectedEntryExitPlaybookRunRow?.run.router_run_id ?? ''}</small>
                            </header>
                            {selectedEntryExitPlaybookRuleSections.length ? (
                              <div className="pattern-family-selected-playbook-rule-sections">
                                {selectedEntryExitPlaybookRuleSections.map((section) => (
                                  <article className="pattern-family-selected-playbook-rule-section" key={section.title}>
                                    <header>
                                      <span>{section.title}</span>
                                      <small>{section.detail}</small>
                                    </header>
                                    <div className="pattern-family-selected-playbook-rule-grid">
                                      {section.items.map((item) => (
                                        <div
                                          className={[
                                            'pattern-family-selected-playbook-logic-card',
                                            'pattern-family-selected-playbook-rule-card',
                                            item.tone ? `pattern-family-selected-playbook-logic-card--${item.tone}` : '',
                                          ].filter(Boolean).join(' ')}
                                          key={`${section.title}-${item.label}`}
                                        >
                                          <span>{item.label}</span>
                                          <strong title={item.value}>{item.value}</strong>
                                          <small title={item.detail}>{item.detail}</small>
                                        </div>
                                      ))}
                                    </div>
                                  </article>
                                ))}
                              </div>
                            ) : (
                              <p>No playbook description available for this row yet.</p>
                            )}
                          </section>
                          <section className="pattern-family-selected-playbook-rule-section pattern-family-selected-playbook-rule-impact">
                            <header>
                              <span>Rule Impact</span>
                            </header>
                            <div className="pattern-family-selected-playbook-rule-grid">
                              {[
                                {
                                  label: 'Overlap Skips',
                                  value: hasSelectedEntryExitLogicRun ? formatNumber(selectedEntryExitLogicRunRow?.run.skipped_overlap_patterns || 0) : '',
                                  detail: hasSelectedEntryExitLogicRun ? 'trade gate' : '',
                                  tone: Number(selectedEntryExitLogicRunRow?.run.skipped_overlap_patterns || 0) ? 'skipped' : '',
                                },
                                {
                                  label: 'Symbol Gate',
                                  value: selectedPlaybookSymbolGateLabel,
                                  detail: selectedPlaybookSymbolGateDetail,
                                  tone: selectedEntryExitLogicRunRow?.run.symbol_filter_enabled ? 'skipped' : '',
                                },
                                {
                                  label: 'Symbol Skips',
                                  value: hasSelectedEntryExitLogicRun ? formatNumber(selectedEntryExitLogicRunRow?.run.skipped_symbol_patterns || 0) : '',
                                  detail: hasSelectedEntryExitLogicRun ? 'symbol gate' : '',
                                  tone: Number(selectedEntryExitLogicRunRow?.run.skipped_symbol_patterns || 0) ? 'skipped' : '',
                                },
                              ].map((item) => (
                                <div
                                  className={[
                                    'pattern-family-selected-playbook-logic-card',
                                    'pattern-family-selected-playbook-rule-card',
                                    item.tone ? `pattern-family-selected-playbook-logic-card--${item.tone}` : '',
                                  ].filter(Boolean).join(' ')}
                                  key={item.label}
                                >
                                  <span>{item.label}</span>
                                  <strong title={item.value}>{item.value}</strong>
                                  <small title={item.detail}>{item.detail}</small>
                                </div>
                              ))}
                            </div>
                          </section>
                            </div>
                          )}
                        </aside>

                        <aside
                          className={[
                            'pattern-family-selected-playbook-panel',
                            'pattern-family-selected-simulation-panel',
                          ].filter(Boolean).join(' ')}
                        >
                          <header className="pattern-family-selected-playbook-head">
                            <span>Simulation Testing</span>
                            <div className="pattern-family-selected-simulation-head-actions">
                              <div className="pattern-family-selected-simulation-head-row pattern-family-selected-simulation-head-row--mode">
                                <div className="pattern-family-selected-simulation-station-buttons" aria-label="Simulation mode">
                                  {[
                                    { id: 'propFirm', label: 'Prop Firm' },
                                    { id: 'dayTrading', label: 'Day Trading' },
                                  ].map((mode) => (
                                    <button
                                      className={
                                        entryExitSimulationMode === mode.id
                                          ? 'pattern-family-selected-simulation-station-button pattern-family-selected-simulation-station-button--active'
                                          : 'pattern-family-selected-simulation-station-button'
                                      }
                                      key={mode.id}
                                      onClick={() => setEntryExitSimulationMode(mode.id)}
                                      type="button"
                                    >
                                      {mode.label}
                                    </button>
                                  ))}
                                </div>
                                <div className="pattern-family-selected-simulation-year-tabs" aria-label="Simulation test year">
                                  {selectedPlaybookTestYearRows.length ? (
                                    selectedPlaybookTestYearRows.map((yearRow) => (
                                      <button
                                        className={
                                          selectedEntryExitSimulationYear === yearRow.key
                                            ? 'pattern-family-selected-simulation-year-tab pattern-family-selected-simulation-year-tab--active'
                                            : 'pattern-family-selected-simulation-year-tab'
                                        }
                                        key={yearRow.key}
                                        onClick={() => setSelectedEntryExitSimulationYearKey(yearRow.key)}
                                        title={`${yearRow.label} simulation test`}
                                        type="button"
                                      >
                                        {yearRow.label}
                                      </button>
                                    ))
                                  ) : (
                                    <span>No Year</span>
                                  )}
                                </div>
                                <div className="pattern-family-selected-simulation-tabs" aria-label="Simulation testing view">
                                  {[
                                    { id: 'overview', label: 'Overview' },
                                    { id: 'plays', label: 'Sim Plays' },
                                    { id: 'rawTrades', label: 'Raw Trades' },
                                    { id: 'aiTrades', label: 'AI Trades' },
                                  ].map((tab) => (
                                    <button
                                      className={
                                        entryExitSimulationTab === tab.id
                                          ? 'pattern-family-selected-simulation-tab pattern-family-selected-simulation-tab--active'
                                          : 'pattern-family-selected-simulation-tab'
                                      }
                                      key={tab.id}
                                      onClick={() => setEntryExitSimulationTab(tab.id)}
                                      type="button"
                                    >
                                      {tab.label}
                                    </button>
                                  ))}
                                </div>
                              </div>
                            </div>
                          </header>
                          <div className={`pattern-family-selected-simulation-body pattern-family-selected-simulation-body--${entryExitSimulationTab}`}>
                            {entryExitSimulationTab === 'overview' ? (
                              <>
                                {entryExitSimulationMode === 'dayTrading' ? (
                                  <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-day-trading pattern-family-selected-simulation-chart--collapsible">
                                    <summary>
                                      <span>Day Trading Simulation</span>
                                      <small>
                                        {isEntryExitDayTradingSimLoading
                                          ? 'Loading day trading summary'
                                          : selectedDayTradingSummary
                                            ? `${formatNumber(selectedDayTradingSummary.total_trades)} trades | ${formatDecimal(selectedDayTradingSummary.total_r, 1)}R`
                                            : entryExitDayTradingSimError || 'No day trading summary loaded'}
                                      </small>
                                    </summary>
                                    {selectedDayTradingSummary ? (
                                      <div className="pattern-family-selected-simulation-day-trading-grid">
                                        {[
                                          {
                                            label: 'Net R',
                                            value: `${formatDecimal(selectedDayTradingSummary.total_r, 1)}R`,
                                            detail: `${formatDecimal(selectedDayTradingSummary.avg_r, 3)}R avg trade`,
                                            tone: selectedDayTradingSummary.total_r >= 0 ? 'win' : 'loss',
                                          },
                                          {
                                            label: 'Win Rate',
                                            value: `${formatDecimal(selectedDayTradingSummary.win_rate, 1)}%`,
                                            detail: `${formatNumber(selectedDayTradingSummary.wins)}W / ${formatNumber(selectedDayTradingSummary.losses)}L`,
                                            tone: selectedDayTradingSummary.win_rate >= 50 ? 'win' : 'loss',
                                          },
                                          {
                                            label: 'Max Drawdown',
                                            value: `${formatDecimal(selectedDayTradingSummary.max_drawdown_r, 1)}R`,
                                            detail: `${formatDecimal(selectedDayTradingSummary.peak_equity_r, 1)}R peak`,
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Profit Factor',
                                            value: formatDecimal(selectedDayTradingSummary.profit_factor, 2),
                                            detail: `${formatDecimal(selectedDayTradingSummary.gross_profit_r, 1)}R gross win`,
                                            tone: selectedDayTradingSummary.profit_factor >= 1 ? 'win' : 'loss',
                                          },
                                          {
                                            label: 'Trading Days',
                                            value: formatNumber(selectedDayTradingSummary.trading_days),
                                            detail: `${formatNumber(selectedDayTradingSummary.profitable_days)} green / ${formatNumber(selectedDayTradingSummary.losing_days)} red`,
                                          },
                                          {
                                            label: 'Avg Day',
                                            value: `${formatDecimal(selectedDayTradingSummary.avg_day_r, 2)}R`,
                                            detail: `${formatDecimal(selectedDayTradingSummary.best_day_r, 1)}R best / ${formatDecimal(selectedDayTradingSummary.worst_day_r, 1)}R worst`,
                                            tone: selectedDayTradingSummary.avg_day_r >= 0 ? 'win' : 'loss',
                                          },
                                          {
                                            label: 'Max Trades / Day',
                                            value: formatNumber(selectedDayTradingSummary.max_trades_per_day),
                                            detail: `${formatNumber(selectedDayTradingSummary.total_trades)} total trades`,
                                          },
                                          {
                                            label: 'Trade Duration',
                                            value: formatGapDuration(selectedDayTradingSummary.median_trade_duration_minutes),
                                            detail: `${formatGapDuration(selectedDayTradingSummary.avg_trade_duration_minutes)} avg`,
                                          },
                                        ].map((item) => (
                                          <div
                                            className={[
                                              'pattern-family-selected-simulation-day-trading-card',
                                              item.tone ? `pattern-family-selected-simulation-day-trading-card--${item.tone}` : '',
                                            ].filter(Boolean).join(' ')}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                            <small>{item.detail}</small>
                                          </div>
                                        ))}
                                      </div>
                                    ) : (
                                      <div className="pattern-family-selected-playbook-empty">
                                        {isEntryExitDayTradingSimLoading
                                          ? 'Loading day trading simulation...'
                                          : entryExitDayTradingSimError || 'No day trading simulation run has been created yet.'}
                                      </div>
                                    )}
                                  </details>
                                ) : (
                                  <>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-overview pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Playbook Test Overview</span>
                                    <small>{selectedEntryExitSimulationTestId || 'No simulation test selected'}</small>
                                  </summary>
                                  {selectedSimulationRunOverviewSections.length ? (
                                    <div className="pattern-family-selected-simulation-run-overview pattern-family-selected-simulation-run-overview--playbook">
                                      {selectedSimulationRunOverviewSections.map((section) => (
                                        <section
                                          className={[
                                            'pattern-family-selected-simulation-run-section',
                                            section.layout ? `pattern-family-selected-simulation-run-section--${section.layout}` : '',
                                          ].filter(Boolean).join(' ')}
                                          key={section.title}
                                        >
                                          <header>
                                            <span>{section.title}</span>
                                          </header>
                                          <div className="pattern-family-selected-simulation-run-grid">
                                            {section.items.map((item) => (
                                              <div
                                                className={[
                                                  'pattern-family-selected-simulation-run-card',
                                                  item.wide ? 'pattern-family-selected-simulation-run-card--wide' : '',
                                                  item.tone ? `pattern-family-selected-simulation-run-card--${item.tone}` : '',
                                                ].filter(Boolean).join(' ')}
                                                key={`${section.title}-${item.label}`}
                                              >
                                                <span>{item.label}</span>
                                                <strong title={item.title || item.value}>{item.value}</strong>
                                              </div>
                                            ))}
                                          </div>
                                        </section>
                                      ))}
                                    </div>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">No simulation run row loaded.</div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-overview pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Prop Simulation Overview</span>
                                    <small>{selectedEntryExitSimulationTestId || 'No simulation test selected'}</small>
                                  </summary>
                                  {selectedPropSimulationOverviewSections.length ? (
                                    <div className="pattern-family-selected-simulation-run-overview">
                                      {selectedPropSimulationOverviewSections.map((section) => (
                                        <section className="pattern-family-selected-simulation-run-section" key={section.title}>
                                          <header>
                                            <span>{section.title}</span>
                                          </header>
                                          <div className="pattern-family-selected-simulation-run-grid">
                                            {section.items.map((item) => (
                                              <div
                                                className={[
                                                  'pattern-family-selected-simulation-run-card',
                                                  item.tone ? `pattern-family-selected-simulation-run-card--${item.tone}` : '',
                                                ].filter(Boolean).join(' ')}
                                                key={`${section.title}-${item.label}`}
                                              >
                                                <span>{item.label}</span>
                                                <strong title={item.value}>{item.value}</strong>
                                              </div>
                                            ))}
                                          </div>
                                        </section>
                                      ))}
                                      <section className="pattern-family-selected-simulation-run-section pattern-family-selected-simulation-run-section--outcomes">
                                        <header>
                                          <span>Prop Cycle Outcomes</span>
                                        </header>
                                        {selectedSimulationOutcomeSegments.length ? (
                                          <>
                                            <div
                                              className="pattern-family-selected-simulation-outcome-bar"
                                              aria-label="Prop cycle outcome distribution"
                                            >
                                              {selectedSimulationOutcomeSegments.map((segment) => (
                                                <div
                                                  className={`pattern-family-selected-simulation-outcome-segment pattern-family-selected-simulation-outcome-segment--${segment.tone}`}
                                                  key={segment.label}
                                                  style={{ width: `${Math.max(segment.percent, segment.value ? 3 : 0)}%` }}
                                                  title={`${segment.label}: ${formatNumber(segment.value)} (${formatDecimal(segment.percent, 1)}%)`}
                                                />
                                              ))}
                                            </div>
                                            <div className="pattern-family-selected-simulation-outcome-grid">
                                              <div className="pattern-family-selected-simulation-outcome-card">
                                                <span>Total Cycles</span>
                                                <strong>{formatNumber(selectedSimulationOutcomeTotal)}</strong>
                                                <small>100.0%</small>
                                              </div>
                                              {selectedSimulationOutcomeSegments.map((segment) => (
                                                <div
                                                  className={`pattern-family-selected-simulation-outcome-card pattern-family-selected-simulation-outcome-card--${segment.tone}`}
                                                  key={segment.label}
                                                >
                                                  <span>{segment.label}</span>
                                                  <strong>{formatNumber(segment.value)}</strong>
                                                  <small>{formatDecimal(segment.percent, 1)}%</small>
                                                </div>
                                              ))}
                                            </div>
                                          </>
                                        ) : (
                                          <div className="pattern-family-selected-playbook-empty">No prop cycle outcome data loaded.</div>
                                        )}
                                      </section>
                                    </div>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">No prop simulation summary loaded.</div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-templates pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Template Performance</span>
                                    <small>
                                      {hasSelectedEntryExitRouterRun
                                        ? `${formatNumber(selectedSimulationTemplatePerformanceRows.length)} templates ranked by total R`
                                        : ''}
                                    </small>
                                  </summary>
                                  <div className="pattern-family-selected-simulation-template-table">
                                    {selectedSimulationTemplatePerformanceRows.length ? (
                                      <table>
                                        <thead>
                                          <tr>
                                            <th>Template</th>
                                            <th>Rule</th>
                                            <th>Tests</th>
                                            <th>W</th>
                                            <th>L</th>
                                            <th>No Entry</th>
                                            <th>Win %</th>
                                            <th>Avg R</th>
                                            <th>Total R</th>
                                            <th>Families</th>
                                          </tr>
                                        </thead>
                                        <tbody>
                                          {selectedSimulationTemplatePerformanceRows.map((play) => (
                                            <tr key={play.templateUid} title={`${play.templateUid} | ${play.name}`}>
                                              <td>{play.label}</td>
                                              <td>{play.name}</td>
                                              <td>{formatNumber(play.evalCount)}</td>
                                              <td className="is-win">{formatNumber(play.passCount)}</td>
                                              <td className="is-loss">{formatNumber(play.failCount)}</td>
                                              <td>{formatNumber(play.noEntryCount)}</td>
                                              <td className={Number(play.winRate) >= 50 ? 'is-win' : 'is-warning'}>
                                                {formatDecimal(play.winRate, 1)}%
                                              </td>
                                              <td className={Number(play.avgR) < 0 ? 'is-loss' : 'is-win'}>
                                                {formatDecimal(play.avgR, 3)}R
                                              </td>
                                              <td className={Number(play.sumR) < 0 ? 'is-loss' : 'is-win'}>
                                                {formatDecimal(play.sumR, 2)}R
                                              </td>
                                              <td>{formatNumber(play.familyCount)}</td>
                                            </tr>
                                          ))}
                                        </tbody>
                                      </table>
                                    ) : (
                                      <div className="pattern-family-selected-playbook-empty">No template performance loaded.</div>
                                    )}
                                  </div>
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-equity pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Equity Curve In R</span>
                                    <small>
                                      {isEntryExitSimEquityLoading
                                        ? 'Loading curve'
                                        : selectedSimulationEquityPoints.length
                                          ? `${formatNumber(selectedSimulationEquityPoints.length)} points`
                                          : entryExitSimEquityError || 'No curve loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationEquityPoints.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Current R',
                                            value: `${formatDecimal(selectedSimulationEquityLast?.cumulative_r ?? 0, 1)}R`,
                                            tone: Number(selectedSimulationEquityLast?.cumulative_r ?? 0) < 0 ? 'loss' : 'win',
                                          },
                                          {
                                            label: 'Peak R',
                                            value: `${formatDecimal(selectedSimulationEquityPeak, 1)}R`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Worst DD',
                                            value: `${formatDecimal(selectedSimulationEquityWorstDrawdown, 1)}R`,
                                            tone: 'loss',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <svg
                                        className="pattern-family-selected-simulation-equity-svg"
                                        viewBox={`0 0 ${selectedSimulationEquityWidth} ${selectedSimulationEquityHeight}`}
                                        role="img"
                                        aria-label="Cumulative R equity curve"
                                        preserveAspectRatio="none"
                                      >
                                        <line
                                          className="pattern-family-selected-simulation-equity-zero"
                                          x1={selectedSimulationEquityPadding.left}
                                          x2={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y1={selectedSimulationEquityZeroY}
                                          y2={selectedSimulationEquityZeroY}
                                        />
                                        <path
                                          className="pattern-family-selected-simulation-equity-line"
                                          d={selectedSimulationEquityPath}
                                        />
                                        <circle
                                          className="pattern-family-selected-simulation-equity-dot"
                                          cx={getSelectedSimulationEquityX(selectedSimulationEquityPoints.length - 1)}
                                          cy={getSelectedSimulationEquityY(selectedSimulationEquityLast?.cumulative_r ?? 0)}
                                          r="4"
                                        />
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityPadding.top + 4}
                                          textAnchor="end"
                                        >
                                          {formatNumber(Math.round(selectedSimulationEquityMax))}R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityHeight - selectedSimulationEquityPadding.bottom + 4}
                                          textAnchor="end"
                                        >
                                          {formatNumber(Math.round(selectedSimulationEquityMin))}R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityPadding.left}
                                          y={selectedSimulationEquityHeight - 8}
                                        >
                                          {selectedSimulationEquityStartLabel}
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y={selectedSimulationEquityHeight - 8}
                                          textAnchor="end"
                                        >
                                          {selectedSimulationEquityEndLabel}
                                        </text>
                                      </svg>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimEquityLoading
                                        ? 'Loading equity curve...'
                                        : entryExitSimEquityError || 'No equity curve data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-daily-r pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Daily R Bars</span>
                                    <small>
                                      {isEntryExitSimDailyRLoading
                                        ? 'Loading daily bars'
                                        : selectedSimulationDailyRows.length
                                          ? `${formatNumber(selectedSimulationDailyRows.length)} days | -${formatDecimal(
                                              selectedSimulationDailyLossLimit,
                                              0
                                            )}R limit`
                                          : entryExitSimDailyRError || 'No daily R loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationDailyRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Best Day',
                                            value: `${formatDecimal(selectedSimulationBestDay?.total_r ?? 0, 1)}R`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Worst Day',
                                            value: `${formatDecimal(selectedSimulationWorstDay?.total_r ?? 0, 1)}R`,
                                            tone:
                                              Number(selectedSimulationWorstDay?.worst_intraday_r ?? 0) <=
                                              -selectedSimulationDailyLossLimit
                                                ? 'loss'
                                                : 'skipped',
                                          },
                                          {
                                            label: 'Daily Hits',
                                            value: formatNumber(selectedSimulationDailyLossHitRows.length),
                                            tone: selectedSimulationDailyLossHitRows.length ? 'loss' : 'win',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <svg
                                        className="pattern-family-selected-simulation-equity-svg pattern-family-selected-simulation-daily-r-svg"
                                        viewBox={`0 0 ${selectedSimulationEquityWidth} ${selectedSimulationEquityHeight}`}
                                        role="img"
                                        aria-label="Daily net R bars"
                                        preserveAspectRatio="none"
                                      >
                                        <line
                                          className="pattern-family-selected-simulation-equity-zero"
                                          x1={selectedSimulationEquityPadding.left}
                                          x2={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y1={selectedSimulationDailyZeroY}
                                          y2={selectedSimulationDailyZeroY}
                                        />
                                        <line
                                          className="pattern-family-selected-simulation-daily-r-limit"
                                          x1={selectedSimulationEquityPadding.left}
                                          x2={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y1={selectedSimulationDailyLossLimitY}
                                          y2={selectedSimulationDailyLossLimitY}
                                        />
                                        {selectedSimulationDailyRows.map((day, index) => {
                                          const totalR = Number(day.total_r || 0);
                                          const isPositive = totalR >= 0;
                                          const barValueY = getSelectedSimulationDailyY(totalR);
                                          const barY = isPositive ? barValueY : selectedSimulationDailyZeroY;
                                          const barHeight = Math.max(1, Math.abs(selectedSimulationDailyZeroY - barValueY));
                                          const hitDailyLoss =
                                            day.hit_daily_loss ||
                                            Number(day.worst_intraday_r || 0) <= -selectedSimulationDailyLossLimit;
                                          const tradeDate = String(day.trade_date || '').slice(0, 10);
                                          const isSelectedDay = tradeDate === selectedSimulationDailyDate;

                                          return (
                                            <g key={`${tradeDate}-${index}`}>
                                              <rect
                                                className={[
                                                  'pattern-family-selected-simulation-daily-r-bar',
                                                  isPositive
                                                    ? 'pattern-family-selected-simulation-daily-r-bar--win'
                                                    : 'pattern-family-selected-simulation-daily-r-bar--loss',
                                                  hitDailyLoss
                                                    ? 'pattern-family-selected-simulation-daily-r-bar--hit'
                                                    : '',
                                                  isSelectedDay
                                                    ? 'pattern-family-selected-simulation-daily-r-bar--selected'
                                                    : '',
                                                ]
                                                  .filter(Boolean)
                                                  .join(' ')}
                                                x={getSelectedSimulationDailyX(index)}
                                                y={barY}
                                                width={selectedSimulationDailyBarWidth}
                                                height={barHeight}
                                                onClick={() => setSelectedSimulationDailyDate(tradeDate)}
                                              >
                                                <title>
                                                  {`${tradeDate} | Net ${formatDecimal(totalR, 2)}R | ${formatNumber(
                                                    day.wins
                                                  )}W / ${formatNumber(day.losses)}L | Trades ${formatNumber(
                                                    day.trades
                                                  )} | Best ${formatDecimal(day.best_trade_r, 2)}R | Worst ${formatDecimal(
                                                    day.worst_trade_r,
                                                    2
                                                  )}R | Intraday low ${formatDecimal(day.worst_intraday_r, 2)}R | TP level ${formatDecimal(
                                                    day.tp_progress_pct,
                                                    1
                                                  )}%`}
                                                </title>
                                              </rect>
                                              {hitDailyLoss ? (
                                                <circle
                                                  className="pattern-family-selected-simulation-daily-r-hit-dot"
                                                  cx={getSelectedSimulationDailyX(index) + selectedSimulationDailyBarWidth / 2}
                                                  cy={selectedSimulationDailyLossLimitY}
                                                  r="3"
                                                />
                                              ) : null}
                                            </g>
                                          );
                                        })}
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityPadding.top + 4}
                                          textAnchor="end"
                                        >
                                          {formatDecimal(selectedSimulationDailyMax, 0)}R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-daily-r-limit-label"
                                          x={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y={selectedSimulationDailyLossLimitY - 8}
                                          textAnchor="end"
                                        >
                                          -10R daily loss
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityHeight - selectedSimulationEquityPadding.bottom + 4}
                                          textAnchor="end"
                                        >
                                          {formatDecimal(selectedSimulationDailyMin, 0)}R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityPadding.left}
                                          y={selectedSimulationEquityHeight - 8}
                                        >
                                          {selectedSimulationDailyStartLabel}
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y={selectedSimulationEquityHeight - 8}
                                          textAnchor="end"
                                        >
                                          {selectedSimulationDailyEndLabel}
                                        </text>
                                      </svg>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimDailyRLoading
                                        ? 'Loading daily R bars...'
                                        : entryExitSimDailyRError || 'No daily R data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-drawdown pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Daily Drawdown Bars</span>
                                    <small>
                                      {isEntryExitSimDailyRLoading
                                        ? 'Loading drawdown bars'
                                        : selectedSimulationDailyRows.length
                                          ? `${formatNumber(selectedSimulationDailyRows.length)} days | ${formatDecimal(
                                              selectedSimulationDailyLossLimit,
                                              0
                                            )}R daily limit`
                                          : entryExitSimDailyRError || 'No drawdown loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationDailyRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Current DD',
                                            value: `${formatDecimal(selectedSimulationDrawdownCurrent, 1)}R`,
                                            tone: selectedSimulationDrawdownCurrent >= selectedSimulationDrawdownLimit ? 'loss' : 'skipped',
                                          },
                                          {
                                            label: 'Worst Day DD',
                                            value: `${formatDecimal(
                                              Math.max(0, -Number(selectedSimulationWorstDrawdownDay?.worst_intraday_r || 0)),
                                              1
                                            )}R`,
                                            tone:
                                              Math.max(0, -Number(selectedSimulationWorstDrawdownDay?.worst_intraday_r || 0)) >=
                                              selectedSimulationDailyLossLimit
                                                ? 'loss'
                                                : 'skipped',
                                          },
                                          {
                                            label: 'Daily Hits',
                                            value: formatNumber(selectedSimulationDailyLossHitRows.length),
                                            tone: selectedSimulationDailyLossHitRows.length ? 'loss' : 'win',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <svg
                                        className="pattern-family-selected-simulation-equity-svg pattern-family-selected-simulation-daily-r-svg pattern-family-selected-simulation-drawdown-svg"
                                        viewBox={`0 0 ${selectedSimulationEquityWidth} ${selectedSimulationEquityHeight}`}
                                        role="img"
                                        aria-label="Daily drawdown bars in R"
                                        preserveAspectRatio="none"
                                      >
                                        <line
                                          className="pattern-family-selected-simulation-equity-zero"
                                          x1={selectedSimulationEquityPadding.left}
                                          x2={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y1={selectedSimulationDailyDrawdownZeroY}
                                          y2={selectedSimulationDailyDrawdownZeroY}
                                        />
                                        <line
                                          className="pattern-family-selected-simulation-daily-r-limit"
                                          x1={selectedSimulationEquityPadding.left}
                                          x2={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y1={selectedSimulationDailyDrawdownBarLimitY}
                                          y2={selectedSimulationDailyDrawdownBarLimitY}
                                        />
                                        {selectedSimulationDailyRows.map((day, index) => {
                                          const drawdownR = Math.max(0, -Number(day.worst_intraday_r || 0));
                                          const barValueY = getSelectedSimulationDailyDrawdownY(drawdownR);
                                          const barHeight = Math.max(1, Math.abs(selectedSimulationDailyDrawdownZeroY - barValueY));
                                          const hitDailyLoss = day.hit_daily_loss || drawdownR >= selectedSimulationDailyLossLimit;
                                          const tradeDate = String(day.trade_date || '').slice(0, 10);
                                          const isSelectedDay = tradeDate === selectedSimulationDailyDate;

                                          return (
                                            <g key={`${tradeDate}-drawdown-${index}`}>
                                              <rect
                                                className={[
                                                  'pattern-family-selected-simulation-daily-r-bar',
                                                  'pattern-family-selected-simulation-daily-r-bar--loss',
                                                  hitDailyLoss ? 'pattern-family-selected-simulation-daily-r-bar--hit' : '',
                                                  isSelectedDay ? 'pattern-family-selected-simulation-daily-r-bar--selected' : '',
                                                ].filter(Boolean).join(' ')}
                                                x={getSelectedSimulationDailyX(index)}
                                                y={selectedSimulationDailyDrawdownZeroY}
                                                width={selectedSimulationDailyBarWidth}
                                                height={barHeight}
                                                role="button"
                                                tabIndex="0"
                                                onClick={() => setSelectedSimulationDailyDate(tradeDate)}
                                                onKeyDown={(event) => {
                                                  if (event.key === 'Enter' || event.key === ' ') {
                                                    event.preventDefault();
                                                    setSelectedSimulationDailyDate(tradeDate);
                                                  }
                                                }}
                                              >
                                                <title>
                                                  {`${tradeDate} | worst drawdown ${formatDecimal(drawdownR, 2)}R | net ${formatDecimal(
                                                    day.total_r,
                                                    2
                                                  )}R | DD level ${formatDecimal(day.drawdown_progress_pct, 1)}%`}
                                                </title>
                                              </rect>
                                              {hitDailyLoss ? (
                                                <circle
                                                  className="pattern-family-selected-simulation-daily-r-hit-dot"
                                                  cx={getSelectedSimulationDailyX(index) + selectedSimulationDailyBarWidth / 2}
                                                  cy={selectedSimulationDailyDrawdownBarLimitY}
                                                  r="3"
                                                />
                                              ) : null}
                                            </g>
                                          );
                                        })}
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityPadding.top + 4}
                                          textAnchor="end"
                                        >
                                          0R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-daily-r-limit-label"
                                          x={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y={selectedSimulationDailyDrawdownBarLimitY - 8}
                                          textAnchor="end"
                                        >
                                          10R daily loss
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis"
                                          x={selectedSimulationEquityPadding.left - 14}
                                          y={selectedSimulationEquityHeight - selectedSimulationEquityPadding.bottom + 4}
                                          textAnchor="end"
                                        >
                                          {formatDecimal(selectedSimulationDailyDrawdownChartMax, 0)}R
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityPadding.left}
                                          y={selectedSimulationEquityHeight - 8}
                                        >
                                          {selectedSimulationDailyStartLabel}
                                        </text>
                                        <text
                                          className="pattern-family-selected-simulation-equity-axis pattern-family-selected-simulation-equity-axis--date"
                                          x={selectedSimulationEquityWidth - selectedSimulationEquityPadding.right}
                                          y={selectedSimulationEquityHeight - 8}
                                          textAnchor="end"
                                        >
                                          {selectedSimulationDailyEndLabel}
                                        </text>
                                      </svg>
                                      <div className="pattern-family-selected-simulation-drawdown-legend">
                                        <span>
                                          <i className="pattern-family-selected-simulation-drawdown-legend-dot pattern-family-selected-simulation-drawdown-legend-dot--drawdown" />
                                          Worst Intraday
                                        </span>
                                        <span>
                                          <i className="pattern-family-selected-simulation-drawdown-legend-dot pattern-family-selected-simulation-drawdown-legend-dot--daily" />
                                          Daily Loss
                                        </span>
                                      </div>
                                      <div className="pattern-family-selected-simulation-daily-trades">
                                        <header>
                                          <span>
                                            {selectedSimulationDailyDate
                                              ? `${selectedSimulationDailyDate} Trades`
                                              : 'Daily Trade Detail'}
                                          </span>
                                          <small>
                                            {selectedSimulationDailyDate
                                              ? `${formatNumber(selectedSimulationDailyTradeRows.length)} trades | ${formatDecimal(
                                                  selectedSimulationDailyRow?.total_r ?? selectedSimulationDailyTradeNetR,
                                                  1
                                                )}R`
                                              : 'No day selected'}
                                          </small>
                                        </header>
                                        {selectedSimulationDailyDate ? (
                                          selectedSimulationDailyTradeRows.length ? (
                                            <div className="pattern-family-selected-simulation-daily-trades-table">
                                              <table>
                                                <thead>
                                                  <tr>
                                                    <th>Time</th>
                                                    <th>Symbol</th>
                                                    <th>Family</th>
                                                    <th>Template</th>
                                                    <th>Dir</th>
                                                    <th>Result</th>
                                                    <th>TP After</th>
                                                    <th>TP Chg</th>
                                                    <th>DD After</th>
                                                    <th>DD Chg</th>
                                                    <th>Exit</th>
                                                  </tr>
                                                </thead>
                                                <tbody>
                                                  {selectedSimulationDailyTradeRows.map((trade) => {
                                                    const resultR = Number(trade.result_r || 0);
                                                    const direction = String(trade.trade_direction || '').toUpperCase();
                                                    return (
                                                      <tr
                                                        className={resultR >= 0 ? 'is-win' : 'is-loss'}
                                                        key={`${trade.id}-${trade.setup_id}`}
                                                      >
                                                        <td>{formatTime(trade.entry_date ?? trade.d_confirm_date)}</td>
                                                        <td>{trade.symbol || 'N/A'}</td>
                                                        <td title={trade.family_key}>{compactText(trade.family_key || 'N/A', 12)}</td>
                                                        <td title={trade.template_uid}>
                                                          {trade.template_label || compactText(trade.template_uid || 'N/A', 8)}
                                                        </td>
                                                        <td>{direction || 'N/A'}</td>
                                                        <td>{formatDecimal(resultR, 2)}R</td>
                                                        <td title={`${formatDecimal(trade.cycle_equity_r_before ?? 0, 2)}R -> ${formatDecimal(trade.cycle_equity_r_after ?? 0, 2)}R`}>
                                                          {trade.tp_progress_pct_after === null || trade.tp_progress_pct_after === undefined
                                                            ? ''
                                                            : `${formatDecimal(trade.tp_progress_pct_after, 1)}%`}
                                                        </td>
                                                        <td>
                                                          {trade.tp_progress_pct_delta === null || trade.tp_progress_pct_delta === undefined
                                                            ? ''
                                                            : formatSignedPercent(trade.tp_progress_pct_delta)}
                                                        </td>
                                                        <td title={`${formatDecimal(trade.cycle_drawdown_r_before ?? 0, 2)}R -> ${formatDecimal(trade.cycle_drawdown_r_after ?? 0, 2)}R`}>
                                                          {trade.drawdown_progress_pct_after === null || trade.drawdown_progress_pct_after === undefined
                                                            ? ''
                                                            : `${formatDecimal(trade.drawdown_progress_pct_after, 1)}%`}
                                                        </td>
                                                        <td>
                                                          {trade.drawdown_progress_pct_delta === null ||
                                                          trade.drawdown_progress_pct_delta === undefined
                                                            ? ''
                                                            : formatSignedPercent(trade.drawdown_progress_pct_delta)}
                                                        </td>
                                                        <td>{formatRouteMode(trade.exit_reason || trade.outcome || 'N/A')}</td>
                                                      </tr>
                                                    );
                                                  })}
                                                </tbody>
                                              </table>
                                            </div>
                                          ) : (
                                            <div className="pattern-family-selected-playbook-empty">
                                              {isEntryExitSimDailyTradesLoading
                                                ? 'Loading daily trades...'
                                                : entryExitSimDailyTradesError || 'No trades loaded for selected day.'}
                                            </div>
                                          )
                                        ) : (
                                          <div className="pattern-family-selected-playbook-empty">No day selected.</div>
                                        )}
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimDailyRLoading
                                        ? 'Loading drawdown bars...'
                                        : entryExitSimDailyRError || 'No drawdown bar data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-hourly pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Time-of-Day Performance</span>
                                    <small>
                                      {isEntryExitSimHourlyLoading
                                        ? 'Loading hours'
                                        : selectedSimulationHourlyRows.length
                                          ? `${formatNumber(selectedSimulationHourlyRows.length)} active hours`
                                          : entryExitSimHourlyError || 'No hourly data loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationHourlyRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Best Hour',
                                            value: `${formatHourLabel(selectedSimulationBestHour?.entry_hour)} / ${formatDecimal(
                                              selectedSimulationBestHour?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Worst Hour',
                                            value: `${formatHourLabel(selectedSimulationWorstHour?.entry_hour)} / ${formatDecimal(
                                              selectedSimulationWorstHour?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Most Bad-Day Trades',
                                            value: `${formatHourLabel(selectedSimulationMostDangerHour?.entry_hour)} / ${formatNumber(
                                              selectedSimulationMostDangerHour?.daily_loss_day_trades ?? 0
                                            )}`,
                                            tone: 'skipped',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-hourly-table">
                                        <table>
                                          <thead>
                                            <tr>
                                              <th>Hour</th>
                                              <th>Trades</th>
                                              <th>WR</th>
                                              <th>Avg R</th>
                                              <th>Net R</th>
                                              <th>Best</th>
                                              <th>Worst</th>
                                              <th>Bad-Day</th>
                                            </tr>
                                          </thead>
                                          <tbody>
                                            {selectedSimulationHourlyRows.map((hour) => {
                                              const netR = Number(hour.sum_r || 0);
                                              return (
                                                <tr className={netR >= 0 ? 'is-win' : 'is-loss'} key={hour.entry_hour}>
                                                  <td>{formatHourLabel(hour.entry_hour)}</td>
                                                  <td>{formatNumber(hour.trades)}</td>
                                                  <td>{formatDecimal(hour.win_rate, 1)}%</td>
                                                  <td>{formatDecimal(hour.avg_r, 3)}R</td>
                                                  <td>{formatDecimal(netR, 1)}R</td>
                                                  <td>{formatDecimal(hour.best_r, 1)}R</td>
                                                  <td>{formatDecimal(hour.worst_r, 1)}R</td>
                                                  <td>
                                                    {formatNumber(hour.daily_loss_day_trades)} /{' '}
                                                    {formatNumber(hour.daily_loss_day_count)}
                                                  </td>
                                                </tr>
                                              );
                                            })}
                                          </tbody>
                                        </table>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimHourlyLoading
                                        ? 'Loading time-of-day performance...'
                                        : entryExitSimHourlyError || 'No hourly performance data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-cadence pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Trade Cadence</span>
                                    <small>
                                      {isEntryExitSimTradeCadenceLoading
                                        ? 'Loading cadence'
                                        : selectedSimulationTradeCadence
                                          ? `${formatNumber(selectedSimulationTradeCadence.trades)} trades | ${formatDecimal(
                                              selectedSimulationTradeCadence.median_gap_minutes,
                                              1
                                            )}m median gap`
                                          : entryExitSimTradeCadenceError || 'No cadence loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationTradeCadence ? (
                                    <>
                                      <section className="pattern-family-selected-simulation-cadence-section">
                                        <header>
                                          <span>Time Between Trades</span>
                                        </header>
                                        <div className="pattern-family-selected-simulation-pressure-summary">
                                          <section className="pattern-family-selected-simulation-pressure-panel pattern-family-selected-simulation-pressure-panel--spacing">
                                            <header>
                                              <span>Overview</span>
                                            </header>
                                            <div className="pattern-family-selected-simulation-spacing-context">
                                              <div>
                                                <span>Measured Gaps</span>
                                                <strong>{formatNumber(selectedSimulationTradeCadence.gap_count)}</strong>
                                              </div>
                                              <div>
                                                <span>Date Range</span>
                                                <strong>
                                                  {formatDate(selectedSimulationTradeCadence.first_trade_at)} -{' '}
                                                  {formatDate(selectedSimulationTradeCadence.last_trade_at)}
                                                </strong>
                                              </div>
                                            </div>
                                            <div className="pattern-family-selected-simulation-spacing-hero">
                                              <div>
                                                <span>Typical Wait</span>
                                                <strong>{formatGapDuration(selectedSimulationTradeCadence.median_gap_minutes)}</strong>
                                                <small>median gap between trade entries</small>
                                              </div>
                                              <div>
                                                <span>Average Wait</span>
                                                <strong>{formatGapDuration(selectedSimulationTradeCadence.avg_gap_minutes)}</strong>
                                                <small>pulled higher by long quiet periods</small>
                                              </div>
                                            </div>
                                            <div className="pattern-family-selected-simulation-spacing-range">
                                              <div>
                                                <span>Fastest repeat</span>
                                                <strong>{formatGapDuration(selectedSimulationTradeCadence.min_gap_minutes)}</strong>
                                              </div>
                                              <div>
                                                <span>Longest pause</span>
                                                <strong>{formatGapDuration(selectedSimulationTradeCadence.max_gap_minutes)}</strong>
                                              </div>
                                            </div>
                                          </section>
                                          <section className="pattern-family-selected-simulation-pressure-panel pattern-family-selected-simulation-pressure-panel--buckets">
                                            <header>
                                              <span>Gap Buckets</span>
                                            </header>
                                            <div className="pattern-family-selected-simulation-pressure-table pattern-family-selected-simulation-pressure-table--embedded">
                                              <table>
                                                <thead>
                                                  <tr>
                                                    <th>Gap Range</th>
                                                    <th>Times Seen</th>
                                                    <th>Share</th>
                                                  </tr>
                                                </thead>
                                                <tbody>
                                                  {selectedSimulationTradeCadenceBuckets.map((bucket) => (
                                                    <tr key={bucket.label}>
                                                      <td>{bucket.label}</td>
                                                      <td>{formatNumber(bucket.count)}</td>
                                                      <td>{formatDecimal(bucket.percent, 1)}%</td>
                                                    </tr>
                                                  ))}
                                                </tbody>
                                              </table>
                                            </div>
                                          </section>
                                        </div>
                                        <div className="pattern-family-selected-simulation-timeline-workload-row">
                                          <div className="pattern-family-selected-simulation-timeline-slot">
                                            {selectedSimulationTradeGapRows.length ? (
                                              <div className="pattern-family-selected-simulation-gap-timeline">
                                                <header>
                                                  <span>Vertical Trade Timeline</span>
                                                  <small>Linear time scale. Empty vertical space represents minutes with no trades.</small>
                                                </header>
                                                <div className="pattern-family-selected-simulation-gap-vertical">
                                                  {selectedSimulationTradeTimelinePoints.slice(0, 600).map((point, index, points) => {
                                                    const gapMinutes = Number(point.gap_minutes || 0);
                                                    const spacerHeight =
                                                      index === 0 ? 0 : Math.max(2, Math.round(gapMinutes * 0.6));
                                                    const tradeDate = formatDate(point.event_at);
                                                    const previousDate = index > 0 ? formatDate(points[index - 1]?.event_at) : '';
                                                    const showDayDivider = index === 0 || tradeDate !== previousDate;
                                                    const showCycleDivider = index > 0 && point.starts_new_cycle;
                                                    return (
                                                      <React.Fragment key={`${point.sequence_number}-${point.event_at}`}>
                                                        {showDayDivider ? (
                                                          <div
                                                            className="pattern-family-selected-simulation-gap-vertical-day"
                                                            style={{ marginTop: index === 0 ? 0 : spacerHeight }}
                                                          >
                                                            <span>{tradeDate}</span>
                                                          </div>
                                                        ) : null}
                                                        {showCycleDivider ? (
                                                          <div
                                                            className="pattern-family-selected-simulation-gap-vertical-cycle"
                                                            style={{ marginTop: showDayDivider ? 0 : spacerHeight }}
                                                          >
                                                            <span>
                                                              End test {formatNumber(point.previous_cycle_number)} / Start test{' '}
                                                              {formatNumber(point.cycle_number)}
                                                            </span>
                                                          </div>
                                                        ) : null}
                                                        <div
                                                          className="pattern-family-selected-simulation-gap-vertical-item"
                                                          style={{ marginTop: showDayDivider || showCycleDivider ? 0 : spacerHeight }}
                                                        >
                                                          <span className="pattern-family-selected-simulation-gap-vertical-time">
                                                            {formatTime(point.event_at)}
                                                          </span>
                                                          <span className="pattern-family-selected-simulation-gap-vertical-dot" />
                                                          <span className="pattern-family-selected-simulation-gap-vertical-gap">
                                                            {point.gap_minutes === null
                                                              ? 'first trade'
                                                              : `${formatGapDuration(gapMinutes)} since previous`}
                                                          </span>
                                                        </div>
                                                      </React.Fragment>
                                                    );
                                                  })}
                                                </div>
                                                <div className="pattern-family-selected-simulation-gap-timeline-legend">
                                                  <span><i /> trade</span>
                                                  <span>scale: 1 minute = 0.6px</span>
                                                  {selectedSimulationTradeTimelinePoints.length > 600 ? (
                                                    <span>showing first 600 of {formatNumber(selectedSimulationTradeTimelinePoints.length)}</span>
                                                  ) : null}
                                                </div>
                                              </div>
                                            ) : (
                                              <div className="pattern-family-selected-playbook-empty">
                                                {isEntryExitSimTradeGapLoading
                                                  ? 'Loading trade gap timeline...'
                                                  : entryExitSimTradeGapError || 'No trade gap timeline data loaded.'}
                                              </div>
                                            )}
                                          </div>
                                          <section className="pattern-family-selected-simulation-cadence-section pattern-family-selected-simulation-cadence-section--workload">
                                            <header>
                                              <span>Trade Workload</span>
                                            </header>
                                            {selectedSimulationTradeWorkloadCards.length ? (
                                              <div className="pattern-family-selected-simulation-workload-board">
                                                {selectedSimulationTradeWorkloadCards.map((card) => (
                                                  <article className={`pattern-family-selected-simulation-workload-tile pattern-family-selected-simulation-workload-tile--${card.key}`} key={card.key}>
                                                    <header>
                                                      <span>{card.title}</span>
                                                    </header>
                                                    <div className="pattern-family-selected-simulation-workload-average">
                                                      <strong>{card.value}</strong>
                                                      <span>{card.unit}</span>
                                                    </div>
                                                    <div className="pattern-family-selected-simulation-workload-stat-list">
                                                      {card.stats.map((stat) => (
                                                        <div key={`${card.key}-${stat.label}`}>
                                                          <span>{stat.label}</span>
                                                          <strong>{stat.value}</strong>
                                                        </div>
                                                      ))}
                                                    </div>
                                                  </article>
                                                ))}
                                              </div>
                                            ) : (
                                              <div className="pattern-family-selected-playbook-empty">
                                                {isEntryExitSimTradeWorkloadLoading
                                                  ? 'Loading trade workload...'
                                                  : entryExitSimTradeWorkloadError || 'No trade workload data loaded.'}
                                              </div>
                                            )}
                                          </section>
                                        </div>
                                      </section>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimTradeCadenceLoading
                                        ? 'Loading trade cadence...'
                                        : entryExitSimTradeCadenceError || 'No trade cadence data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-test-frequency pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Test Frequency</span>
                                    <small>
                                      {isEntryExitSimTestFrequencyLoading
                                        ? 'Loading test frequency'
                                        : selectedSimulationTestFrequencyRows.length
                                          ? `${formatNumber(selectedSimulationTestFrequencyRows.length)} tests | ${formatDecimal(
                                              selectedSimulationAvgTradesPerTest,
                                              1
                                            )} avg trades / test`
                                          : entryExitSimTestFrequencyError || 'No test frequency data loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationTestFrequencyRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Avg Trades / Test',
                                            value: formatDecimal(selectedSimulationAvgTradesPerTest, 1),
                                            tone: 'skipped',
                                          },
                                          {
                                            label: 'Max Trades / Test',
                                            value: formatNumber(selectedSimulationMaxTestTrades),
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Avg Test Length',
                                            value: formatGapDuration(selectedSimulationAvgTestDurationMinutes),
                                            tone: 'skipped',
                                          },
                                          {
                                            label: 'Longest Test',
                                            value: formatGapDuration(selectedSimulationLongestTestMinutes),
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Closed Tests',
                                            value: `${formatNumber(selectedSimulationTestFrequencyCompletedRows.length)} / ${formatNumber(
                                              selectedSimulationTestFrequencyRows.length
                                            )}`,
                                            tone: 'win',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-frequency-table">
                                        <div className="pattern-family-selected-simulation-frequency-scroll" tabIndex={0}>
                                          <table>
                                            <thead>
                                              <tr>
                                                <th>Test</th>
                                                <th>Outcome</th>
                                                <th>Start</th>
                                                <th>End</th>
                                                <th>Length</th>
                                                <th>Days</th>
                                                <th>Trade Days</th>
                                                <th>Trades</th>
                                                <th>Events</th>
                                                <th>Avg / Day</th>
                                                <th>Avg / Hr</th>
                                                <th>Net R</th>
                                                <th>Max DD</th>
                                              </tr>
                                            </thead>
                                            <tbody>
                                              {selectedSimulationTestFrequencyRows.map((test) => {
                                                const netR = Number(test.sum_r || 0);
                                                return (
                                                  <tr className={netR >= 0 ? 'is-win' : 'is-loss'} key={test.cycle_number}>
                                                    <td>{formatNumber(test.cycle_number)}</td>
                                                    <td>{formatTrendLabel(test.outcome)}</td>
                                                    <td>{formatShortDateTime(test.start_at)}</td>
                                                    <td>{formatShortDateTime(test.end_at)}</td>
                                                    <td>{formatGapDuration(test.duration_minutes)}</td>
                                                    <td>{formatNumber(test.calendar_days)}</td>
                                                    <td>{formatNumber(test.active_trade_days)}</td>
                                                    <td>{formatNumber(test.trades)}</td>
                                                    <td>{formatNumber(test.events)}</td>
                                                    <td>{formatDecimal(test.avg_trades_per_active_day, 1)}</td>
                                                    <td>{formatDecimal(test.avg_trades_per_hour, 1)}</td>
                                                    <td>{formatDecimal(netR, 1)}R</td>
                                                    <td>{formatDecimal(test.max_drawdown_r, 1)}R</td>
                                                  </tr>
                                                );
                                              })}
                                            </tbody>
                                          </table>
                                        </div>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimTestFrequencyLoading
                                        ? 'Loading test frequency...'
                                        : entryExitSimTestFrequencyError || 'No test frequency rows loaded for this sim yet.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-loss-cluster pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Loss Behavior</span>
                                    <small>
                                      {isEntryExitSimLossClusterLoading
                                        ? 'Loading loss clustering'
                                        : selectedSimulationLossSummary
                                          ? `${formatNumber(selectedSimulationLossSummary.losses)} losses | ${formatDecimal(
                                              selectedSimulationLossSummary.clustered_60m_rate,
                                              1
                                            )}% within 60m`
                                          : entryExitSimLossClusterError || 'No loss behavior loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationLossSummary ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Max Loss Streak',
                                            value: `${formatNumber(selectedSimulationLossSummary.max_loss_streak)}L`,
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Median Loss Gap',
                                            value: `${formatDecimal(
                                              selectedSimulationLossSummary.median_loss_gap_minutes,
                                              1
                                            )}m`,
                                            tone:
                                              Number(selectedSimulationLossSummary.median_loss_gap_minutes || 0) <= 60
                                                ? 'loss'
                                                : 'win',
                                          },
                                          {
                                            label: 'Same-Hour Pairs',
                                            value: `${formatNumber(
                                              selectedSimulationLossSummary.clustered_60m_loss_pairs
                                            )} / ${formatDecimal(selectedSimulationLossSummary.clustered_60m_rate, 1)}%`,
                                            tone:
                                              Number(selectedSimulationLossSummary.clustered_60m_rate || 0) >= 50
                                                ? 'loss'
                                                : 'skipped',
                                          },
                                          {
                                            label: '5+ Loss Days',
                                            value: `${formatNumber(selectedSimulationLossSummary.loss_days_5_plus)} / ${formatNumber(
                                              selectedSimulationLossSummary.loss_days
                                            )}`,
                                            tone: selectedSimulationLossSummary.loss_days_5_plus ? 'loss' : 'win',
                                          },
                                          {
                                            label: 'Worst Loss Day',
                                            value: `${formatDate(selectedSimulationLossSummary.worst_loss_day)} / ${formatNumber(
                                              selectedSimulationLossSummary.worst_loss_day_losses
                                            )}L`,
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Worst Hour',
                                            value: `${formatDate(selectedSimulationLossSummary.worst_loss_hour_date)} ${formatHourLabel(
                                              selectedSimulationLossSummary.worst_loss_hour ?? 0
                                            )} / ${formatNumber(selectedSimulationLossSummary.worst_loss_hour_losses)}L`,
                                            tone: 'loss',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-cadence-buckets">
                                        {selectedSimulationLossBuckets.map((bucket) => (
                                          <div className="pattern-family-selected-simulation-cadence-bucket" key={bucket.bucket_key}>
                                            <span>{bucket.bucket_label}</span>
                                            <div>
                                              <i style={{ width: `${bucket.gap_count ? Math.max(3, bucket.gap_percent) : 0}%` }} />
                                            </div>
                                            <strong>
                                              {formatNumber(bucket.gap_count)} / {formatDecimal(bucket.gap_percent, 1)}%
                                            </strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-contribution-table">
                                        <table>
                                          <thead>
                                            <tr>
                                              <th>Date</th>
                                              <th>Hour</th>
                                              <th>Losses</th>
                                              <th>Wins</th>
                                              <th>Loss Rate</th>
                                              <th>Net R</th>
                                              <th>Top Symbol</th>
                                              <th>Top Family</th>
                                              <th>Top Test</th>
                                            </tr>
                                          </thead>
                                          <tbody>
                                            {selectedSimulationLossWindows.slice(0, 18).map((row) => (
                                              <tr className="is-loss" key={`${row.trade_date}-${row.entry_hour}`}>
                                                <td>{formatDate(row.trade_date)}</td>
                                                <td>{formatHourLabel(row.entry_hour)}</td>
                                                <td>{formatNumber(row.losses)}</td>
                                                <td>{formatNumber(row.wins)}</td>
                                                <td>{formatDecimal(row.loss_rate, 1)}%</td>
                                                <td>{formatDecimal(row.total_r, 1)}R</td>
                                                <td>{row.top_root_symbol || 'N/A'}</td>
                                                <td title={row.top_family_key}>{compactText(row.top_family_key || 'N/A', 14)}</td>
                                                <td title={row.top_template_uid}>{compactText(row.top_template_uid || 'N/A', 12)}</td>
                                              </tr>
                                            ))}
                                          </tbody>
                                        </table>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimLossClusterLoading
                                        ? 'Loading loss behavior...'
                                        : entryExitSimLossClusterError || 'No loss clustering data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-contribution pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Symbol Contribution</span>
                                    <small>
                                      {isEntryExitSimSymbolContributionLoading
                                        ? 'Loading symbols'
                                        : selectedSimulationSymbolContributionRows.length
                                          ? `${formatNumber(selectedSimulationSymbolContributionRows.length)} symbols`
                                          : entryExitSimSymbolContributionError || 'No symbol contribution loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationSymbolContributionRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Top Symbol',
                                            value: `${selectedSimulationTopSymbol?.root_symbol || 'N/A'} / ${formatDecimal(
                                              selectedSimulationTopSymbol?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Worst Symbol',
                                            value: `${selectedSimulationWorstSymbol?.root_symbol || 'N/A'} / ${formatDecimal(
                                              selectedSimulationWorstSymbol?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: Number(selectedSimulationWorstSymbol?.sum_r ?? 0) < 0 ? 'loss' : 'skipped',
                                          },
                                          {
                                            label: 'Bad-Day Leader',
                                            value: `${selectedSimulationMostDangerSymbol?.root_symbol || 'N/A'} / ${formatNumber(
                                              selectedSimulationMostDangerSymbol?.daily_loss_day_trades ?? 0
                                            )}`,
                                            tone: 'skipped',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-family-chart">
                                        <header>
                                          <span>Top Families By Net R</span>
                                          <small>{formatNumber(selectedSimulationFamilyPerformanceRows.length)} shown</small>
                                        </header>
                                        {selectedSimulationFamilyPerformanceRows.map((row) => {
                                          const barPercent = Math.max(
                                            0,
                                            (Number(row.netR || 0) / selectedSimulationFamilyPerformanceMaxR) * 100
                                          );
                                          return (
                                            <div
                                              className="pattern-family-selected-simulation-family-bar-row"
                                              key={row.family_key}
                                              title={`${row.family_key} | ${row.label}`}
                                            >
                                              <div className="pattern-family-selected-simulation-family-bar-head">
                                                <span>
                                                  #{String(row.rank).padStart(2, '0')} {row.label}
                                                </span>
                                                <strong>{formatDecimal(row.netR, 1)}R</strong>
                                              </div>
                                              <div className="pattern-family-selected-simulation-family-bar-track">
                                                <i style={{ width: `${barPercent ? Math.max(4, barPercent) : 0}%` }} />
                                              </div>
                                              <small>
                                                {formatNumber(row.trades)} trades | {formatDecimal(row.winRate, 1)}% WR |{' '}
                                                {formatDecimal(row.avgR, 3)}R avg
                                              </small>
                                            </div>
                                          );
                                        })}
                                      </div>
                                      <div className="pattern-family-selected-simulation-contribution-table">
                                        <div className="pattern-family-selected-simulation-table-actions">
                                          <span>Symbol Table</span>
                                          <button
                                            type="button"
                                            onClick={() => setExpandedSimulationTable('symbolContribution')}
                                          >
                                            Expand
                                          </button>
                                        </div>
                                        {renderSymbolContributionTable()}
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimSymbolContributionLoading
                                        ? 'Loading symbol contribution...'
                                        : entryExitSimSymbolContributionError || 'No symbol contribution data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-contribution pattern-family-selected-simulation-contribution--family pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Family Contribution</span>
                                    <small>
                                      {isEntryExitSimFamilyContributionLoading
                                        ? 'Loading families'
                                        : selectedSimulationFamilyContributionRows.length
                                          ? `${formatNumber(selectedSimulationFamilyContributionRows.length)} families`
                                          : entryExitSimFamilyContributionError || 'No family contribution loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationFamilyContributionRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Top Family',
                                            value: `${compactText(selectedSimulationTopFamily?.family_key || 'N/A', 10)} / ${formatDecimal(
                                              selectedSimulationTopFamily?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Worst Family',
                                            value: `${compactText(selectedSimulationWorstFamily?.family_key || 'N/A', 10)} / ${formatDecimal(
                                              selectedSimulationWorstFamily?.sum_r ?? 0,
                                              1
                                            )}R`,
                                            tone: Number(selectedSimulationWorstFamily?.sum_r ?? 0) < 0 ? 'loss' : 'skipped',
                                          },
                                          {
                                            label: 'Bad-Day Leader',
                                            value: `${compactText(selectedSimulationMostDangerFamily?.family_key || 'N/A', 10)} / ${formatNumber(
                                              selectedSimulationMostDangerFamily?.daily_loss_day_trades ?? 0
                                            )}`,
                                            tone: 'skipped',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-contribution-table">
                                        <div className="pattern-family-selected-simulation-table-actions">
                                          <span>Family Table</span>
                                          <button
                                            type="button"
                                            onClick={() => setExpandedSimulationTable('familyContribution')}
                                          >
                                            Expand
                                          </button>
                                        </div>
                                        {renderFamilyContributionTable()}
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimFamilyContributionLoading
                                        ? 'Loading family contribution...'
                                        : entryExitSimFamilyContributionError || 'No family contribution data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-streaks pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Win/Loss Streaks</span>
                                    <small>
                                      {isEntryExitSimStreakLoading
                                        ? 'Loading streaks'
                                        : selectedSimulationStreakRows.length
                                          ? `${formatNumber(selectedSimulationStreakRows.length)} streaks`
                                          : entryExitSimStreakError || 'No streaks loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationStreakRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Largest Loss',
                                            value: `${formatNumber(selectedSimulationLargestLossStreak?.streak_length || 0)}L`,
                                            tone: 'loss',
                                          },
                                          {
                                            label: 'Largest Win',
                                            value: `${formatNumber(selectedSimulationLargestWinStreak?.streak_length || 0)}W`,
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Loss Runs',
                                            value: formatNumber(selectedSimulationLossStreakRows.length),
                                            tone: 'skipped',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-streak-chart">
                                        {selectedSimulationStreakDistribution.map((bucket) => (
                                          <div className="pattern-family-selected-simulation-streak-bucket" key={bucket.length}>
                                            <span>{bucket.length}</span>
                                            <div className="pattern-family-selected-simulation-streak-bars">
                                              <div
                                                className="pattern-family-selected-simulation-streak-bar pattern-family-selected-simulation-streak-bar--win"
                                                style={{
                                                  height: `${Math.max(
                                                    bucket.wins
                                                      ? (bucket.wins / selectedSimulationMaxStreakBucketCount) * 100
                                                      : 0,
                                                    bucket.wins ? 8 : 0
                                                  )}%`,
                                                }}
                                                title={`${formatNumber(bucket.wins)} win streaks of length ${bucket.length}`}
                                              />
                                              <div
                                                className="pattern-family-selected-simulation-streak-bar pattern-family-selected-simulation-streak-bar--loss"
                                                style={{
                                                  height: `${Math.max(
                                                    bucket.losses
                                                      ? (bucket.losses / selectedSimulationMaxStreakBucketCount) * 100
                                                      : 0,
                                                    bucket.losses ? 8 : 0
                                                  )}%`,
                                                }}
                                                title={`${formatNumber(bucket.losses)} loss streaks of length ${bucket.length}`}
                                              />
                                            </div>
                                            <small>
                                              {formatNumber(bucket.wins)} / {formatNumber(bucket.losses)}
                                            </small>
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-streak-legend">
                                        <span><i className="pattern-family-selected-simulation-streak-dot pattern-family-selected-simulation-streak-dot--win" />Wins</span>
                                        <span><i className="pattern-family-selected-simulation-streak-dot pattern-family-selected-simulation-streak-dot--loss" />Losses</span>
                                        <small>Bucket label = streak length</small>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimStreakLoading
                                        ? 'Loading streak chart...'
                                        : entryExitSimStreakError || 'No streak data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-market-trends pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Market Trend Snapshot</span>
                                    <small>
                                      {isEntryExitSimMarketTrendLoading
                                        ? 'Loading market trends'
                                        : selectedSimulationMarketTrendPerformanceRows.length
                                          ? `${formatNumber(selectedSimulationMarketTrendPerformanceRows.length)} trend buckets | ${formatNumber(
                                              selectedSimulationMarketTrendAlignmentRows.length
                                            )} alignments`
                                          : entryExitSimMarketTrendError || 'No trend snapshot loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationMarketTrendPerformanceRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {[
                                          {
                                            label: 'Best Trend',
                                            value: selectedSimulationBestMarketTrend
                                              ? `${selectedSimulationBestMarketTrend.timeframe} ${formatTrendLabel(
                                                  selectedSimulationBestMarketTrend.trend_label
                                                )}`
                                              : 'N/A',
                                            detail: selectedSimulationBestMarketTrend
                                              ? `${formatDecimal(selectedSimulationBestMarketTrend.sum_r, 1)}R net`
                                              : '',
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Best Alignment',
                                            value: selectedSimulationBestMarketTrendAlignment
                                              ? `${formatTrendLabel(
                                                  selectedSimulationBestMarketTrendAlignment.trend_5m
                                                )} / ${formatTrendLabel(
                                                  selectedSimulationBestMarketTrendAlignment.trend_15m
                                                )} / ${formatTrendLabel(
                                                  selectedSimulationBestMarketTrendAlignment.trend_1h
                                                )}`
                                              : 'N/A',
                                            detail: selectedSimulationBestMarketTrendAlignment
                                              ? `${formatDecimal(selectedSimulationBestMarketTrendAlignment.sum_r, 1)}R net`
                                              : '',
                                            tone: 'win',
                                          },
                                          {
                                            label: 'Trend Samples',
                                            value: formatNumber(
                                              selectedSimulationMarketTrendPerformanceRows.reduce(
                                                (total, row) => total + Number(row.trades || 0),
                                                0
                                              )
                                            ),
                                            detail: '3 timeframes per trade',
                                            tone: 'skipped',
                                          },
                                        ].map((item) => (
                                          <div
                                            className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${item.tone}`}
                                            key={item.label}
                                            title={item.detail}
                                          >
                                            <span>{item.label}</span>
                                            <strong>{item.value}</strong>
                                            {item.detail ? <small>{item.detail}</small> : null}
                                          </div>
                                        ))}
                                      </div>
                                      <div className="pattern-family-selected-simulation-market-trend-grid">
                                        <div className="pattern-family-selected-simulation-contribution-table pattern-family-selected-simulation-market-trend-table">
                                          <table>
                                            <thead>
                                              <tr>
                                                <th>Timeframe</th>
                                                <th>Trend</th>
                                                <th>Trades</th>
                                                <th>WR</th>
                                                <th>Avg R</th>
                                                <th>Net R</th>
                                                <th>Strength</th>
                                                <th>Symbols</th>
                                              </tr>
                                            </thead>
                                            <tbody>
                                              {selectedSimulationMarketTrendPerformanceRows.map((row) => {
                                                const netR = Number(row.sum_r || 0);
                                                return (
                                                  <tr
                                                    className={netR >= 0 ? 'is-win' : 'is-loss'}
                                                    key={`${row.timeframe}-${row.trend_label}`}
                                                  >
                                                    <td>{row.timeframe}</td>
                                                    <td>{formatTrendLabel(row.trend_label)}</td>
                                                    <td>{formatNumber(row.trades)}</td>
                                                    <td>{formatDecimal(row.win_rate, 1)}%</td>
                                                    <td>{formatDecimal(row.avg_r, 3)}R</td>
                                                    <td>{formatDecimal(netR, 1)}R</td>
                                                    <td>{formatDecimal(row.avg_strength_pct, 1)}%</td>
                                                    <td>{formatNumber(row.symbol_count)}</td>
                                                  </tr>
                                                );
                                              })}
                                            </tbody>
                                          </table>
                                        </div>
                                        <div className="pattern-family-selected-simulation-contribution-table pattern-family-selected-simulation-market-trend-table">
                                          <table>
                                            <thead>
                                              <tr>
                                                <th>5m</th>
                                                <th>15m</th>
                                                <th>1h</th>
                                                <th>Trades</th>
                                                <th>WR</th>
                                                <th>Avg R</th>
                                                <th>Net R</th>
                                                <th>Best</th>
                                                <th>Worst</th>
                                              </tr>
                                            </thead>
                                            <tbody>
                                              {selectedSimulationMarketTrendAlignmentRows.map((row) => {
                                                const netR = Number(row.sum_r || 0);
                                                return (
                                                  <tr
                                                    className={netR >= 0 ? 'is-win' : 'is-loss'}
                                                    key={`${row.trend_5m}-${row.trend_15m}-${row.trend_1h}`}
                                                  >
                                                    <td>{formatTrendLabel(row.trend_5m)}</td>
                                                    <td>{formatTrendLabel(row.trend_15m)}</td>
                                                    <td>{formatTrendLabel(row.trend_1h)}</td>
                                                    <td>{formatNumber(row.trades)}</td>
                                                    <td>{formatDecimal(row.win_rate, 1)}%</td>
                                                    <td>{formatDecimal(row.avg_r, 3)}R</td>
                                                    <td>{formatDecimal(netR, 1)}R</td>
                                                    <td>{formatDecimal(row.best_r, 1)}R</td>
                                                    <td>{formatDecimal(row.worst_r, 1)}R</td>
                                                  </tr>
                                                );
                                              })}
                                            </tbody>
                                          </table>
                                        </div>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimMarketTrendLoading
                                        ? 'Loading market trend snapshot...'
                                        : entryExitSimMarketTrendError || 'No market trend data loaded.'}
                                    </div>
                                  )}
                                </details>
                                <details className="pattern-family-selected-simulation-chart pattern-family-selected-simulation-direction-trends pattern-family-selected-simulation-chart--collapsible">
                                  <summary>
                                    <span>Trade Direction vs HTF Trend</span>
                                    <small>
                                      {isEntryExitSimMarketTrendLoading
                                        ? 'Loading direction trend data'
                                        : selectedSimulationMarketTrendDirectionRows.length
                                          ? `${formatNumber(selectedSimulationMarketTrendDirectionRows.length)} direction buckets`
                                          : entryExitSimMarketTrendError || 'No direction trend data loaded'}
                                    </small>
                                  </summary>
                                  {selectedSimulationMarketTrendDirectionRows.length ? (
                                    <>
                                      <div className="pattern-family-selected-simulation-equity-stats">
                                        {selectedSimulationMarketTrendDirectionSummaryRows.map((row) => {
                                          const tone =
                                            row.trend_alignment === 'against_trend'
                                              ? 'loss'
                                              : row.trend_alignment === 'with_trend'
                                                ? 'win'
                                                : 'skipped';
                                          return (
                                            <div
                                              className={`pattern-family-selected-simulation-equity-stat pattern-family-selected-simulation-equity-stat--${tone}`}
                                              key={row.trend_alignment}
                                            >
                                              <span>{formatTrendAlignmentLabel(row.trend_alignment)}</span>
                                              <strong>{formatDecimal(row.avg_r, 3)}R</strong>
                                              <small>
                                                {formatDecimal(row.win_rate, 1)}% WR | {formatDecimal(row.sum_r, 1)}R
                                              </small>
                                            </div>
                                          );
                                        })}
                                      </div>
                                      <div className="pattern-family-selected-simulation-contribution-table pattern-family-selected-simulation-direction-trend-table">
                                        <table>
                                          <thead>
                                            <tr>
                                              <th>Timeframe</th>
                                              <th>Side</th>
                                              <th>Trend</th>
                                              <th>Alignment</th>
                                              <th>Trades</th>
                                              <th>Pass</th>
                                              <th>Fail</th>
                                              <th>WR</th>
                                              <th>Avg R</th>
                                              <th>Net R</th>
                                            </tr>
                                          </thead>
                                          <tbody>
                                            {selectedSimulationMarketTrendDirectionRows.map((row) => {
                                              const netR = Number(row.sum_r || 0);
                                              return (
                                                <tr
                                                  className={netR >= 0 ? 'is-win' : 'is-loss'}
                                                  key={`${row.timeframe}-${row.trade_direction}-${row.trend_label}-${row.trend_alignment}`}
                                                >
                                                  <td>{row.timeframe}</td>
                                                  <td>{row.trade_direction}</td>
                                                  <td>{formatTrendLabel(row.trend_label)}</td>
                                                  <td>{formatTrendAlignmentLabel(row.trend_alignment)}</td>
                                                  <td>{formatNumber(row.trades)}</td>
                                                  <td>{formatNumber(row.wins)}</td>
                                                  <td>{formatNumber(row.losses)}</td>
                                                  <td>{formatDecimal(row.win_rate, 1)}%</td>
                                                  <td>{formatDecimal(row.avg_r, 3)}R</td>
                                                  <td>{formatDecimal(netR, 1)}R</td>
                                                </tr>
                                              );
                                            })}
                                          </tbody>
                                        </table>
                                      </div>
                                    </>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimMarketTrendLoading
                                        ? 'Loading direction trend data...'
                                        : entryExitSimMarketTrendError || 'No direction trend data loaded.'}
                                    </div>
                                  )}
                                </details>
                                  </>
                                )}
                              </>
                            ) : entryExitSimulationTab === 'aiTrades' ? (
                              renderAiStage1TradesPanel()
                            ) : entryExitSimulationTab === 'rawTrades' ? (
                              <section className="pattern-family-selected-playbook-used pattern-family-selected-simulation-raw-trades">
                                <header>
                                  <span>Raw Trades</span>
                                  <small>
                                    {selectedSimulationRawTradeTotal
                                      ? `${formatNumber(selectedSimulationRawTradeTotal)} trades | showing ${formatNumber(selectedSimulationRawTradeRows.length)}`
                                      : selectedEntryExitSimulationTestId || ''}
                                  </small>
                                </header>
                                <div className="pattern-family-entry-dashboard-raw-table pattern-family-selected-simulation-raw-trades-table">
                                  {selectedSimulationRawTradeRows.length ? (
                                    <table>
                                      <thead>
                                        <tr>
                                          <th>#</th>
                                          <th>Time</th>
                                          <th>Symbol</th>
                                          <th>TF</th>
                                          <th>Family</th>
                                          <th>Play</th>
                                          <th>Dir</th>
                                          <th>R</th>
                                          <th>Outcome</th>
                                          <th>Exit</th>
                                          <th>Duration</th>
                                          <th>Entry</th>
                                          <th>Stop</th>
                                          <th>Target</th>
                                          <th>Exit Px</th>
                                          <th>TP %</th>
                                          <th>DD %</th>
                                        </tr>
                                      </thead>
                                      <tbody>
                                        {selectedSimulationRawTradeRows.map((trade, index) =>
                                          renderSimulationRawTradeRow(trade, index)
                                        )}
                                      </tbody>
                                    </table>
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">
                                      {isEntryExitSimRawTradesLoading
                                        ? 'Loading raw trades...'
                                        : entryExitSimRawTradesError || 'No raw trades loaded for this simulation.'}
                                    </div>
                                  )}
                                </div>
                              </section>
                            ) : (
                              <section className="pattern-family-selected-playbook-used pattern-family-selected-simulation-plays">
                                <header>
                                  <span>Sim Plays</span>
                                  <small>
                                    {hasSelectedEntryExitRouterRun
                                      ? `${formatNumber(selectedSimulationUsedPlayRows.length)} evaluated templates`
                                      : ''}
                                  </small>
                                </header>
                                <div className="pattern-family-selected-playbook-list">
                                  {selectedSimulationUsedPlayRows.length ? (
                                    selectedSimulationUsedPlayRows.map((play) => (
                                      <div
                                        className="pattern-family-selected-playbook-row"
                                        key={play.templateUid}
                                        title={`${play.templateUid} | ${play.name} | ${formatNumber(play.passCount)}W / ${formatNumber(
                                          play.failCount
                                        )}L / ${formatNumber(play.noEntryCount)} no entry`}
                                      >
                                        <span>{play.label}</span>
                                        <small>
                                          {formatDecimal(play.winRate, 1)}% WR | {formatDecimal(play.avgR, 3)}R |{' '}
                                          {formatNumber(play.familyCount)} families
                                        </small>
                                        <strong>{formatNumber(play.evalCount)} tests</strong>
                                      </div>
                                    ))
                                  ) : (
                                    <div className="pattern-family-selected-playbook-empty">No simulation plays loaded.</div>
                                  )}
                                </div>
                              </section>
                            )}
                          </div>
                        </aside>
                      </div>
                    </section>
                  ) : null}
              </section>
            </section>
          ) : testOverviewTab === 'patterns' ? (
            <section className="pattern-family-side-data-profile pattern-family-pattern-catalog-frame">
              <header className="pattern-family-pattern-catalog-head">
                <div>
                  <span>Pattern Catalog</span>
                  <strong>
                    {selectedPatternCatalogProfile
                      ? `${selectedPatternCatalogProfile.displayId} | ${selectedPatternCatalogProfile.source} ${selectedPatternCatalogProfile.timeframe}`
                      : 'No scan profile'}
                  </strong>
                </div>
                <small>{formatNumber(patternCatalogTotalPatterns)} patterns</small>
              </header>
              <div className="pattern-family-pattern-catalog" aria-label="Pattern catalog dashboard">
                {patternStorageError ? (
                  <div className="pattern-family-market-empty">{patternStorageError}</div>
                ) : isPatternStorageLoading ? (
                  <div className="pattern-family-market-empty">Loading pattern universe...</div>
                ) : (
                  <div className="pattern-family-pattern-catalog-shell">
                    <section className="pattern-family-pattern-catalog-topbar">
                      <header>
                        <div>
                          <span>Catalog Filters</span>
                          <strong>Scan Shelf</strong>
                        </div>
                        <small>
                          {formatNumber(filteredPatternCatalogProfiles.length)} shown / {formatNumber(patternCatalogProfiles.length)} total
                        </small>
                      </header>

                      <section className="pattern-family-pattern-catalog-filters" aria-label="Pattern catalog filters">
                        <div className="pattern-family-pattern-catalog-filter-group">
                          <span>Source</span>
                          <div>
                            {patternCatalogSourceOptions.map((option) => (
                              <button
                                className={patternCatalogSource === option ? 'pattern-family-pattern-catalog-filter--active' : ''}
                                key={option}
                                onClick={() => setPatternCatalogSource(option)}
                                type="button"
                              >
                                {option}
                              </button>
                            ))}
                          </div>
                        </div>
                        <div className="pattern-family-pattern-catalog-filter-group">
                          <span>Timeframe</span>
                          <div>
                            {patternCatalogTimeframeOptions.map((option) => (
                              <button
                                className={patternCatalogTimeframe === option ? 'pattern-family-pattern-catalog-filter--active' : ''}
                                key={option}
                                onClick={() => setPatternCatalogTimeframe(option)}
                                type="button"
                              >
                                {option}
                              </button>
                            ))}
                          </div>
                        </div>
                        <div className="pattern-family-pattern-catalog-filter-group">
                          <span>Fit</span>
                          <div>
                            {patternCatalogFitOptions.map((option) => (
                              <button
                                className={patternCatalogFit === option.key ? 'pattern-family-pattern-catalog-filter--active' : ''}
                                key={option.key}
                                onClick={() => setPatternCatalogFit(option.key)}
                                type="button"
                              >
                                {option.label}
                              </button>
                            ))}
                          </div>
                        </div>
                      </section>

                      <section className="pattern-family-pattern-catalog-profiles">
                        {filteredPatternCatalogProfiles.length ? (
                          filteredPatternCatalogProfiles.map((profile) => (
                            <button
                              className={[
                                'pattern-family-pattern-profile-card',
                                `pattern-family-pattern-profile-card--${profile.sourceKey}`,
                                selectedPatternCatalogProfile?.id === profile.id ? 'pattern-family-pattern-profile-card--active' : '',
                              ].join(' ')}
                              key={profile.id}
                              onClick={() => setSelectedPatternCatalogProfileId(profile.id)}
                              type="button"
                            >
                              <span>{profile.displayId}</span>
                              <strong>{profile.source} | {profile.timeframe}</strong>
                              <small>{profile.fitLabel}</small>
                              <div>
                                <b>{formatNumber(profile.setup_count)}</b>
                                <em>patterns</em>
                              </div>
                              <footer>
                                {formatNumber(profile.root_count)} roots | {formatNumber(profile.contract_count)} contracts
                              </footer>
                          </button>
                        ))
                      ) : (
                        <div className="pattern-family-market-empty">No scan profiles matched this filter.</div>
                      )}
                    </section>
                    </section>

                    <main className="pattern-family-pattern-catalog-workbench">
                      {selectedPatternCatalogProfile ? (
                        <>
                          <section className="pattern-family-pattern-catalog-hero">
                            <div className="pattern-family-pattern-catalog-hero-title">
                              <span>{selectedPatternCatalogProfile.displayId}</span>
                              <div>
                                <strong>
                                  {selectedPatternCatalogProfile.source} | {selectedPatternCatalogProfile.timeframe} |{' '}
                                  {selectedPatternCatalogProfile.fitLabel}
                                </strong>
                                <small>
                                  {formatDate(selectedPatternCatalogProfile.first_d_date)} to{' '}
                                  {formatDate(selectedPatternCatalogProfile.last_d_date)}
                                </small>
                              </div>
                            </div>
                            <div className="pattern-family-pattern-catalog-metrics">
                              <section>
                                <span>Patterns</span>
                                <strong>{formatNumber(selectedPatternCatalogProfile.setup_count)}</strong>
                                <small>selected scan universe</small>
                              </section>
                              <section>
                                <span>Roots</span>
                                <strong>{formatNumber(selectedPatternCatalogProfile.root_count)}</strong>
                                <small>tradable roots</small>
                              </section>
                              <section>
                                <span>Contracts</span>
                                <strong>{formatNumber(selectedPatternCatalogProfile.contract_count)}</strong>
                                <small>contract symbols</small>
                              </section>
                              <section>
                                <span>Venues</span>
                                <strong>{formatNumber(selectedPatternCatalogProfile.exchange_count)}</strong>
                                <small>exchange groups</small>
                              </section>
                            </div>
                          </section>

                          <section className="pattern-family-pattern-catalog-body-grid">
                            <section className="pattern-family-pattern-catalog-section pattern-family-pattern-catalog-section--symbols">
                              <header>
                                <div>
                                  <span>Exchange / Symbol Drilldown</span>
                                  <strong>{formatNumber(selectedPatternCatalogRootRows.length)} roots</strong>
                                </div>
                                <small>{formatNumber(selectedPatternCatalogExchangeSections.length)} groups</small>
                              </header>
                              {selectedPatternCatalogExchangeSections.length ? (
                                <div className="pattern-family-pattern-catalog-exchanges">
                                  {selectedPatternCatalogExchangeSections.map((section) => (
                                    <div
                                      className={[
                                        'pattern-family-pattern-catalog-exchange',
                                        `pattern-family-pattern-catalog-exchange--${getExchangeClassSuffix(section.exchange)}`,
                                      ].join(' ')}
                                      key={section.exchange}
                                    >
                                      <header>
                                        <span>{section.exchange}</span>
                                        <strong>{formatNumber(section.setupCount)}</strong>
                                        <small>{formatNumber(section.rows.length)} roots</small>
                                      </header>
                                      <div>
                                        {section.rows.map((row) => (
                                          <article className="pattern-family-pattern-catalog-symbol" key={row.root_symbol}>
                                            <strong>{row.root_symbol}</strong>
                                            <span>{formatNumber(row.setup_count)}</span>
                                            <small>{formatNumber(row.contract_count)} contracts</small>
                                          </article>
                                        ))}
                                      </div>
                                    </div>
                                  ))}
                                </div>
                              ) : (
                                <div className="pattern-family-market-empty">No symbol data loaded for this scan profile.</div>
                              )}
                            </section>

                            <div className="pattern-family-pattern-catalog-side-stack">
                              <section className="pattern-family-pattern-catalog-section">
                                <header>
                                  <div>
                                    <span>Harmonic Types</span>
                                    <strong>{formatNumber(selectedPatternCatalogHarmonicRows.length)} types</strong>
                                  </div>
                                </header>
                                <div className="pattern-family-pattern-catalog-rank-list">
                                  {selectedPatternCatalogHarmonicRows.length ? (
                                    selectedPatternCatalogHarmonicRows.slice(0, 12).map((row) => (
                                      <div key={row.harmonic_type}>
                                        <span>{row.harmonic_type}</span>
                                        <strong>{formatNumber(row.setup_count)}</strong>
                                      </div>
                                    ))
                                  ) : (
                                    <small>No harmonic breakdown loaded.</small>
                                  )}
                                </div>
                              </section>
                              <section className="pattern-family-pattern-catalog-section">
                                <header>
                                  <div>
                                    <span>Pattern Market</span>
                                    <strong>{formatNumber(selectedPatternCatalogMarketRows.length)} sides</strong>
                                  </div>
                                </header>
                                <div className="pattern-family-pattern-catalog-rank-list">
                                  {selectedPatternCatalogMarketRows.length ? (
                                    selectedPatternCatalogMarketRows.map((row) => (
                                      <div key={row.market}>
                                        <span>{row.market}</span>
                                        <strong>{formatNumber(row.setup_count)}</strong>
                                      </div>
                                    ))
                                  ) : (
                                    <small>No market breakdown loaded.</small>
                                  )}
                                </div>
                              </section>
                            </div>
                          </section>
                        </>
                      ) : (
                        <div className="pattern-family-market-empty">Select a scan profile to drill in.</div>
                      )}
                    </main>
                  </div>
                )}
              </div>
            </section>
          ) : null}
        </CanvasCollapseWrapper>

        <aside
          className={[
            'pattern-family-inspector',
            isInspectorCollapsed ? 'pattern-family-inspector--collapsed' : '',
          ].filter(Boolean).join(' ')}
        >
          <div className="pattern-family-inspector-collapse-rail">
            <button
              aria-label={isInspectorCollapsed ? 'Expand canvas section' : 'Collapse canvas section'}
              onClick={() => setInspectorCollapsed((current) => !current)}
              type="button"
            >
              <span
                aria-hidden="true"
                className={[
                  'pattern-family-selected-collapse-arrow',
                  isInspectorCollapsed
                    ? 'pattern-family-inspector-collapse-arrow--left'
                    : 'pattern-family-inspector-collapse-arrow--right',
                ].join(' ')}
              />
            </button>
          </div>

          <div className="pattern-family-inspector-content">
          <header className="pattern-family-inspector-head">
            <div className="pattern-family-inspector-title">
              <span>Trade Inspector</span>
            </div>

            <div
              className={[
                'pattern-family-inspector-trade-chip',
                selectedTradeTone ? `pattern-family-inspector-trade-chip--${selectedTradeTone}` : '',
              ].filter(Boolean).join(' ')}
            >
              <span>{selectedTradeSummaryLabel}</span>
              <strong>{selectedTradeSummaryR}</strong>
            </div>

            <div className="pattern-family-inspector-hover-strip" aria-label="Hovered candle">
              {inspectorHoveredCandleStats.map((item) => (
                <div
                  className={[
                    'pattern-family-inspector-hover-cell',
                    item.wide ? 'pattern-family-inspector-hover-cell--wide' : '',
                  ].filter(Boolean).join(' ')}
                  key={item.label}
                >
                  <span>{item.label}</span>
                  <strong style={item.label !== 'Candle' ? { color: inspectorHoveredCandle?.color } : undefined}>
                    {item.value}
                  </strong>
                </div>
              ))}
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
            <section
              className={[
                'pattern-family-trade-ticket',
                showRouteLogicPanel && !isRouteLogicCollapsed ? 'pattern-family-trade-ticket--logic-open' : '',
                showRouteLogicPanel && isRouteLogicCollapsed ? 'pattern-family-trade-ticket--logic-collapsed' : '',
              ].filter(Boolean).join(' ')}
            >
              <div className="pattern-family-trade-ticket-head">
                <div className="pattern-family-trade-ticket-title">
                  <span>{inspectorDetailTitle}</span>
                  {inspectorDetailHeading ? <strong>{inspectorDetailHeading}</strong> : null}
                  {inspectorDetailSubheading ? <small>{inspectorDetailSubheading}</small> : null}
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
                    className={inspectorDetailMode === 'pattern' ? 'pattern-family-ticket-toggle-button pattern-family-ticket-toggle-button--active' : 'pattern-family-ticket-toggle-button'}
                    onClick={() => setInspectorDetailMode('pattern')}
                    type="button"
                  >
                    Pattern
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
                {inspectorDetailStats.map((item) => (
                  <button
                    className={[
                      'pattern-family-trade-ticket-cell',
                      item.wide ? 'pattern-family-trade-ticket-cell--wide' : '',
                      item.code ? 'pattern-family-trade-ticket-cell--code' : '',
                      item.tone ? `pattern-family-trade-ticket-cell--${item.tone}` : '',
                      item.onClick ? 'pattern-family-trade-ticket-cell--button' : '',
                      item.copyLabel ? 'pattern-family-trade-ticket-cell--copy' : '',
                    ].filter(Boolean).join(' ')}
                    aria-label={item.copyLabel ? `Copy ${item.label}` : undefined}
                    key={item.label}
                    onClick={item.onClick}
                    type="button"
                    disabled={!item.onClick}
                  >
                    <span>{item.label}</span>
                    <strong title={item.title ?? item.value}>
                      <span>{item.value}</span>
                      {item.copyLabel ? <small>{item.copyLabel}</small> : null}
                    </strong>
                  </button>
                ))}
              </div>

              {showRouteLogicPanel ? (
                <div
                  className={[
                    'pattern-family-route-logic-panel',
                    isRouteLogicCollapsed ? 'pattern-family-route-logic-panel--collapsed' : '',
                  ].filter(Boolean).join(' ')}
                >
                  <div className="pattern-family-route-logic-title">
                    <span className="pattern-family-section-bar-title">
                      {selectedTradeTemplate ? 'Template Logic' : 'Route Logic'}
                    </span>
                    <span className="pattern-family-section-bar-line" aria-hidden="true" />
                    <strong
                      className="pattern-family-section-bar-context"
                      title={
                        selectedTradeTemplate
                          ? `${selectedTradeTemplate.template_label || 'Template'} | ${selectedTradeTemplate.template_uid}`
                          : `${selectedRoute.route_label} | ${selectedRoute.route_id}`
                      }
                    >
                      {selectedTradeTemplate
                        ? selectedTradeTemplate.template_label || compactText(selectedTradeTemplate.template_uid, 12)
                        : selectedRoute.route_label}
                    </strong>
                    <button
                      aria-label={isRouteLogicCollapsed ? 'Show Route Logic' : 'Hide Route Logic'}
                      aria-expanded={!isRouteLogicCollapsed}
                      onClick={() => {
                        setRouteLogicCollapsed((current) => !current);
                        setRouteLogicHover(null);
                      }}
                      title={isRouteLogicCollapsed ? 'Show Route Logic' : 'Hide Route Logic'}
                      type="button"
                    >
                      <span
                        className={
                          isRouteLogicCollapsed
                            ? 'pattern-family-selected-collapse-arrow pattern-family-selected-collapse-arrow--down'
                            : 'pattern-family-selected-collapse-arrow pattern-family-selected-collapse-arrow--up'
                        }
                      />
                    </button>
                  </div>
                  {!isRouteLogicCollapsed ? (
                    <div className="pattern-family-route-logic-steps">
                      {selectedLogicOverlayDetails.map((item, index) => (
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
                  ) : null}
                </div>
              ) : null}
            </section>

            <section className="pattern-family-chart-bay">
              <div className="pattern-family-chart-stage">
                {canvasChartData.candles.length && canvasChartData.rust_patterns ? (
                  <div
                    className={[
                      'pattern-family-live-canvas-layout',
                      isRawCanvasView ? 'pattern-family-live-canvas-layout--raw' : '',
                    ].filter(Boolean).join(' ')}
                  >
                    {isRawCanvasView ? (
                      <aside className="pattern-family-live-trends-panel">
                        <header className="pattern-family-live-trends-topbar">
                          <div className="pattern-family-live-trends-identity">
                            <span>{livePanelModeTitle}</span>
                            <strong>
                              {liveTrendPanelMode === 'trades'
                                ? LIVE_CANVAS_SIGNAL_INSTRUMENT
                                : liveTrendPanelMode === 'oracle' || liveTrendPanelMode === 'scanner'
                                  ? LIVE_CANVAS_ROOT
                                  : liveCanvasSymbol} {LIVE_CANVAS_TIMEFRAME}
                            </strong>
                          </div>
                          <div className="pattern-family-live-trends-headline">
                            <span>{liveTrendPanelMode === 'trades' ? 'Trade Count' : 'Rows'}</span>
                            <strong>{livePanelShownLabel}</strong>
                          </div>
                        </header>
                        <div className="pattern-family-live-trends-dashboard">
                          <section className="pattern-family-live-status-card pattern-family-live-status-card--run">
                            <span>Run</span>
                            <strong title={livePanelRunId ?? undefined}>
                              {livePanelRunId ? compactText(livePanelRunId, 22) : 'No run'}
                            </strong>
                          </section>
                          <section
                            className={[
                              'pattern-family-live-status-card',
                              'pattern-family-live-status-card--feed',
                              liveCanvasRefreshMeta.error ? 'pattern-family-live-status-card--loss' : 'pattern-family-live-status-card--win',
                            ].filter(Boolean).join(' ')}
                          >
                            <span>Feed</span>
                            <strong>{liveFeedLabel}</strong>
                          </section>
                          <section className="pattern-family-live-status-card">
                            <span>Candle</span>
                            <strong>{formatLiveCanvasNtTime(liveCanvasRefreshMeta.latestCandleAt)}</strong>
                          </section>
                          <section className="pattern-family-live-status-card pattern-family-live-status-card--price">
                            <span>Price</span>
                            <strong>{formatNullableDecimal(liveCanvasRefreshMeta.latestSnapshotPrice, 4)}</strong>
                          </section>
                          {liveTrendPanelMode === 'trades' ? (
                            <>
                              <section className="pattern-family-live-status-card">
                                <span>Record</span>
                                <strong>
                                {formatNumber(liveTradeSignals.summary?.wins ?? 0)} / {formatNumber(liveTradeSignals.summary?.losses ?? 0)}
                                </strong>
                              </section>
                              <section
                                className={[
                                  'pattern-family-live-status-card',
                                  `pattern-family-live-status-card--${liveTradePnlTone}`,
                                ].join(' ')}
                              >
                                <span>P/L</span>
                                <strong>{formatSignedLiveMoney(liveTradeSignals.summary?.total_accounting_pnl)}</strong>
                              </section>
                            </>
                          ) : null}
                          {liveTrendPanelMode === 'model' ? (
                            <section className="pattern-family-live-status-card">
                              <span>Skipped</span>
                              <button
                                type="button"
                                className={[
                                  'pattern-family-live-trends-toggle',
                                  showSkippedLiveTrends ? 'pattern-family-live-trends-toggle--active' : '',
                                ].filter(Boolean).join(' ')}
                                onClick={() => setShowSkippedLiveTrends((current) => !current)}
                              >
                                {showSkippedLiveTrends
                                  ? 'Hide skipped'
                                  : `Show skipped ${hiddenSkippedLiveTrendCount ? `(${formatNumber(hiddenSkippedLiveTrendCount)})` : ''}`}
                              </button>
                            </section>
                          ) : null}
                        </div>
                        <div className="pattern-family-live-trends-table" role="table" aria-label="Detected trends">
                          <div
                            className={[
                              'pattern-family-live-trends-row',
                              'pattern-family-live-trends-row--head',
                              liveTrendPanelMode === 'trades' ? 'pattern-family-live-trends-row--trade' : '',
                              liveTrendPanelMode === 'scanner' ? 'pattern-family-live-trends-row--scanner' : '',
                              liveTrendPanelMode === 'oracle' ? 'pattern-family-live-trends-row--oracle' : '',
                            ].filter(Boolean).join(' ')}
                            role="row"
                          >
                            {liveTrendPanelMode === 'trades' ? (
                              <>
                                <span>Time</span>
                                <span>Side</span>
                                <span>S2</span>
                                <span>Status</span>
                                <span>Win</span>
                                <span>MHO P/L</span>
                                <span>Ticks</span>
                              </>
                            ) : liveTrendPanelMode === 'scanner' ? (
                              <>
                                <span>Time</span>
                                <span>Event</span>
                                <span>L1</span>
                                <span>L2</span>
                                <span>S2</span>
                                <span>State</span>
                              </>
                            ) : liveTrendPanelMode === 'oracle' ? (
                              <>
                                <span>Entry</span>
                                <span>Sym</span>
                                <span>Dir</span>
                                <span>Exit</span>
                                <span>Bars</span>
                                <span>R</span>
                              </>
                            ) : (
                              <>
                                <span>Time</span>
                                <span>Dir</span>
                                <span>Score</span>
                                <span>Entry</span>
                                <span>State</span>
                                <span>R</span>
                              </>
                            )}
                          </div>
                          {liveTrendPanelMode === 'trades' ? (
                            liveTradeSignals.rows.length ? (
                              liveTradeSignals.rows.map((trade) => {
                                const direction = String(trade.side || '').toUpperCase();
                                const tradeKey = getLiveTradeSignalKey(trade);
                                const resultLabel = getLiveTradeSignalResultLabel(trade);
                                const resultTone = getLiveTradeSignalResultTone(trade);
                                const status = String(trade.status || 'N/A');
                                const stage2Score = getLiveTradeStage2Score(trade);

                                return (
                                  <button
                                    type="button"
                                    className={[
                                      'pattern-family-live-trends-row',
                                      'pattern-family-live-trends-row--trade',
                                      selectedLiveTrendKey === tradeKey ? 'pattern-family-live-trends-row--selected' : '',
                                      direction === 'LONG' ? 'pattern-family-live-trends-row--long' : '',
                                      direction === 'SHORT' ? 'pattern-family-live-trends-row--short' : '',
                                    ].filter(Boolean).join(' ')}
                                    key={tradeKey}
                                    onClick={() => handleLiveTradeSignalSelect(trade)}
                                    role="row"
                                    title={`${trade.signal_uid || 'Signal'} | ${formatShortDateTime(trade.triggered_at || trade.expected_time || trade.created_at)} | ${direction || 'N/A'} | S2 ${formatScorePercent(stage2Score)} | ${status} | ${resultLabel} | MHO ${formatSignedLiveMoney(trade.realized_accounting_dollars)} | ${formatNullableDecimal(trade.realized_ticks, 1)} ticks`}
                                  >
                                    <span>{formatShortDateTime(trade.triggered_at || trade.expected_time || trade.created_at)}</span>
                                    <strong>{direction || 'N/A'}</strong>
                                    <span className="pattern-family-live-trends-score">
                                      {formatScorePercent(stage2Score)}
                                    </span>
                                    <span
                                      className={[
                                        'pattern-family-live-trends-state',
                                        status.toLowerCase() === 'completed' ? 'pattern-family-live-trends-state--closed' : '',
                                        status.toLowerCase() === 'triggered' ? 'pattern-family-live-trends-state--open' : '',
                                      ].filter(Boolean).join(' ')}
                                    >
                                      {formatRouteMode(status)}
                                    </span>
                                    <em
                                      className={[
                                        'pattern-family-live-trends-entry',
                                        resultTone === 'win' ? 'pattern-family-live-trends-entry--accepted' : '',
                                        resultTone === 'loss' ? 'pattern-family-live-trends-entry--rejected_risk' : '',
                                      ].filter(Boolean).join(' ')}
                                    >
                                      {resultLabel}
                                    </em>
                                    <span
                                      className={[
                                        'pattern-family-live-trends-result',
                                        resultTone === 'win' ? 'pattern-family-live-trends-result--win' : '',
                                        resultTone === 'loss' ? 'pattern-family-live-trends-result--loss' : '',
                                      ].filter(Boolean).join(' ')}
                                    >
                                      {formatSignedLiveMoney(trade.realized_accounting_dollars)}
                                    </span>
                                    <span className="pattern-family-live-trends-result">
                                      {formatNullableDecimal(trade.realized_ticks, 1)}
                                    </span>
                                  </button>
                                );
                              })
                            ) : (
                              <div className="pattern-family-live-trends-empty">
                                {liveTradeSignals.error || 'No live trades taken for this run yet.'}
                              </div>
                            )
                          ) : liveTrendPanelMode === 'scanner' ? (
                            liveScannerActivity.rows.length ? (
                              liveScannerActivity.rows.map((row) => {
                                const details = row.details ?? {};
                                const eventLabel = getScannerActivityLabel(row);
                                const eventTone = getScannerActivityTone(row);
                                const stateLabel = getScannerActivityState(row);
                                const eventType = String(row.event_type || '').toLowerCase();
                                const timeValue = row.candle_time || row.ts_utc || row.created_at;
                                const l1Value = eventType === 'cycle_scored'
                                  ? formatScannerCount(details.stage1_rows)
                                  : formatScannerScore(row.level2_score);
                                const l2Value = eventType === 'cycle_scored'
                                  ? formatScannerCount(details.level2_picks)
                                  : formatScannerScore(row.level2_score);
                                const s2Value = eventType === 'cycle_scored'
                                  ? formatScannerCount(details.stage2_rows)
                                  : formatScannerScore(row.stage2_score);

                                return (
                                  <div
                                    className={[
                                      'pattern-family-live-trends-row',
                                      'pattern-family-live-trends-row--scanner',
                                      eventType === 'trend_confirmed' ? 'pattern-family-live-trends-row--scanner-signal' : '',
                                    ].filter(Boolean).join(' ')}
                                    key={row.event_uid || row.id}
                                    role="row"
                                    title={`${eventLabel} | ${formatShortDateTime(timeValue)} | ${formatRouteMode(row.status || 'seen')} | ${details.read || row.event_uid || ''}`}
                                  >
                                    <span>{formatShortDateTime(timeValue)}</span>
                                    <strong>{eventLabel}</strong>
                                    <span>{l1Value}</span>
                                    <span>{l2Value}</span>
                                    <span>{s2Value}</span>
                                    <em
                                      className={[
                                        'pattern-family-live-trends-entry',
                                        eventTone === 'win' ? 'pattern-family-live-trends-entry--accepted' : '',
                                        eventTone === 'loss' ? 'pattern-family-live-trends-entry--rejected_risk' : '',
                                        eventTone === 'open' ? 'pattern-family-live-trends-state--open' : '',
                                      ].filter(Boolean).join(' ')}
                                    >
                                      {stateLabel}
                                    </em>
                                  </div>
                                );
                              })
                            ) : (
                              <div className="pattern-family-live-trends-empty">
                                {liveScannerActivity.error || 'No scanner activity stored yet. Restart the live monitor after this update to log every candle check.'}
                              </div>
                            )
                          ) : liveTrendPanelMode === 'oracle' ? (
                            visibleLiveOracleTrendRows.length ? (
                              visibleLiveOracleTrendRows.map((trend) => {
                                const direction = String(trend.direction || '').toUpperCase();
                                const trendKey = getOracleTrendKey(trend);

                                return (
                                  <button
                                    type="button"
                                    className={[
                                      'pattern-family-live-trends-row',
                                      'pattern-family-live-trends-row--oracle',
                                      selectedLiveTrendKey === trendKey ? 'pattern-family-live-trends-row--selected' : '',
                                      direction === 'LONG' ? 'pattern-family-live-trends-row--long' : '',
                                      direction === 'SHORT' ? 'pattern-family-live-trends-row--short' : '',
                                    ].filter(Boolean).join(' ')}
                                    key={trendKey}
                                    onClick={() => handleLiveOracleTrendSelect(trend)}
                                    role="row"
                                    title={`${trend.symbol || LIVE_CANVAS_ROOT} | ${formatShortDateTime(trend.entry_date)} to ${formatShortDateTime(trend.exit_date)} | ${direction || 'N/A'} | Q ${formatDecimal(trend.quality_score, 1)} | Result ${formatSignedR(trend.result_r)}`}
                                  >
                                    <span>{formatShortDateTime(trend.entry_date)}</span>
                                    <span>{trend.symbol || LIVE_CANVAS_ROOT}</span>
                                    <strong>{direction || 'N/A'}</strong>
                                    <span>{formatShortDateTime(trend.exit_date)}</span>
                                    <span>{formatNumber(trend.duration_bars)}</span>
                                    <span
                                      className={[
                                        'pattern-family-live-trends-result',
                                        Number.isFinite(Number(trend.result_r)) && Number(trend.result_r) > 0 ? 'pattern-family-live-trends-result--win' : '',
                                        Number.isFinite(Number(trend.result_r)) && Number(trend.result_r) <= 0 ? 'pattern-family-live-trends-result--loss' : '',
                                      ].filter(Boolean).join(' ')}
                                    >
                                      {formatSignedR(trend.result_r, 1)}
                                    </span>
                                  </button>
                                );
                              })
                            ) : (
                              <div className="pattern-family-live-trends-empty">
                                {liveOracleTrendEvents.error || 'No oracle trends loaded for this run.'}
                              </div>
                            )
                          ) : visibleLiveTrendRows.length ? (
                            visibleLiveTrendRows.map((trend) => {
                              const direction = String(trend.direction || '').toUpperCase();
                              const entryStatus = String(trend.paper_entry_status || 'confirmed').toLowerCase();
                              const exitStatus = String(trend.paper_exit_status || '').toLowerCase();
                              const trendKey = getTrendEventKey(trend);
                              const resultR = Number(trend.paper_result_r);
                              const entryLabel =
                                entryStatus === 'accepted'
                                  ? 'OK'
                                  : entryStatus === 'rejected_risk'
                                    ? 'Risk'
                                    : 'Watch';
                              const tradeState =
                                exitStatus === 'closed'
                                  ? 'Closed'
                                  : entryStatus === 'accepted'
                                    ? 'Open'
                                    : 'Skipped';

                              return (
                                <button
                                  type="button"
                                  className={[
                                    'pattern-family-live-trends-row',
                                    selectedLiveTrendKey === trendKey ? 'pattern-family-live-trends-row--selected' : '',
                                    direction === 'LONG' ? 'pattern-family-live-trends-row--long' : '',
                                    direction === 'SHORT' ? 'pattern-family-live-trends-row--short' : '',
                                  ].filter(Boolean).join(' ')}
                                  key={trendKey}
                                  onClick={() => handleLiveTrendSelect(trend)}
                                  role="row"
                                  title={`${trend.model_symbol || LIVE_CANVAS_ROOT} | ${formatShortDateTime(trend.candle_time)} | ${direction || 'N/A'} | Stage 2 ${formatScorePercent(trend.stage2_score)} | Entry ${formatMoney(trend.paper_entry_price ?? trend.entry_price)} | ${tradeState} | Result ${formatSignedR(trend.paper_result_r)}`}
                                >
                                  <span>{formatShortDateTime(trend.candle_time)}</span>
                                  <strong>{direction || 'N/A'}</strong>
                                  <span>{formatScorePercent(trend.stage2_score)}</span>
                                  <em className={`pattern-family-live-trends-entry pattern-family-live-trends-entry--${entryStatus}`}>
                                    {entryLabel}
                                  </em>
                                  <span
                                    className={[
                                      'pattern-family-live-trends-state',
                                      tradeState === 'Closed' ? 'pattern-family-live-trends-state--closed' : '',
                                      tradeState === 'Open' ? 'pattern-family-live-trends-state--open' : '',
                                      tradeState === 'Skipped' ? 'pattern-family-live-trends-state--skipped' : '',
                                    ].filter(Boolean).join(' ')}
                                  >
                                    {tradeState}
                                  </span>
                                  <span
                                    className={[
                                      'pattern-family-live-trends-result',
                                      Number.isFinite(resultR) && resultR > 0 ? 'pattern-family-live-trends-result--win' : '',
                                      Number.isFinite(resultR) && resultR <= 0 ? 'pattern-family-live-trends-result--loss' : '',
                                    ].filter(Boolean).join(' ')}
                                  >
                                    {formatSignedR(trend.paper_result_r, 1)}
                                  </span>
                                </button>
                              );
                            })
                          ) : (
                            <div className="pattern-family-live-trends-empty">
                              {liveTrendEvents.error || (showSkippedLiveTrends ? 'No confirmed trends loaded.' : 'No accepted trends loaded.')}
                            </div>
                          )}
                        </div>
                      </aside>
                    ) : null}
                    <div className="pattern-family-live-chart-area">
                      <div className="pattern-family-full-chart pattern-family-inspector-chart">
                        <CandleChartPanel
                          chartData={canvasChartData}
                          isSectionsExpanded={isCanvasExpanded}
                          setSectionsExpanded={setCanvasExpanded}
                          focusMode={isRawCanvasView ? 'pattern' : 'prop'}
                          market={canvasChartData.rust_patterns.market ?? selectedFamily?.market ?? 'Bullish'}
                          overlayTopOffset={0}
                          showCandles={showCanvasCandles}
                          presentationMode={isRawCanvasView ? 'chart' : 'graph'}
                          routeLogicHover={routeLogicHover}
                          onHoveredCandleChange={setInspectorHoveredCandle}
                          onRawCandleViewportEdge={handleRawCandleViewportEdge}
                        />
                      </div>
                      {isCanvasLoading ? (
                        <div className="pattern-family-chart-loading-badge">Updating candles...</div>
                      ) : null}
                      {isRawCanvasView ? (
                        <div className="pattern-family-chart-hover-status" aria-label="Hovered candle OHLCV">
                          {inspectorHoveredCandleStats.map((item) => (
                            <div
                              className={[
                                'pattern-family-chart-hover-status__cell',
                                item.wide ? 'pattern-family-chart-hover-status__cell--wide' : '',
                              ].filter(Boolean).join(' ')}
                              key={item.label}
                            >
                              <span>{item.label}</span>
                              <strong style={item.label !== 'Candle' ? { color: inspectorHoveredCandle?.color } : undefined}>
                                {item.value}
                              </strong>
                            </div>
                          ))}
                        </div>
                      ) : null}
                      {canvasError ? <div className="pattern-family-chart-error-badge">{canvasError}</div> : null}
                    </div>
                  </div>
                ) : isCanvasLoading ? (
                  <div className="pattern-family-inspector-empty">Loading family canvas...</div>
                ) : canvasError ? (
                  <div className="pattern-family-inspector-error">{canvasError}</div>
                ) : canvasPattern ? (
                  <div className="pattern-family-inspector-empty">
                    Canvas is waiting for candle data so the XABCD lines can use the chart scale.
                  </div>
                ) : (
                  <div className="pattern-family-inspector-empty">Select a family to preview its canvas.</div>
                )}
              </div>
            </section>
          </div>
          </div>
        </aside>
      </div>

      {expandedSimulationTable === 'symbolContribution' ? (
        <div
          className="pattern-family-simulation-table-overlay"
          role="presentation"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) {
              setExpandedSimulationTable(null);
            }
          }}
        >
          <section
            className="pattern-family-simulation-table-dialog"
            role="dialog"
            aria-modal="true"
            aria-label="Symbol Contribution"
          >
            <header>
              <div>
                <span>Simulation Table</span>
                <strong>Symbol Contribution</strong>
              </div>
              <button type="button" onClick={() => setExpandedSimulationTable(null)}>
                Close
              </button>
            </header>
            <div className="pattern-family-simulation-table-dialog-body pattern-family-selected-simulation-contribution-table">
              {renderSymbolContributionTable()}
            </div>
          </section>
        </div>
      ) : null}

      {expandedSimulationTable === 'familyContribution' ? (
        <div
          className="pattern-family-simulation-table-overlay"
          role="presentation"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) {
              setExpandedSimulationTable(null);
            }
          }}
        >
          <section
            className="pattern-family-simulation-table-dialog"
            role="dialog"
            aria-modal="true"
            aria-label="Family Contribution"
          >
            <header>
              <div>
                <span>Simulation Table</span>
                <strong>Family Contribution</strong>
              </div>
              <button type="button" onClick={() => setExpandedSimulationTable(null)}>
                Close
              </button>
            </header>
            <div className="pattern-family-simulation-table-dialog-body pattern-family-selected-simulation-contribution-table">
              {renderFamilyContributionTable(selectedSimulationFamilyContributionRows, { compactFamily: false })}
            </div>
          </section>
        </div>
      ) : null}

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
                  <section className="pattern-family-controls pattern-family-browse-filters">
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
                      <span>Timeframe</span>
                      <select value={timeframeFilter} onChange={(event) => setTimeframeFilter(event.target.value)}>
                        {TIMEFRAME_OPTIONS.map((option) => (
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
                  <section className="pattern-family-controls pattern-family-browse-filters pattern-family-browse-filters--patterns">
                    <label>
                      <span>Family Scope</span>
                      <select value={patternBrowseScope} onChange={(event) => setPatternBrowseScope(event.target.value)}>
                        <option value="selected">Selected Family</option>
                        <option value="all">All Families</option>
                      </select>
                    </label>
                    <label>
                      <span>Symbol</span>
                      <select
                        disabled={isPatternBrowseLoading}
                        value={patternBrowseSymbol}
                        onChange={(event) => setPatternBrowseSymbol(event.target.value)}
                      >
                        {patternBrowseSymbolOptions.map((option) => (
                          <option key={option} value={option}>
                            {option}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="pattern-family-search-field">
                      <span>Search</span>
                      <input
                        value={patternBrowseSearch}
                        onChange={(event) => setPatternBrowseSearch(event.target.value)}
                        placeholder="pattern, family, market..."
                      />
                    </label>
                    <div className="pattern-family-browse-actions">
                      <button
                        disabled={!canApplyPatternNavigation}
                        onClick={applyPatternNavigationSet}
                        type="button"
                      >
                        Apply
                      </button>
                      <button
                        disabled={!appliedPatternNavigation && patternBrowseSymbol === 'All' && !patternBrowseSearch.trim()}
                        onClick={clearPatternNavigationSet}
                        type="button"
                      >
                        Clear
                      </button>
                    </div>
                    <div className="pattern-family-browse-filter-summary">
                      <span>Showing</span>
                      <strong>
                        {isPatternBrowseLoading
                          ? 'Loading...'
                          : patternBrowseSearch.trim()
                            ? `${formatNumber(visiblePatternBrowseRows.length)} / ${formatNumber(patternBrowseRows.length)} loaded`
                            : `${formatNumber(patternBrowseRows.length)} / ${formatNumber(patternBrowseMeta.totalCount || patternBrowseRows.length)}`}
                      </strong>
                      <small title={appliedPatternNavigation?.label ?? undefined}>
                        {appliedPatternNavigation
                          ? `Nav: ${appliedPatternNavigation.label}`
                          : patternBrowseScope === 'selected'
                            ? selectedFamily?.family_key ?? selectedFamilyKey ?? 'No selected family'
                            : patternBrowseMeta.hasMore
                              ? 'All families | first rows'
                              : 'All families'}
                      </small>
                    </div>
                  </section>
                  <div className="pattern-family-pattern-row pattern-family-pattern-row--head">
                    <span>Pattern</span>
                    <span>Twin</span>
                    <span>Family</span>
                    <span>Symbol</span>
                    <span>Market</span>
                    <span>Harmonic</span>
                    <span>D Date</span>
                    <span>Trade</span>
                  </div>
                  {patternBrowseError ? (
                    <div className="pattern-family-empty">{patternBrowseError}</div>
                  ) : isPatternBrowseLoading ? (
                    <div className="pattern-family-empty">Loading patterns...</div>
                  ) : visiblePatternBrowseRows.length ? (
                    visiblePatternBrowseRows.map((pattern) => {
                      const patternKey = getFamilyPatternKey(pattern);
                      const patternFamilyKey = getPatternFamilyKey(pattern);
                      const matchingTrade =
                        patternFamilyKey === selectedFamilyKey
                          ? routeTrades.find((trade) => patternMatchesTrade(pattern, trade))
                          : null;

                      return (
                        <button
                          className={`pattern-family-pattern-row pattern-family-pattern-row--button${
                            patternKey === selectedFamilyPatternKey ? ' pattern-family-pattern-row--selected' : ''
                          }`}
                          key={patternKey}
                          onClick={() => {
                            setAppliedPatternNavigation({
                              familyKey: patternBrowseScope === 'selected' ? selectedFamilyKey : null,
                              label: patternBrowseFilterLabel,
                              rows: visiblePatternBrowseRows,
                              scope: patternBrowseScope,
                              search: patternBrowseSearch.trim(),
                              symbol: patternBrowseSymbol,
                            });
                            if (patternFamilyKey && patternFamilyKey !== selectedFamilyKey) {
                              setSelectedFamilyKey(patternFamilyKey);
                              setSelectedRouteTradeKey(null);
                            }
                            setSelectedSimulationRawTradeKey(null);
                            setSelectedSimulationRawTrade(null);
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
                          <span title={pattern.event_id ?? ''}>
                            {Number(pattern.event_sister_count) > 1
                              ? `${pattern.event_rank ?? '-'} / ${pattern.event_sister_count}`
                              : 'Solo'}
                          </span>
                          <span className="pattern-family-id" title={patternFamilyKey ?? ''}>
                            {patternFamilyKey ?? 'N/A'}
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
                    <div className="pattern-family-empty">No patterns matched this browse filter.</div>
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
                            setSelectedSimulationRawTradeKey(null);
                            setSelectedSimulationRawTrade(null);
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
