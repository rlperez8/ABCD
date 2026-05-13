import React, { useEffect, useMemo, useState } from 'react';
import { fetchAllStrategyTrades, fetchStrategyCandidates } from '../../services/patternApi';

const ROOT_PREFIXES = [
  'MES',
  'MNQ',
  'MYM',
  'M2K',
  'MCL',
  'MGC',
  'SIL',
  'EMD',
  'RTY',
  'ES',
  'NQ',
  'YM',
  'CL',
  'QM',
  'NG',
  'GC',
  'SI',
  '6E',
  '6J',
  'ZN',
  'ZB',
  'ZF',
  'ZT',
  'ZC',
  'ZS',
  'ZM',
  'ZL',
  'ZW',
  'HE',
  'LE',
].sort((left, right) => right.length - left.length);

const formatPercent = (value) =>
  Number.isFinite(Number(value)) ? `${(Number(value) * 100).toFixed(1)}%` : 'N/A';

const formatSignedPoints = (value) =>
  Number.isFinite(Number(value)) ? `${Number(value) >= 0 ? '+' : ''}${Number(value).toFixed(1)} pts` : 'N/A';

const parseDate = (value) => {
  if (!value) {
    return null;
  }

  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
};

const parseNumber = (value) => {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
};

const getFamilyId = (family = {}) =>
  family?.propStrategyId ??
  family?.prop_strategy_id ??
  family?.familyKey ??
  family?.family_key ??
  family?.id ??
  '';

const getFamilyLabel = (family = {}) =>
  [
    getFamilyId(family),
    family.market,
    family.harmonicType ?? family.harmonic_type,
    family.bin,
    family.reversalType ?? family.reversal_type,
    family.sizeBucket ?? family.size_bucket,
    family.timeBin ?? family.time_bin,
  ]
    .filter(Boolean)
    .join(' / ');

const getRootSymbol = (symbol) => {
  const value = String(symbol ?? '').toUpperCase();
  const prefix = ROOT_PREFIXES.find((root) => value.startsWith(root));
  if (prefix) {
    return prefix;
  }

  const match = value.match(/^[A-Z]+/);
  return match?.[0]?.replace(/[FGHJKMNQUVXZ]\d*$/, '') || value || 'N/A';
};

const getTradeResult = (trade = {}) => Number.parseInt(String(trade.trade_result ?? 0), 10) || 0;

const getEntryDate = (trade = {}) =>
  parseDate(trade.entry_date ?? trade.reversal_detect_date ?? trade.d_confirm_date ?? trade.d_date);

const getEntryHour = (trade = {}) => {
  const date = getEntryDate(trade);
  return date ? date.getHours() : null;
};

const getHoldMinutes = (trade = {}) => {
  const entryDate = parseDate(trade.entry_date);
  const targetDate = parseDate(trade.target_date);
  if (!entryDate || !targetDate) {
    return null;
  }

  return (targetDate.getTime() - entryDate.getTime()) / 60000;
};

const getLengthBucket = (value) => {
  const parsed = parseNumber(value);
  if (!Number.isFinite(parsed)) {
    return null;
  }

  if (parsed <= 35) return '<=35';
  if (parsed <= 45) return '36-45';
  if (parsed <= 55) return '46-55';
  if (parsed <= 65) return '56-65';
  return '66+';
};

const getClosedTrades = (trades = []) =>
  trades.filter((trade) => [1, 2].includes(getTradeResult(trade)));

const summarizeTrades = (trades = []) => {
  const closedTrades = getClosedTrades(trades);
  const wins = closedTrades.filter((trade) => getTradeResult(trade) === 1).length;
  const losses = closedTrades.filter((trade) => getTradeResult(trade) === 2).length;
  const total = wins + losses;

  return {
    trades: total,
    wins,
    losses,
    winRate: total ? wins / total : 0,
    lossRate: total ? losses / total : 0,
  };
};

