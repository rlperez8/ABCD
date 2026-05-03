function hasTimeComponent(value) {
  return typeof value === 'string' && /[T\s]\d{2}:\d{2}/.test(value);
}

function normalizeDate(dateStr) {
  if (!dateStr) return null;

  if (/^\d{2}-\d{2}-\d{4}$/.test(dateStr)) {
    const [mm, dd, yyyy] = dateStr.split('-');
    return `${yyyy}-${mm}-${dd}`;
  }

  const parsed = new Date(dateStr);
  if (!Number.isNaN(parsed.getTime())) {
    return parsed.toISOString().split('T')[0];
  }

  return null;
}

function normalizeDateTimeKey(dateStr) {
  if (!dateStr) return null;

  const parsed = new Date(dateStr);
  if (Number.isNaN(parsed.getTime())) {
    return null;
  }

  return parsed.getTime();
}

function findIndexByDate(candles, patternDate) {
  if (!patternDate) return -1;

  if (hasTimeComponent(patternDate)) {
    const pivotTime = normalizeDateTimeKey(patternDate);
    if (pivotTime === null) return -1;

    const exactIndex = candles.findIndex((item) => {
      const candleTime = normalizeDateTimeKey(item.date || item.candle_date);
      return candleTime === pivotTime;
    });

    if (exactIndex >= 0) {
      return exactIndex + 1;
    }

    return -1;
  }

  const pivotDate = normalizeDate(patternDate);
  if (!pivotDate) return -1;

  return (
    candles.findIndex((item) => {
      const candleDate = normalizeDate(item.date || item.candle_date);
      return candleDate === pivotDate;
    }) + 1
  );
}

const buildFormattedPattern = (candles, rustPattern) => {
  const dConfirmDate = rustPattern?.d_confirm_date ?? null;
  const reversalDetectDate = rustPattern?.reversal_detect_date ?? null;
  const entryDate = rustPattern?.entry_date ?? null;
  const effectiveTargetDate = rustPattern?.target_date ?? null;
  const effectiveExitDate = rustPattern?.target_date ?? rustPattern?.trade_date ?? null;
  const indexX = findIndexByDate(candles, rustPattern?.x_date);
  const indexA = findIndexByDate(candles, rustPattern?.a_date);
  const indexB = findIndexByDate(candles, rustPattern?.b_date);
  const indexC = findIndexByDate(candles, rustPattern?.c_date);
  const indexD = findIndexByDate(candles, rustPattern?.d_date);
  const indexDConfirm = findIndexByDate(candles, dConfirmDate);
  const indexReversalDetect = findIndexByDate(candles, reversalDetectDate);
  const indexEntry = findIndexByDate(candles, entryDate);
  const indexTarget = findIndexByDate(candles, effectiveTargetDate);
  const resolvedDConfirm = indexDConfirm > 0 ? indexDConfirm : indexD > 1 ? indexD - 1 : -1;
  const resolvedReversalDetect =
    indexReversalDetect > 0 ? indexReversalDetect : -1;
  const resolvedEntry = indexEntry > 0 ? indexEntry : -1;
  const resolvedTarget = indexTarget > 0 ? indexTarget : -1;
  const exit = findIndexByDate(candles, effectiveExitDate);
  const isBearish = rustPattern?.market === 'Bearish';
  const exitPrice =
    rustPattern?.target_close ??
    rustPattern?.target_open ??
    rustPattern?.trade_current_price ??
    rustPattern?.trade_reward_exit_price;

  return {
    ...rustPattern,
    symbol: rustPattern.symbol,
    pattern_ABCD_bar_length: rustPattern.trade_length,
    x: indexX,
    a: indexA,
    b: indexB,
    c: indexC,
    d: indexD,
    d_confirm: resolvedDConfirm,
    reversal_detect: resolvedReversalDetect,
    entry: resolvedEntry,
    target: resolvedTarget,
    x_price: parseFloat(isBearish ? rustPattern.x_high : rustPattern.x_low),
    a_price: parseFloat(isBearish ? rustPattern.a_low : rustPattern.a_high),
    b_price: parseFloat(isBearish ? rustPattern.b_high : rustPattern.b_low),
    c_price: parseFloat(isBearish ? rustPattern.c_low : rustPattern.c_high),
    d_price: parseFloat(isBearish ? rustPattern.d_high : rustPattern.d_low),
    stop_loss: parseFloat(rustPattern.trade_risk_exit_price),
    take_profit: parseFloat(rustPattern.trade_reward_exit_price),
    entered_price: parseFloat(rustPattern.trade_enter_price),
    exit_price: parseFloat(exitPrice),
    exit_date: exit > 0 ? exit : -1,
  };
};

export const formatPattern = (candles, rustPattern, snrLines, setChartData) => {
  setChartData({
    candles,
    snr_lines: snrLines,
    rust_patterns: buildFormattedPattern(candles, rustPattern),
  });
};

export const buildPattern = (candles, rustPattern) => {
  return buildFormattedPattern(candles, rustPattern);
};

export const format_pattern = formatPattern;
export const format_pattern_ = buildPattern;
