import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  fetchPhase1Leaderboard,
  fetchPhase1YearlyBreakdown,
} from '../../services/patternApi';

const formatNumber = (value) =>
  Number.isFinite(Number(value)) ? Number(value).toLocaleString() : '0';

const formatDecimal = (value, digits = 2) =>
  Number.isFinite(Number(value)) ? Number(value).toFixed(digits) : '0.00';

const SOURCE_OPTIONS = [
  { value: 'futures', label: 'Futures' },
  { value: 'daily', label: 'Daily' },
  { value: 'all', label: 'All' },
];

const VIEW_OPTIONS = [
  { value: 'best-family', label: 'Best Per Family' },
  { value: 'all-routes', label: 'All Routes' },
];

const getRouteKey = (route) => `${route.run_id}-${route.route_id}`;

const uniqueValues = (rows, key) =>
  [...new Set(rows.map((row) => row[key]).filter(Boolean))].sort((left, right) =>
    String(left).localeCompare(String(right))
  );

const getFamilyFeatureLabel = (row) =>
  [row.harmonic_type, row.bin, row.size_bucket, row.time_bin, row.x_strictness]
    .filter(Boolean)
    .join(' / ');

const getOutcomeText = (row) =>
  [
    row.family_key,
    row.harmonic_type,
    row.bin,
    row.size_bucket,
    row.time_bin,
    row.x_strictness,
    row.route_label,
    row.route_id,
  ]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();

const blocksRouteArrowBrowsing = (target) => {
  const targetTag = target?.tagName;
  return (
    ['INPUT', 'SELECT', 'TEXTAREA'].includes(targetTag) ||
    Boolean(target?.isContentEditable)
  );
};

const copyTextToClipboard = async (text) => {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const textarea = document.createElement('textarea');
  textarea.value = text;
  textarea.setAttribute('readonly', '');
  textarea.style.position = 'fixed';
  textarea.style.opacity = '0';
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand('copy');
  document.body.removeChild(textarea);
};