const buildRuleLabel = (kind, value) => {
  if (kind === 'root') return `Exclude ${value}`;
  if (kind === 'hour') return `Exclude hour ${value}`;
  if (kind === 'fullPatternLength') return `Exclude full length ${value}`;
  if (kind === 'dLength') return `Exclude D length ${value}`;
  if (kind === 'holdMinutes') return `Exclude hold ${value} min`;
  return `Exclude ${value}`;
};

const buildRuleExpression = (rule) => {
  if (rule.kind === 'root') return `root_symbol != '${rule.value}'`;
  if (rule.kind === 'hour') return `entry_hour != ${rule.value}`;
  if (rule.kind === 'fullPatternLength') return `full_pattern_length not in ${rule.value}`;
  if (rule.kind === 'dLength') return `d_length not in ${rule.value}`;
  if (rule.kind === 'holdMinutes') return `hold_minutes not in ${rule.value}`;
  return rule.label;
};

const buildLossRuleCandidates = (trades, baseLossRate) => {
  const closedTrades = getClosedTrades(trades);
  const minGroupSize = Math.max(8, Math.floor(closedTrades.length * 0.04));
  const groupDefinitions = [
    { kind: 'root', getValue: (trade) => getRootSymbol(trade.symbol) },
    { kind: 'hour', getValue: getEntryHour },
    { kind: 'fullPatternLength', getValue: (trade) => getLengthBucket(trade.full_pattern_length) },
    { kind: 'dLength', getValue: (trade) => getLengthBucket(trade.d_length) },
    { kind: 'holdMinutes', getValue: (trade) => getLengthBucket(getHoldMinutes(trade)) },
  ];
  const candidates = [];

  groupDefinitions.forEach((definition) => {
    const groups = new Map();
    closedTrades.forEach((trade) => {
      const value = definition.getValue(trade);
      if (value === null || value === undefined || value === 'N/A') {
        return;
      }

      if (!groups.has(value)) {
        groups.set(value, []);
      }
      groups.get(value).push(trade);
    });

    groups.forEach((groupTrades, value) => {
      const summary = summarizeTrades(groupTrades);
      const lossRate = summary.lossRate;

      if (
        summary.trades >= minGroupSize &&
        lossRate >= Math.max(baseLossRate + 0.08, 0.56)
      ) {
        const label = buildRuleLabel(definition.kind, value);
        candidates.push({
          kind: definition.kind,
          value,
          label,
          expression: buildRuleExpression({ kind: definition.kind, value, label }),
          groupTrades: summary.trades,
          groupLosses: summary.losses,
          groupLossRate: lossRate,
        });
      }
    });
  });

  return candidates.sort(
    (left, right) =>
      right.groupLossRate - left.groupLossRate || right.groupTrades - left.groupTrades
  );
};

const tradePassesRule = (trade, rule) => {
  if (rule.kind === 'root') return getRootSymbol(trade.symbol) !== rule.value;
  if (rule.kind === 'hour') return getEntryHour(trade) !== rule.value;
  if (rule.kind === 'fullPatternLength') return getLengthBucket(trade.full_pattern_length) !== rule.value;
  if (rule.kind === 'dLength') return getLengthBucket(trade.d_length) !== rule.value;
  if (rule.kind === 'holdMinutes') return getLengthBucket(getHoldMinutes(trade)) !== rule.value;
  return true;
};

const evaluateRules = (trades, rules = []) =>
  summarizeTrades(trades.filter((trade) => rules.every((rule) => tradePassesRule(trade, rule))));

const chooseRuleSet = (trades, candidates, baseWinRate) => {
  const originalCount = getClosedTrades(trades).length;
  const minRetainedTrades = Math.max(60, Math.floor(originalCount * 0.45));
  const selectedRules = [];
  let currentWinRate = baseWinRate;

  for (let pass = 0; pass < 3; pass += 1) {
    let best = null;
    const requiredWinRate = currentWinRate;

    candidates.forEach((candidate) => {
      if (
        selectedRules.some(
          (rule) => rule.kind === candidate.kind && rule.value === candidate.value
        )
      ) {
        return;
      }

      const nextSummary = evaluateRules(trades, [...selectedRules, candidate]);
      if (nextSummary.trades < minRetainedTrades) {
        return;
      }

      const incrementalLift = nextSummary.winRate - requiredWinRate;
      if (incrementalLift <= 0.005) {
        return;
      }

      const totalLift = nextSummary.winRate - baseWinRate;
      const retentionPenalty = ((originalCount - nextSummary.trades) / originalCount) * 0.05;
      const score = totalLift - retentionPenalty;

      if (!best || score > best.score) {
        best = { candidate, score, summary: nextSummary };
      }
    });

    if (!best) {
      break;
    }

    selectedRules.push(best.candidate);
    currentWinRate = best.summary.winRate;
  }

  return selectedRules;
};

