import React, { useEffect, useMemo, useState } from 'react';
import {
  fetchPatternDiscoveryFamily,
  fetchStrategyCandidates,
  savePatternDiscoveryLogic,
} from '../../services/patternApi';

const formatPercent = (value) =>
  Number.isFinite(Number(value)) ? `${(Number(value) * 100).toFixed(1)}%` : 'N/A';

const formatR = (value) =>
  Number.isFinite(Number(value)) ? `${Number(value) >= 0 ? '+' : ''}${Number(value).toFixed(2)}R` : 'N/A';

const formatNumber = (value) =>
  Number.isFinite(Number(value)) ? Number(value).toLocaleString() : '0';

const formatDate = (value) => {
  if (!value) return 'N/A';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return String(value).slice(0, 10);
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
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
    family.family_name ?? family.familyName,
    family.market,
    family.harmonic_type ?? family.harmonicType,
    family.bin,
    family.reversal_type ?? family.reversalType,
    family.size_bucket ?? family.sizeBucket,
    family.time_bin ?? family.timeBin,
  ]
    .filter(Boolean)
    .join(' / ');

const getDiscoveryFamilyLabel = (family = {}) =>
  [
    family.family_name,
    family.outcome_model,
    family.market,
    family.harmonic_type,
    family.bin,
    family.reversal_type,
    family.size_bucket,
    family.time_bin,
    family.x_strictness,
  ]
    .filter(Boolean)
    .join(' / ');

const getAllBreakdownRows = (breakdowns = []) =>
  breakdowns.flatMap((breakdown) =>
    (breakdown.rows ?? []).map((row) => ({
      ...row,
      title: breakdown.title,
    }))
  );

const buildCandidateLogic = (analysis) => {
  if (!analysis?.summary?.observations) {
    return '';
  }

  const { summary } = analysis;
  const rows = getAllBreakdownRows(analysis.breakdowns);
  const baseFollowRate = summary.pos_1r_before_neg_1r_rate;
  const baseNegRate = summary.hit_neg_1_0r_rate;
  const bestTheme = rows
    .filter((row) => row.observations >= 5)
    .sort(
      (left, right) =>
        right.pos_1r_before_neg_1r_rate - left.pos_1r_before_neg_1r_rate ||
        right.observations - left.observations
    )[0];
  const riskTheme = rows
    .filter((row) => row.observations >= 5)
    .sort(
      (left, right) =>
        right.hit_neg_1_0r_rate - left.hit_neg_1_0r_rate ||
        right.observations - left.observations
    )[0];
  const action =
    baseFollowRate >= 0.58 && summary.avg_return_3x_r > 0
      ? 'candidate_entry_bias = participate'
      : baseNegRate >= 0.48
        ? 'candidate_entry_bias = avoid_without_extra_filter'
        : 'candidate_entry_bias = watchlist_only';
  const target =
    summary.hit_pos_1_0r_rate >= 0.6
      ? 'candidate_target = first +1R, then review trail'
      : summary.avg_return_5x_r > summary.avg_return_3x_r
        ? 'candidate_target = hold toward 5x window'
        : 'candidate_target = collect by 3x window';
  const lines = [
    `family_key = ${analysis.family?.family_key ?? ''}`,
    action,
    target,
    `base_follow_through = +1R before -1R ${formatPercent(baseFollowRate)}`,
    `base_risk = -1R hit ${formatPercent(baseNegRate)}`,
    `avg_3x_close = ${formatR(summary.avg_return_3x_r)}`,
    `avg_5x_close = ${formatR(summary.avg_return_5x_r)}`,
  ];

  if (bestTheme && bestTheme.pos_1r_before_neg_1r_rate >= baseFollowRate + 0.05) {
    lines.push(
      `favor_when ${bestTheme.title} = ${bestTheme.value} (${formatPercent(
        bestTheme.pos_1r_before_neg_1r_rate
      )} +1R-before--1R, ${formatNumber(bestTheme.observations)} obs)`
    );
  }

  if (riskTheme && riskTheme.hit_neg_1_0r_rate >= baseNegRate + 0.05) {
    lines.push(
      `avoid_when ${riskTheme.title} = ${riskTheme.value} (${formatPercent(
        riskTheme.hit_neg_1_0r_rate
      )} -1R hit, ${formatNumber(riskTheme.observations)} obs)`
    );
  }

  return lines.join('\n');
};