const Phase1OutcomesPage = ({ onOpenSimulatorFamily = null }) => {
  const [sourceScope, setSourceScope] = useState('futures');
  const [search, setSearch] = useState('');
  const [harmonicType, setHarmonicType] = useState('All');
  const [outcomeView, setOutcomeView] = useState('best-family');
  const [minTrades, setMinTrades] = useState('100');
  const [limit, setLimit] = useState('250');
  const [leaderboardResults, setLeaderboardResults] = useState([]);
  const [isLoading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [selectedRouteKey, setSelectedRouteKey] = useState(null);
  const [yearlyBreakdown, setYearlyBreakdown] = useState(null);
  const [isYearlyLoading, setYearlyLoading] = useState(false);
  const [yearlyError, setYearlyError] = useState('');
  const [yearlyCanBuild, setYearlyCanBuild] = useState(false);
  const [copiedFamilyKey, setCopiedFamilyKey] = useState('');
  const yearlyBreakdownCacheRef = useRef({});
  const selectedRouteButtonRef = useRef(null);

  useEffect(() => {
    let isCancelled = false;

    const loadLeaderboard = async (showLoading = true) => {
      try {
        if (showLoading) {
          setLoading(true);
        }
        setError('');
        const rows = await fetchPhase1Leaderboard({
          sourceScope,
          year: null,
          limit,
          minTradeCount: minTrades,
          minSetupCount: 1,
          bestPerFamily: outcomeView === 'best-family',
        });

        if (!isCancelled) {
          setLeaderboardResults(rows);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setError('Could not load Phase 1 outcomes.');
        }
      } finally {
        if (!isCancelled && showLoading) {
          setLoading(false);
        }
      }
    };

    void loadLeaderboard(true);
    const intervalId = window.setInterval(() => {
      void loadLeaderboard(false);
    }, 30000);

    return () => {
      isCancelled = true;
      window.clearInterval(intervalId);
    };
  }, [limit, minTrades, outcomeView, sourceScope]);

  const harmonicOptions = useMemo(
    () => ['All', ...uniqueValues(leaderboardResults, 'harmonic_type')],
    [leaderboardResults]
  );

  const visibleResults = useMemo(() => {
    const query = search.trim().toLowerCase();

    return leaderboardResults.filter((row) => {
      if (harmonicType !== 'All' && row.harmonic_type !== harmonicType) return false;
      if (query && !getOutcomeText(row).includes(query)) return false;
      return true;
    });
  }, [harmonicType, leaderboardResults, search]);

  const totalTrades = useMemo(
    () => visibleResults.reduce((sum, row) => sum + Number(row.trade_count || 0), 0),
    [visibleResults]
  );
  const bestScore = visibleResults[0]?.score ?? 0;
  const selectedSourceLabel =
    SOURCE_OPTIONS.find((option) => option.value === sourceScope)?.label ?? 'All';
  const selectedRoute = useMemo(
    () => visibleResults.find((result) => getRouteKey(result) === selectedRouteKey) ?? null,
    [selectedRouteKey, visibleResults]
  );
  const yearlyRows = useMemo(() => yearlyBreakdown?.years ?? [], [yearlyBreakdown]);
  const yearlyTradeCount = useMemo(
    () => yearlyRows.reduce((sum, row) => sum + Number(row.trade_count || 0), 0),
    [yearlyRows]
  );
  const positiveYearCount = yearlyRows.filter((row) => Number(row.avg_r || 0) > 0).length;
  const maxAbsYearAvgR = Math.max(
    0.1,
    ...yearlyRows.map((row) => Math.abs(Number(row.avg_r || 0)))
  );

  useEffect(() => {
    const selectedStillVisible = visibleResults.some(
      (result) => getRouteKey(result) === selectedRouteKey
    );

    if (!selectedStillVisible) {
      setSelectedRouteKey(visibleResults[0] ? getRouteKey(visibleResults[0]) : null);
    }
  }, [selectedRouteKey, visibleResults]);

  useEffect(() => {
    selectedRouteButtonRef.current?.scrollIntoView({
      block: 'nearest',
      inline: 'nearest',
    });
    setCopiedFamilyKey('');
  }, [selectedRouteKey]);

  useEffect(() => {
    let isCancelled = false;

    const loadYearlyBreakdown = async () => {
      if (!selectedRoute) {
        setYearlyBreakdown(null);
        setYearlyError('');
        setYearlyCanBuild(false);
        return;
      }

      const routeKey = getRouteKey(selectedRoute);
      const cachedBreakdown = yearlyBreakdownCacheRef.current[routeKey] ?? null;
      setYearlyBreakdown(cachedBreakdown);
      setYearlyCanBuild(cachedBreakdown ? !cachedBreakdown?.years?.length : false);

      try {
        setYearlyLoading(!cachedBreakdown);
        setYearlyError('');
        const data = await fetchPhase1YearlyBreakdown({
          familyKey: selectedRoute.family_key,
          runId: selectedRoute.run_id,
          routeId: selectedRoute.route_id,
          cacheOnly: true,
        });

        if (!isCancelled) {
          yearlyBreakdownCacheRef.current[routeKey] = data;
          setYearlyBreakdown(data);
          setYearlyCanBuild(!data?.years?.length);
        }
      } catch (loadError) {
        console.error(loadError);
        if (!isCancelled) {
          setYearlyError('Could not load yearly breakdown.');
        }
      } finally {
        if (!isCancelled) {
          setYearlyLoading(false);
        }
      }
    };

    void loadYearlyBreakdown();

    return () => {
      isCancelled = true;
    };
  }, [selectedRoute]);

  useEffect(() => {
    if (!selectedRoute || !yearlyCanBuild || yearlyRows.length) return undefined;

    let isCancelled = false;
    const routeKey = getRouteKey(selectedRoute);
    const refreshCachedBreakdown = async () => {
      const data = await fetchPhase1YearlyBreakdown({
        familyKey: selectedRoute.family_key,
        runId: selectedRoute.run_id,
        routeId: selectedRoute.route_id,
        cacheOnly: true,
      });

      if (isCancelled || !data?.years?.length) return;
      yearlyBreakdownCacheRef.current[routeKey] = data;
      setYearlyBreakdown(data);
      setYearlyCanBuild(false);
    };

    const firstRefreshId = window.setTimeout(() => {
      void refreshCachedBreakdown();
    }, 3000);
    const intervalId = window.setInterval(() => {
      void refreshCachedBreakdown();
    }, 10000);

    return () => {
      isCancelled = true;
      window.clearTimeout(firstRefreshId);
      window.clearInterval(intervalId);
    };
  }, [selectedRoute, yearlyCanBuild, yearlyRows.length]);

  const buildYearlyBreakdown = async () => {
    if (!selectedRoute) return;

    try {
      setYearlyLoading(true);
      setYearlyError('');
      setYearlyCanBuild(false);
      const data = await fetchPhase1YearlyBreakdown({
        familyKey: selectedRoute.family_key,
        runId: selectedRoute.run_id,
        routeId: selectedRoute.route_id,
        cacheOnly: false,
      });

      yearlyBreakdownCacheRef.current[getRouteKey(selectedRoute)] = data;
      setYearlyBreakdown(data);
      setYearlyCanBuild(!data?.years?.length);
    } catch (loadError) {
      console.error(loadError);
      setYearlyError('Could not build yearly breakdown.');
      setYearlyCanBuild(true);
    } finally {
      setYearlyLoading(false);
    }
  };

  const copySelectedFamilyKey = async () => {
    if (!selectedRoute?.family_key) return;

    await copyTextToClipboard(selectedRoute.family_key);
    setCopiedFamilyKey(selectedRoute.family_key);
  };

  const openSelectedFamilyInSimulator = () => {
    if (!selectedRoute?.family_key || !onOpenSimulatorFamily) return;

    onOpenSimulatorFamily(selectedRoute.family_key);
  };

  const moveSelectedRoute = useCallback((direction) => {
    if (!visibleResults.length) return;

    const currentIndex = visibleResults.findIndex(
      (result) => getRouteKey(result) === selectedRouteKey
    );
    const fallbackIndex = direction > 0 ? 0 : visibleResults.length - 1;
    const nextIndex =
      currentIndex === -1
        ? fallbackIndex
        : Math.max(0, Math.min(visibleResults.length - 1, currentIndex + direction));

    setSelectedRouteKey(getRouteKey(visibleResults[nextIndex]));
  }, [selectedRouteKey, visibleResults]);

  useEffect(() => {
    const handleRouteArrowBrowsing = (event) => {
      if (event.defaultPrevented || blocksRouteArrowBrowsing(event.target)) return;

      if (event.key === 'ArrowDown' || event.key === 'ArrowRight') {
        event.preventDefault();
        moveSelectedRoute(1);
      }

      if (event.key === 'ArrowUp' || event.key === 'ArrowLeft') {
        event.preventDefault();
        moveSelectedRoute(-1);
      }
    };

    window.addEventListener('keydown', handleRouteArrowBrowsing);
    return () => window.removeEventListener('keydown', handleRouteArrowBrowsing);
  }, [moveSelectedRoute]);

  return (
    <div className="pattern-family-page phase1-outcomes-page">
      <section className="pattern-family-header">
        <div className="pattern-family-header-main">
          <div>
            <span>Phase 1 Outcomes</span>
            <strong>{formatNumber(visibleResults.length)} routes</strong>
            <small>
              {formatNumber(leaderboardResults.length)} ranked {selectedSourceLabel.toLowerCase()}{' '}
              {outcomeView === 'best-family' ? 'family winners' : 'outcomes'} loaded
            </small>
          </div>
          <div className="pattern-family-stats">
            <div>
              <span>Trades</span>
              <strong>{formatNumber(totalTrades)}</strong>
            </div>
            <div>
              <span>Best Score</span>
              <strong>{formatDecimal(bestScore, 1)}</strong>
            </div>
          </div>
        </div>
      </section>

      <section className="pattern-family-controls">
        <label>
          <span>Search</span>
          <input value={search} onChange={(event) => setSearch(event.target.value)} />
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
          <span>View</span>
          <select value={outcomeView} onChange={(event) => setOutcomeView(event.target.value)}>
            {VIEW_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Source</span>
          <select
            value={sourceScope}
            onChange={(event) => setSourceScope(event.target.value)}
          >
            {SOURCE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Min Trades</span>
          <input value={minTrades} onChange={(event) => setMinTrades(event.target.value)} />
        </label>
        <label>
          <span>Limit</span>
          <input value={limit} onChange={(event) => setLimit(event.target.value)} />
        </label>
      </section>

      {error ? <div className="pattern-family-error">{error}</div> : null}

      <div className="phase1-outcomes-workspace">
        <section
          aria-label="Yearly breakdown chart. Use up and down arrows to browse routes."
          className="phase1-yearly-panel"
          onMouseDown={(event) => {
            if (event.target?.tagName !== 'BUTTON') {
              event.currentTarget.focus();
            }
          }}
          tabIndex={0}
        >
          <div className="phase1-results-header">
            <div>
              <span>Yearly Breakdown</span>
              <strong>
                {selectedRoute
                  ? `${positiveYearCount}/${formatNumber(yearlyRows.length)} positive years`
                  : 'No route selected'}
              </strong>
            </div>
            <div className="phase1-selected-route-actions">
              <small>
                {selectedRoute ? selectedRoute.route_label : 'Select a Phase 1 outcome'}
              </small>
              <button
                className="phase1-copy-family-button"
                disabled={!selectedRoute?.family_key}
                onClick={copySelectedFamilyKey}
                type="button"
              >
                {copiedFamilyKey === selectedRoute?.family_key ? 'Copied' : 'Copy Family'}
              </button>
              {onOpenSimulatorFamily ? (
                <button
                  className="phase1-copy-family-button"
                  disabled={!selectedRoute?.family_key}
                  onClick={openSelectedFamilyInSimulator}
                  type="button"
                >
                  Load In Sim
                </button>
              ) : null}
            </div>
          </div>

          {yearlyError ? <div className="pattern-family-error">{yearlyError}</div> : null}
          {isYearlyLoading ? (
            <div className="phase1-yearly-status">Checking yearly rows...</div>
          ) : null}

          {selectedRoute && yearlyRows.length ? (
            <>
              <div className="phase1-yearly-summary">
                <div>
                  <span>Years</span>
                  <strong>{formatNumber(yearlyRows.length)}</strong>
                </div>
                <div>
                  <span>Trades</span>
                  <strong>{formatNumber(yearlyTradeCount)}</strong>
                </div>
                <div>
                  <span>Worst Year</span>
                  <strong>
                    {formatDecimal(
                      yearlyRows.reduce(
                        (worst, row) => Math.min(worst, Number(row.avg_r || 0)),
                        Number.POSITIVE_INFINITY
                      ),
                      3
                    )}
                  </strong>
                </div>
                <div>
                  <span>Best Year</span>
                  <strong>
                    {formatDecimal(
                      yearlyRows.reduce(
                        (best, row) => Math.max(best, Number(row.avg_r || 0)),
                        Number.NEGATIVE_INFINITY
                      ),
                      3
                    )}
                  </strong>
                </div>
              </div>

              <div className="phase1-yearly-chart">
                {yearlyRows.map((row) => {
                  const avgR = Number(row.avg_r || 0);
                  const isPositive = avgR >= 0;
                  const barHeight = `${Math.max(
                    5,
                    Math.min(46, (Math.abs(avgR) / maxAbsYearAvgR) * 46)
                  )}%`;

                  return (
                    <div className="phase1-yearly-column" key={row.year}>
                      <div className="phase1-yearly-bar-track">
                        <div
                          className={`phase1-yearly-bar${
                            isPositive
                              ? ' phase1-yearly-bar--positive'
                              : ' phase1-yearly-bar--negative'
                          }`}
                          style={{ height: barHeight }}
                          title={`${row.year}: ${formatDecimal(avgR, 3)} avg R`}
                        />
                      </div>
                      <strong>{row.year}</strong>
                      <span>{formatDecimal(avgR, 3)}R</span>
                      <small>{formatNumber(row.trade_count)} trades</small>
                    </div>
                  );
                })}
              </div>
            </>
          ) : selectedRoute && yearlyCanBuild ? (
            <div className="pattern-family-empty phase1-yearly-empty">
              <span>Yearly rows have not been stored for this route yet.</span>
              <button type="button" onClick={buildYearlyBreakdown}>
                Build Yearly Rows
              </button>
            </div>
          ) : selectedRoute ? (
            <div className="pattern-family-empty phase1-yearly-placeholder">
              Checking yearly rows...
            </div>
          ) : (
            <div className="pattern-family-empty">
              Select a stored Phase 1 outcome to inspect each year.
            </div>
          )}
        </section>

        <section className="phase1-results-table phase1-results-table--leaderboard phase1-outcomes-table">
          <div className="phase1-results-header">
            <div>
              <span>{outcomeView === 'best-family' ? 'Best Per Family' : 'Top Ranked'}</span>
              <strong>{formatNumber(visibleResults.length)} shown</strong>
            </div>
            <small>
              {selectedSourceLabel} all-years optimizer run
            </small>
          </div>

          <div className="phase1-leaderboard-row phase1-leaderboard-row--head">
            <span>Rank</span>
            <span>Family</span>
            <span>Harmonic</span>
            <span>Features</span>
            <span>Route</span>
            <span>Fam Rank</span>
            <span>Avg R</span>
            <span>Win</span>
            <span>Trades</span>
            <span>PF</span>
            <span>DD</span>
            <span>Score</span>
          </div>
          {isLoading ? (
            <div className="pattern-family-empty">Loading Phase 1 outcomes...</div>
          ) : visibleResults.length ? (
            visibleResults.map((result, index) => {
              const routeKey = getRouteKey(result);
              const featureLabel = getFamilyFeatureLabel(result);

              return (
                <button
                  className={`phase1-leaderboard-row phase1-leaderboard-row--button${
                    routeKey === selectedRouteKey ? ' phase1-leaderboard-row--selected' : ''
                  }`}
                  key={routeKey}
                  onClick={() => setSelectedRouteKey(routeKey)}
                  ref={routeKey === selectedRouteKey ? selectedRouteButtonRef : null}
                  type="button"
                >
                  <strong>#{index + 1}</strong>
                  <span className="pattern-family-id" title={result.family_key}>
                    {result.family_key}
                  </span>
                  <span className="pattern-family-badge pattern-family-badge--harmonic">
                    {result.harmonic_type}
                  </span>
                  <span title={featureLabel}>{featureLabel}</span>
                  <span title={result.route_label}>{result.route_label}</span>
                  <span>#{result.result_rank}</span>
                  <strong>{formatDecimal(result.avg_r, 3)}</strong>
                  <span>{formatDecimal(result.win_rate, 1)}%</span>
                  <span>{formatNumber(result.trade_count)}</span>
                  <span>{formatDecimal(result.profit_factor, 2)}</span>
                  <span>{formatDecimal(result.max_drawdown_r, 2)}</span>
                  <span>{formatDecimal(result.score, 1)}</span>
                </button>
              );
            })
          ) : (
            <div className="pattern-family-empty">No stored Phase 1 outcomes matched.</div>
          )}
        </section>
      </div>
    </div>
  );
};

export default Phase1OutcomesPage;
