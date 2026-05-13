import React, { useEffect, useMemo, useState } from 'react';
import { fetchPatternFamilies, fetchPhase1Results } from '../../services/patternApi';

const formatNumber = (value) =>
  Number.isFinite(Number(value)) ? Number(value).toLocaleString() : '0';

const formatDate = (value) => {
  if (!value) return 'N/A';
  const text = String(value);
  return text.length > 10 ? text.slice(0, 10) : text;
};

const formatDecimal = (value, digits = 2) =>
  Number.isFinite(Number(value)) ? Number(value).toFixed(digits) : '0.00';

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

const getRouteKey = (route) => `${route.run_id}-${route.route_id}`;

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
  const selectedSourceLabel =
    SOURCE_OPTIONS.find((option) => option.value === sourceScope)?.label ?? 'All';

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

  return (
    <div className="pattern-family-page">
      <section className="pattern-family-header">
        <div className="pattern-family-header-main">
          <div>
            <span>Pattern Families</span>
            <strong>{formatNumber(visibleFamilies.length)} families</strong>
            <small>
              {formatNumber(families.length)}{' '}
              {yearFilter === 'All'
                ? `${selectedSourceLabel.toLowerCase()} pattern-identity groups`
                : `${selectedSourceLabel.toLowerCase()} ${yearFilter} groups`}
            </small>
          </div>
          <div className="pattern-family-stats">
            <div>
              <span>Setups</span>
              <strong>{formatNumber(visibleSetupCount)}</strong>
            </div>
            <div>
              <span>Largest</span>
              <strong>{formatNumber(largestFamily)}</strong>
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

      {error ? <div className="pattern-family-error">{error}</div> : null}

      <div className="pattern-family-workspace">
        <section className="pattern-family-table">
          <div className="pattern-family-table-header">
            <div>
              <span>Families</span>
              <strong>{formatNumber(visibleFamilies.length)} shown</strong>
            </div>
            <small>Click a family to view stored Phase 1 routes</small>
          </div>
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
          {isLoading ? (
            <div className="pattern-family-empty">Loading pattern families...</div>
          ) : visibleFamilies.length ? (
            visibleFamilies.map((family) => (
              <button
                className={`pattern-family-row pattern-family-row--button${
                  family.family_key === selectedFamilyKey ? ' pattern-family-row--selected' : ''
                }`}
                key={family.family_key}
                onClick={() => setSelectedFamilyKey(family.family_key)}
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
        </section>

        <section className="phase1-results-table">
          <div className="phase1-results-header">
            <div>
              <span>Phase 1 Results</span>
              <strong>{formatNumber(phase1Results.length)} routes</strong>
            </div>
            <small>
              {selectedFamily
                ? `${selectedFamily.harmonic_type} / ${selectedFamily.bin} / ${selectedFamily.size_bucket} / ${selectedFamily.time_bin} / ${selectedFamily.x_strictness}`
                : 'No family selected'}
            </small>
          </div>

          {phase1Error ? <div className="pattern-family-error">{phase1Error}</div> : null}

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
          {isPhase1Loading ? (
            <div className="pattern-family-empty">Loading Phase 1 results...</div>
          ) : phase1Results.length ? (
            phase1Results.map((result) => {
              const routeKey = getRouteKey(result);

              return (
                <button
                  className={`phase1-results-row phase1-results-row--button${
                    routeKey === selectedRouteKey ? ' phase1-results-row--selected' : ''
                  }`}
                  key={routeKey}
                  onClick={() => setSelectedRouteKey(routeKey)}
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
            <div className="pattern-family-empty">No stored Phase 1 results for this family.</div>
          )}
        </section>
      </div>
    </div>
  );
};

export default PatternFamilyUniversePage;