const PatternDiscoveryPage = ({ initialFamily = null, onOpenSimulatorFamily = null }) => {
  const [families, setFamilies] = useState([]);
  const [familyError, setFamilyError] = useState('');
  const [isLoadingFamilies, setLoadingFamilies] = useState(false);
  const [selectedFamilyId, setSelectedFamilyId] = useState(getFamilyId(initialFamily));
  const [manualFamilyId, setManualFamilyId] = useState(getFamilyId(initialFamily));
  const [analysis, setAnalysis] = useState(null);
  const [logicDraft, setLogicDraft] = useState('');
  const [analysisError, setAnalysisError] = useState('');
  const [isLoadingAnalysis, setLoadingAnalysis] = useState(false);
  const [isSavingLogic, setSavingLogic] = useState(false);
  const [saveMessage, setSaveMessage] = useState('');

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

  const runDiscovery = async (family) => {
    const familyId = getFamilyId(family);
    if (!familyId) return;

    try {
      setSelectedFamilyId(familyId);
      setManualFamilyId(familyId);
      setAnalysis(null);
      setAnalysisError('');
      setSaveMessage('');
      setLoadingAnalysis(true);
      const data = await fetchPatternDiscoveryFamily({ familyId, limit: 40 });

      if (!data) {
        setAnalysisError('No forward observations were returned for this family.');
        return;
      }

      setAnalysis(data);
      setLogicDraft(buildCandidateLogic(data));
    } catch (error) {
      console.error(error);
      setAnalysisError('Pattern discovery failed.');
    } finally {
      setLoadingAnalysis(false);
    }
  };

  const runManualFamily = () => {
    const familyId = manualFamilyId.trim();
    if (!familyId) return;
    void runDiscovery({
      id: familyId,
      propStrategyId: familyId,
      prop_strategy_id: familyId,
    });
  };

  const handleSaveLogic = async () => {
    const familyId = analysis?.family?.family_key ?? selectedFamilyId;
    if (!familyId || !logicDraft.trim()) return;

    try {
      setSavingLogic(true);
      setSaveMessage('');
      const savedLogic = await savePatternDiscoveryLogic({
        familyId,
        title: 'Forward observation candidate',
        logicText: logicDraft.trim(),
        ruleJson: JSON.stringify({
          source: 'pattern_discovery',
          summary: analysis.summary,
          generated_at: new Date().toISOString(),
        }),
      });
      setAnalysis((current) => (current ? { ...current, saved_logic: savedLogic } : current));
      setSaveMessage('Saved');
    } finally {
      setSavingLogic(false);
    }
  };

  const topThemes = useMemo(() => {
    if (!analysis) return [];
    return getAllBreakdownRows(analysis.breakdowns)
      .filter((row) => row.observations >= 5)
      .sort(
        (left, right) =>
          right.pos_1r_before_neg_1r_rate - left.pos_1r_before_neg_1r_rate ||
          right.observations - left.observations
      )
      .slice(0, 6);
  }, [analysis]);

  const riskThemes = useMemo(() => {
    if (!analysis) return [];
    return getAllBreakdownRows(analysis.breakdowns)
      .filter((row) => row.observations >= 5)
      .sort(
        (left, right) =>
          right.hit_neg_1_0r_rate - left.hit_neg_1_0r_rate ||
          right.observations - left.observations
      )
      .slice(0, 6);
  }, [analysis]);

  return (
    <div className="pattern-discovery-page">
      <aside className="pattern-discovery-sidebar">
        <div className="pattern-discovery-sidebar-head">
          <div>
            <span>Pattern Discovery</span>
            <strong>{formatNumber(families.length)} families</strong>
          </div>
          <button
            type="button"
            className="pattern-discovery-run-button"
            onClick={() => selectedFamily && runDiscovery(selectedFamily)}
            disabled={!selectedFamily || isLoadingAnalysis}
          >
            {isLoadingAnalysis ? 'Running' : 'Run'}
          </button>
        </div>

        <label className="pattern-discovery-manual">
          <span>Family ID</span>
          <div>
            <input
              value={manualFamilyId}
              onChange={(event) => setManualFamilyId(event.target.value)}
              spellCheck="false"
            />
            <button
              type="button"
              onClick={runManualFamily}
              disabled={!manualFamilyId.trim() || isLoadingAnalysis}
            >
              Run ID
            </button>
          </div>
        </label>

        {familyError ? <div className="pattern-discovery-error">{familyError}</div> : null}

        <div className="pattern-discovery-family-list">
          {families.map((family) => {
            const familyId = getFamilyId(family);
            const isSelected = familyId === selectedFamilyId;
            return (
              <button
                type="button"
                className={
                  isSelected
                    ? 'pattern-discovery-family-row pattern-discovery-family-row--selected'
                    : 'pattern-discovery-family-row'
                }
                key={familyId}
                onClick={() => runDiscovery(family)}
              >
                <span>{familyId}</span>
                <strong>{formatPercent(family.win_rate)}</strong>
                <small>{getFamilyLabel(family)}</small>
              </button>
            );
          })}
        </div>
      </aside>

      <main className="pattern-discovery-workspace">
        {analysisError ? <div className="pattern-discovery-error">{analysisError}</div> : null}

        {!analysis ? (
          <div className="pattern-discovery-empty">
            <span>{isLoadingFamilies ? 'Loading families' : 'Select a family'}</span>
            <strong>{isLoadingAnalysis ? 'Reading forward observations...' : 'Discovery results will appear here'}</strong>
          </div>
        ) : (
          <>
            <section className="pattern-discovery-hero">
              <div>
                <span>Selected Family</span>
                <strong>{analysis.family.family_key}</strong>
                <small>{getDiscoveryFamilyLabel(analysis.family)}</small>
              </div>
              <button
                type="button"
                className="pattern-discovery-secondary-button"
                onClick={() => onOpenSimulatorFamily?.(analysis.family.family_key)}
              >
                Simulator
              </button>
            </section>

            <section className="pattern-discovery-score-grid">
              <div>
                <span>Observations</span>
                <strong>{formatNumber(analysis.summary.observations)}</strong>
                <small>{formatNumber(analysis.summary.complete_windows)} complete windows</small>
              </div>
              <div>
                <span>+1R First</span>
                <strong>{formatPercent(analysis.summary.pos_1r_before_neg_1r_rate)}</strong>
                <small>{formatPercent(analysis.summary.hit_pos_1_0r_rate)} hit +1R</small>
              </div>
              <div>
                <span>Risk Hit</span>
                <strong>{formatPercent(analysis.summary.hit_neg_1_0r_rate)}</strong>
                <small>{formatPercent(analysis.summary.hit_neg_0_5r_rate)} hit -0.5R</small>
              </div>
              <div>
                <span>5x Close</span>
                <strong>{formatR(analysis.summary.avg_return_5x_r)}</strong>
                <small>3x {formatR(analysis.summary.avg_return_3x_r)}</small>
              </div>
            </section>

            <section className="pattern-discovery-panel pattern-discovery-logic-panel">
              <div className="pattern-discovery-panel-head">
                <span>Candidate Logic</span>
                <strong>{saveMessage || `${analysis.saved_logic.length} saved`}</strong>
              </div>
              <textarea
                className="pattern-discovery-logic-editor"
                value={logicDraft}
                onChange={(event) => setLogicDraft(event.target.value)}
                spellCheck="false"
              />
              <div className="pattern-discovery-actions">
                <button
                  type="button"
                  className="pattern-discovery-run-button"
                  onClick={handleSaveLogic}
                  disabled={!logicDraft.trim() || isSavingLogic}
                >
                  {isSavingLogic ? 'Saving' : 'Save Logic'}
                </button>
              </div>
            </section>

            <section className="pattern-discovery-theme-grid">
              <div className="pattern-discovery-panel">
                <div className="pattern-discovery-panel-head">
                  <span>Best Follow-Through</span>
                  <strong>+1R First</strong>
                </div>
                <div className="pattern-discovery-table">
                  {topThemes.map((row) => (
                    <div className="pattern-discovery-table-row" key={`top-${row.title}-${row.value}`}>
                      <span>{row.title}: {row.value}</span>
                      <strong>{formatPercent(row.pos_1r_before_neg_1r_rate)}</strong>
                      <small>{formatNumber(row.observations)} obs / 5x {formatR(row.avg_return_5x_r)}</small>
                    </div>
                  ))}
                </div>
              </div>

              <div className="pattern-discovery-panel">
                <div className="pattern-discovery-panel-head">
                  <span>Risk Themes</span>
                  <strong>-1R Hit</strong>
                </div>
                <div className="pattern-discovery-table">
                  {riskThemes.map((row) => (
                    <div className="pattern-discovery-table-row" key={`risk-${row.title}-${row.value}`}>
                      <span>{row.title}: {row.value}</span>
                      <strong>{formatPercent(row.hit_neg_1_0r_rate)}</strong>
                      <small>{formatNumber(row.observations)} obs / MFE {formatR(row.avg_mfe_r)}</small>
                    </div>
                  ))}
                </div>
              </div>
            </section>

            <section className="pattern-discovery-breakdown-grid">
              {analysis.breakdowns.map((breakdown) => (
                <div className="pattern-discovery-panel" key={breakdown.title}>
                  <div className="pattern-discovery-panel-head">
                    <span>{breakdown.title}</span>
                    <strong>{breakdown.rows.length} rows</strong>
                  </div>
                  <div className="pattern-discovery-table">
                    {breakdown.rows.map((row) => (
                      <div className="pattern-discovery-table-row" key={`${breakdown.title}-${row.value}`}>
                        <span>{row.value}</span>
                        <strong>{formatPercent(row.pos_1r_before_neg_1r_rate)}</strong>
                        <small>
                          {formatNumber(row.observations)} obs / -1R {formatPercent(row.hit_neg_1_0r_rate)}
                        </small>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </section>

            <section className="pattern-discovery-panel">
              <div className="pattern-discovery-panel-head">
                <span>Recent Observations</span>
                <strong>{analysis.examples.length} rows</strong>
              </div>
              <div className="pattern-discovery-example-table">
                <div className="pattern-discovery-example-row pattern-discovery-example-row--head">
                  <span>Symbol</span>
                  <span>Entry</span>
                  <span>MFE</span>
                  <span>MAE</span>
                  <span>3x</span>
                  <span>5x</span>
                  <span>First</span>
                </div>
                {analysis.examples.map((example) => (
                  <div className="pattern-discovery-example-row" key={example.observation_id}>
                    <span>{example.symbol}</span>
                    <span>{formatDate(example.entry_date)}</span>
                    <span>{formatR(example.mfe_r)}</span>
                    <span>{formatR(example.mae_r)}</span>
                    <span>{formatR(example.close_return_3x_r)}</span>
                    <span>{formatR(example.close_return_5x_r)}</span>
                    <span>
                      {example.hit_pos_1r_before_neg_1r === null
                        ? 'None'
                        : example.hit_pos_1r_before_neg_1r
                          ? '+1R'
                          : '-1R'}
                    </span>
                  </div>
                ))}
              </div>
            </section>

            {analysis.saved_logic.length ? (
              <section className="pattern-discovery-panel">
                <div className="pattern-discovery-panel-head">
                  <span>Saved Logic</span>
                  <strong>{analysis.saved_logic.length}</strong>
                </div>
                <div className="pattern-discovery-saved-list">
                  {analysis.saved_logic.map((logic) => (
                    <div className="pattern-discovery-saved-row" key={logic.id}>
                      <span>{logic.title}</span>
                      <pre>{logic.logic_text}</pre>
                      <small>{formatDate(logic.created_at)}</small>
                    </div>
                  ))}
                </div>
              </section>
            ) : null}
          </>
        )}
      </main>
    </div>
  );
};

export default PatternDiscoveryPage;