const getPeriodLabel = (trades = []) => {
  const dates = trades
    .map(getEntryDate)
    .filter(Boolean)
    .sort((left, right) => left.getTime() - right.getTime());

  if (!dates.length) {
    return 'N/A';
  }

  const formatDate = (date) =>
    date.toLocaleDateString('en-US', {
      month: 'short',
      year: 'numeric',
    });

  return `${formatDate(dates[0])} to ${formatDate(dates[dates.length - 1])}`;
};

const splitWalkForwardTrades = (trades = []) => {
  const closedTrades = getClosedTrades(trades)
    .map((trade) => ({ trade, entryDate: getEntryDate(trade) }))
    .filter((item) => item.entryDate)
    .sort((left, right) => left.entryDate.getTime() - right.entryDate.getTime());

  if (closedTrades.length < 120) {
    return { trainTrades: [], testTrades: [] };
  }

  const splitIndex = Math.max(60, Math.floor(closedTrades.length * 0.7));
  return {
    trainTrades: closedTrades.slice(0, splitIndex).map((item) => item.trade),
    testTrades: closedTrades.slice(splitIndex).map((item) => item.trade),
  };
};

const getWalkForwardVerdict = (trainBase, trainFiltered, testBase, testFiltered) => {
  const trainLift = trainFiltered.winRate - trainBase.winRate;
  const testLift = testFiltered.winRate - testBase.winRate;

  if (testFiltered.trades < 30) {
    return {
      tone: 'thin',
      label: 'Thin Test',
      note: 'Not enough newer trades to trust the forward check yet.',
    };
  }

  if (trainLift > 0.03 && testLift > 0.02) {
    return {
      tone: 'passed',
      label: 'Promising',
      note: 'Rules improved both the discovery period and the later unseen period.',
    };
  }

  if (trainLift > 0.05 && testLift < -0.01) {
    return {
      tone: 'failed',
      label: 'Likely Overfit',
      note: 'Rules helped old data but hurt the later unseen period.',
    };
  }

  if (trainLift > 0.03 && testLift >= -0.01) {
    return {
      tone: 'mixed',
      label: 'Mixed',
      note: 'Rules improved discovery data, but the forward period did not confirm much edge.',
    };
  }

  return {
    tone: 'weak',
    label: 'Weak',
    note: 'The discovery period did not produce a strong enough lift.',
  };
};

const buildWalkForwardAnalysis = (trades) => {
  const { trainTrades, testTrades } = splitWalkForwardTrades(trades);

  if (!trainTrades.length || !testTrades.length) {
    return {
      isAvailable: false,
      verdict: {
        tone: 'thin',
        label: 'Not Enough Data',
        note: 'Walk-forward validation needs at least 120 closed trades.',
      },
      trainTrades,
      testTrades,
      trainBase: summarizeTrades(trainTrades),
      trainFiltered: summarizeTrades(trainTrades),
      testBase: summarizeTrades(testTrades),
      testFiltered: summarizeTrades(testTrades),
      rules: [],
      trainPeriod: getPeriodLabel(trainTrades),
      testPeriod: getPeriodLabel(testTrades),
    };
  }

  const trainBase = summarizeTrades(trainTrades);
  const candidates = buildLossRuleCandidates(trainTrades, trainBase.lossRate);
  const rules = chooseRuleSet(trainTrades, candidates, trainBase.winRate);
  const trainFiltered = evaluateRules(trainTrades, rules);
  const testBase = summarizeTrades(testTrades);
  const testFiltered = evaluateRules(testTrades, rules);

  return {
    isAvailable: true,
    verdict: getWalkForwardVerdict(trainBase, trainFiltered, testBase, testFiltered),
    trainTrades,
    testTrades,
    trainBase,
    trainFiltered,
    testBase,
    testFiltered,
    rules,
    trainPeriod: getPeriodLabel(trainTrades),
    testPeriod: getPeriodLabel(testTrades),
  };
};

const buildBreakdown = (trades, title, getValue) => {
  const groups = new Map();
  getClosedTrades(trades).forEach((trade) => {
    const value = getValue(trade);
    if (value === null || value === undefined) {
      return;
    }

    if (!groups.has(value)) {
      groups.set(value, []);
    }
    groups.get(value).push(trade);
  });

  return {
    title,
    rows: [...groups.entries()]
      .map(([value, groupTrades]) => ({
        value,
        ...summarizeTrades(groupTrades),
      }))
      .filter((row) => row.trades >= 5)
      .sort((left, right) => right.lossRate - left.lossRate || right.trades - left.trades)
      .slice(0, 8),
  };
};

const analyzeFamily = (family, trades) => {
  const base = summarizeTrades(trades);
  const candidates = buildLossRuleCandidates(trades, base.lossRate);
  const rules = chooseRuleSet(trades, candidates, base.winRate);
  const filtered = evaluateRules(trades, rules);

  return {
    family,
    familyId: getFamilyId(family),
    trades,
    base,
    candidates,
    rules,
    filtered,
    improvement: filtered.winRate - base.winRate,
    retainedRate: base.trades ? filtered.trades / base.trades : 0,
    walkForward: buildWalkForwardAnalysis(trades),
    breakdowns: [
      buildBreakdown(trades, 'Worst Roots', (trade) => getRootSymbol(trade.symbol)),
      buildBreakdown(trades, 'Worst Hours', getEntryHour),
      buildBreakdown(trades, 'Full Length', (trade) => getLengthBucket(trade.full_pattern_length)),
      buildBreakdown(trades, 'D Length', (trade) => getLengthBucket(trade.d_length)),
      buildBreakdown(trades, 'Hold Time', (trade) => getLengthBucket(getHoldMinutes(trade))),
    ],
  };
};

const FamilyRuleLabPage = ({ initialFamily = null, onOpenSimulatorFamily = null }) => {
  const [families, setFamilies] = useState([]);
  const [isLoadingFamilies, setLoadingFamilies] = useState(false);
  const [familyError, setFamilyError] = useState('');
  const [selectedFamilyId, setSelectedFamilyId] = useState(getFamilyId(initialFamily));
  const [manualFamilyId, setManualFamilyId] = useState(getFamilyId(initialFamily));
  const [analysis, setAnalysis] = useState(null);
  const [isAnalyzing, setAnalyzing] = useState(false);
  const [analysisError, setAnalysisError] = useState('');

  useEffect(() => {
    let isCancelled = false;

    const loadFamilies = async () => {
      try {
        setLoadingFamilies(true);
        setFamilyError('');
        const result = await fetchStrategyCandidates({
          minClosedTrades: 100,
          limit: 1000,
          includeCount: true,
          propMode: true,
          propOutcomeMode: 'reversal',
          sort: { key: 'score', direction: 'desc' },
        });

        if (!isCancelled) {
          setFamilies(result.strategies ?? []);
        }
      } catch (error) {
        console.error(error);
        if (!isCancelled) {
          setFamilyError('Could not load families.');
        }
      } finally {
        if (!isCancelled) {
          setLoadingFamilies(false);
        }
      }
    };

    void loadFamilies();

    return () => {
      isCancelled = true;
    };
  }, []);

  const selectedFamily = useMemo(
    () =>
      families.find((family) => getFamilyId(family) === selectedFamilyId) ??
      (selectedFamilyId
        ? {
            id: selectedFamilyId,
            propStrategyId: selectedFamilyId,
            prop_strategy_id: selectedFamilyId,
          }
        : null),
    [families, selectedFamilyId]
  );

  const runAnalysis = async (family) => {
    const familyId = getFamilyId(family);
    if (!familyId) {
      return;
    }

    try {
      setSelectedFamilyId(familyId);
      setManualFamilyId(familyId);
      setAnalyzing(true);
      setAnalysisError('');
      setAnalysis(null);

      const data = await fetchAllStrategyTrades(
        {
          ...family,
          id: familyId,
          propStrategyId: familyId,
          familyKey: familyId,
        },
        {
          propMode: true,
          propOutcomeMode: 'reversal',
        }
      );
      const trades = data.patterns ?? [];

      if (!trades.length) {
        setAnalysisError('No trades were returned for this family.');
        return;
      }

      setAnalysis(analyzeFamily(family, trades));
    } catch (error) {
      console.error(error);
      setAnalysisError('Family analysis failed. Check that the API server is running.');
    } finally {
      setAnalyzing(false);
    }
  };

  const runManualFamily = () => {
    const familyId = manualFamilyId.trim();
    if (!familyId) {
      return;
    }

    void runAnalysis({
      id: familyId,
      propStrategyId: familyId,
      prop_strategy_id: familyId,
    });
  };

  return (
    <div className="rule-lab-page">
      <aside className="rule-lab-sidebar">
        <div className="rule-lab-sidebar-head">
          <div>
            <span>Family Rule Lab</span>
            <strong>{families.length.toLocaleString()} families</strong>
          </div>
          <button
            type="button"
            className="rule-lab-run-button"
            onClick={() => selectedFamily && runAnalysis(selectedFamily)}
            disabled={!selectedFamily || isAnalyzing}
          >
            {isAnalyzing ? 'Running' : 'Run'}
          </button>
        </div>

        <label className="rule-lab-manual">
          <span>Family ID</span>
          <div>
            <input
              value={manualFamilyId}
              onChange={(event) => setManualFamilyId(event.target.value)}
              spellCheck="false"
            />
            <button type="button" onClick={runManualFamily} disabled={!manualFamilyId.trim() || isAnalyzing}>
              Run ID
            </button>
          </div>
        </label>

        {familyError ? <div className="rule-lab-error">{familyError}</div> : null}

        <div className="rule-lab-family-list">
          {families.map((family) => {
            const familyId = getFamilyId(family);
            const isSelected = familyId === selectedFamilyId;
            return (
              <button
                type="button"
                className={isSelected ? 'rule-lab-family-row rule-lab-family-row--selected' : 'rule-lab-family-row'}
                key={familyId}
                onClick={() => runAnalysis(family)}
              >
                <span>{familyId}</span>
                <strong>{formatPercent(family.win_rate)}</strong>
                <small>
                  {[family.market, family.harmonic_type, family.reversal_type, family.size_bucket]
                    .filter(Boolean)
                    .join(' / ')}
                </small>
              </button>
            );
          })}
        </div>
      </aside>

      <main className="rule-lab-workspace">
        {analysisError ? <div className="rule-lab-error">{analysisError}</div> : null}

        {!analysis ? (
          <div className="rule-lab-empty">
            <span>{isLoadingFamilies ? 'Loading families' : 'Select a family'}</span>
            <strong>{isAnalyzing ? 'Analyzing trades...' : 'Rule results will appear here'}</strong>
          </div>
        ) : (
          <>
            <section className="rule-lab-hero">
              <div>
                <span>Selected Family</span>
                <strong>{analysis.familyId}</strong>
                <small>{getFamilyLabel(analysis.family)}</small>
              </div>
              <button
                type="button"
                className="rule-lab-secondary-button"
                onClick={() => onOpenSimulatorFamily?.(analysis.familyId)}
              >
                Simulator
              </button>
            </section>

            <section className="rule-lab-score-grid">
              <div>
                <span>Base</span>
                <strong>{formatPercent(analysis.base.winRate)}</strong>
                <small>{analysis.base.trades.toLocaleString()} trades</small>
              </div>
              <div>
                <span>Filtered</span>
                <strong>{formatPercent(analysis.filtered.winRate)}</strong>
                <small>{analysis.filtered.trades.toLocaleString()} trades</small>
              </div>
              <div>
                <span>Lift</span>
                <strong>{formatSignedPoints(analysis.improvement * 100)}</strong>
                <small>{formatPercent(analysis.retainedRate)} retained</small>
              </div>
            </section>

            <section className="rule-lab-panel">
              <div className="rule-lab-panel-head">
                <span>Candidate Rule Set</span>
                <strong>{analysis.rules.length ? `${analysis.rules.length} rules` : 'No clear rule'}</strong>
              </div>
              {analysis.rules.length ? (
                <div className="rule-lab-rule-list">
                  {analysis.rules.map((rule) => (
                    <div className="rule-lab-rule-row" key={`${rule.kind}-${rule.value}`}>
                      <span>{rule.label}</span>
                      <code>{rule.expression}</code>
                      <small>
                        {rule.groupLosses}/{rule.groupTrades} losses, {formatPercent(rule.groupLossRate)} loss rate
                      </small>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="rule-lab-muted">No rule survived the sample-size guardrails.</div>
              )}
            </section>

            <section className={`rule-lab-panel rule-lab-walk-forward rule-lab-walk-forward--${analysis.walkForward.verdict.tone}`}>
              <div className="rule-lab-panel-head">
                <span>Walk-Forward Check</span>
                <strong>{analysis.walkForward.verdict.label}</strong>
              </div>
              <p className="rule-lab-walk-note">{analysis.walkForward.verdict.note}</p>

              <div className="rule-lab-walk-grid">
                <div>
                  <span>Train</span>
                  <strong>
                    {formatPercent(analysis.walkForward.trainBase.winRate)}
                    {' -> '}
                    {formatPercent(analysis.walkForward.trainFiltered.winRate)}
                  </strong>
                  <small>
                    {analysis.walkForward.trainPeriod} / {analysis.walkForward.trainFiltered.trades.toLocaleString()} kept
                  </small>
                </div>
                <div>
                  <span>Forward Test</span>
                  <strong>
                    {formatPercent(analysis.walkForward.testBase.winRate)}
                    {' -> '}
                    {formatPercent(analysis.walkForward.testFiltered.winRate)}
                  </strong>
                  <small>
                    {analysis.walkForward.testPeriod} / {analysis.walkForward.testFiltered.trades.toLocaleString()} kept
                  </small>
                </div>
                <div>
                  <span>Test Lift</span>
                  <strong>
                    {formatSignedPoints(
                      (analysis.walkForward.testFiltered.winRate -
                        analysis.walkForward.testBase.winRate) *
                        100
                    )}
                  </strong>
                  <small>Rules discovered on train only</small>
                </div>
              </div>

              {analysis.walkForward.rules.length ? (
                <div className="rule-lab-rule-list">
                  {analysis.walkForward.rules.map((rule) => (
                    <div className="rule-lab-rule-row" key={`walk-${rule.kind}-${rule.value}`}>
                      <span>{rule.label}</span>
                      <code>{rule.expression}</code>
                      <small>
                        Train evidence: {rule.groupLosses}/{rule.groupTrades} losses,{' '}
                        {formatPercent(rule.groupLossRate)} loss rate
                      </small>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="rule-lab-muted">No train-only rule set was selected.</div>
              )}
            </section>

            <section className="rule-lab-breakdown-grid">
              {analysis.breakdowns.map((breakdown) => (
                <div className="rule-lab-panel" key={breakdown.title}>
                  <div className="rule-lab-panel-head">
                    <span>{breakdown.title}</span>
                    <strong>Loss Rate</strong>
                  </div>
                  <div className="rule-lab-table">
                    {breakdown.rows.map((row) => (
                      <div className="rule-lab-table-row" key={`${breakdown.title}-${row.value}`}>
                        <span>{row.value}</span>
                        <strong>{formatPercent(row.lossRate)}</strong>
                        <small>
                          {row.losses}/{row.trades} losses
                        </small>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </section>
          </>
        )}
      </main>
    </div>
  );
};

export default FamilyRuleLabPage;
