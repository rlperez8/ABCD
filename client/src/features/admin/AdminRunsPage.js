import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { fetchAdminStatus, runAdminAction } from '../../services/patternApi';

const TIMEFRAME_OPTIONS = ['1m', '3m', '5m', '15m', '30m', '1h', '4h', '12h', '1d'];
const MAINTENANCE_ACTIONS = [
  {
    action: 'rebuild_indexes',
    title: 'Rebuild Indexes',
    description: 'Restore lookup speed after fast scans.',
    tone: 'blue',
  },
  {
    action: 'refresh_rollups',
    title: 'Refresh Rollups',
    description: 'Rebuild family, yearly, contract-week, and cadence summaries.',
    tone: 'green',
  },
  {
    action: 'refresh_storage',
    title: 'Refresh Storage',
    description: 'Update row counts, table sizes, and storage breakdowns.',
    tone: 'violet',
  },
];
const SCAN_TOGGLES = [
  ['defaultFitOnly', 'Default Fit'],
  ['skipProcessedSymbols', 'Skip Processed'],
  ['deferRebuildIndexes', 'Defer Indexes'],
  ['skipPropFamilySummaries', 'Skip Rollups'],
];

const formatNumber = (value, maximumFractionDigits = 0) => {
  const numericValue = Number(value);
  if (!Number.isFinite(numericValue)) {
    return '0';
  }

  return new Intl.NumberFormat('en-US', {
    maximumFractionDigits,
  }).format(numericValue);
};

const formatBytes = (value) => {
  const numericValue = Number(value);
  if (!Number.isFinite(numericValue) || numericValue <= 0) {
    return '0 B';
  }

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = numericValue;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${formatNumber(size, size >= 10 ? 1 : 2)} ${units[unitIndex]}`;
};

const formatDuration = (durationMs) => {
  if (durationMs === null || durationMs === undefined || durationMs === '') {
    return 'Running';
  }

  const numericDuration = Number(durationMs);
  if (!Number.isFinite(numericDuration)) {
    return 'Running';
  }
  if (numericDuration <= 0) {
    return '0s';
  }

  const seconds = numericDuration / 1000;
  if (seconds < 60) {
    return `${formatNumber(seconds, 1)}s`;
  }
  if (seconds < 3600) {
    return `${formatNumber(seconds / 60, 1)}m`;
  }

  return `${formatNumber(seconds / 3600, 1)}h`;
};

const formatDateTime = (value) => {
  if (!value) {
    return 'Unknown';
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleString('en-US', {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });
};

const formatActionTitle = (value = '') =>
  String(value || 'Unknown')
    .split('_')
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');

const operationScope = (operation = {}) =>
  [operation.root_symbol, operation.contract_symbol, operation.source_timeframe]
    .filter(Boolean)
    .join(' / ') || 'System';

const AdminMetricCard = ({ label, value, subvalue = null, tone = 'neutral' }) => (
  <div className={`admin-metric-card admin-metric-card--${tone}`}>
    <span>{label}</span>
    <strong>{value}</strong>
    {subvalue ? <small>{subvalue}</small> : null}
  </div>
);

const AdminPanelHead = ({ eyebrow, title, detail = null }) => (
  <div className="admin-panel-head">
    <div>
      <span>{eyebrow}</span>
      <strong>{title}</strong>
    </div>
    {detail ? <small>{detail}</small> : null}
  </div>
);

const AdminActionCard = ({ action, onRun, disabled }) => (
  <div className={`admin-action-card admin-action-card--${action.tone}`}>
    <div>
      <span>{action.action}</span>
      <strong>{action.title}</strong>
      <small>{action.description}</small>
    </div>
    <button type="button" onClick={() => onRun(action.action)} disabled={disabled}>
      Run
    </button>
  </div>
);

const AdminRunsPage = () => {
  const [status, setStatus] = useState(null);
  const [isLoading, setLoading] = useState(false);
  const [isSubmitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');
  const [lastLoadedAt, setLastLoadedAt] = useState(null);
  const [scanConfig, setScanConfig] = useState({
    rootSymbol: '',
    contractSymbol: '',
    sourceTimeframe: '1m',
    scanConcurrency: '1',
    defaultFitOnly: true,
    skipProcessedSymbols: true,
    deferRebuildIndexes: true,
    skipPropFamilySummaries: true,
  });

  const loadStatus = useCallback(async ({ quiet = false } = {}) => {
    try {
      if (!quiet) {
        setLoading(true);
      }
      setErrorMessage('');
      const result = await fetchAdminStatus();
      if (!result) {
        setErrorMessage('Admin status failed to load.');
        return;
      }

      setStatus(result);
      setLastLoadedAt(new Date());
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      loadStatus({ quiet: true });
    }, 5000);

    return () => window.clearInterval(timer);
  }, [loadStatus]);

  const runningOperations = useMemo(
    () => (status?.operations ?? []).filter((operation) => operation.status === 'running'),
    [status]
  );
  const runningEngineScan = useMemo(
    () => runningOperations.find((operation) => operation.action === 'run_engine_scan') ?? null,
    [runningOperations]
  );
  const latestOperation = status?.operations?.[0] ?? null;
  const latestPhase = status?.engine_phases?.[0] ?? null;
  const engineProgress = status?.engine_progress ?? null;
  const tableSnapshots = status?.table_snapshots ?? [];
  const cacheStates = status?.cache_states ?? [];
  const entryExitTests = status?.entry_exit_tests ?? [];
  const enabledEntryExitTests = entryExitTests.filter((test) => test.is_enabled);
  const activeOperation = runningOperations[0] ?? latestOperation;
  const latestLog =
    latestOperation?.error_message ||
    latestOperation?.output_tail ||
    latestOperation?.command_text ||
    'No output yet.';

  const updateScanConfig = (key, value) => {
    setScanConfig((current) => ({
      ...current,
      [key]: value,
    }));
  };

  const startAction = async (action, options = {}) => {
    try {
      setSubmitting(true);
      setErrorMessage('');
      const result = await runAdminAction(action, options);
      if (!result) {
        setErrorMessage('Action failed to start.');
        return;
      }
      if (result.error) {
        setErrorMessage(result.error);
        return;
      }

      await loadStatus({ quiet: true });
    } finally {
      setSubmitting(false);
    }
  };

  const startScan = () => {
    startAction('run_engine_scan', scanConfig);
  };

  const clearEngine = () => {
    startAction('clear_engine', { confirmText: 'CLEAR ENGINE' });
  };

  const clearPhase1Routes = () => {
    startAction('clear_phase1_routes', { confirmText: 'CLEAR PHASE1 ROUTES' });
  };

  const runEntryExitTests = () => {
    startAction('run_entry_exit_tests');
  };

  return (
    <div className="admin-runs-page">
      <header className="admin-command-hero">
        <div className="admin-command-hero-main">
          <span
            className={`admin-live-pill ${
              runningOperations.length ? 'admin-live-pill--running' : 'admin-live-pill--idle'
            }`}
          >
            {runningOperations.length ? 'Running' : 'Idle'}
          </span>
          <h1>Admin Runs</h1>
          <p>{status?.abcd_dir ?? 'Set ABCD_ADMIN_ABCD_DIR if needed'}</p>
        </div>
        <div className="admin-command-hero-side">
          <div>
            <span>Active Job</span>
            <strong>{activeOperation ? formatActionTitle(activeOperation.action) : 'None'}</strong>
            <small>
              {activeOperation
                ? `${activeOperation.status} / ${formatDuration(activeOperation.duration_ms)}`
                : 'No operation history'}
            </small>
          </div>
          <button type="button" onClick={() => loadStatus()} disabled={isLoading}>
            {isLoading ? 'Refreshing' : 'Refresh'}
          </button>
        </div>
      </header>

      {errorMessage ? <div className="admin-runs-error">{errorMessage}</div> : null}

      <section className="admin-metric-grid">
        <AdminMetricCard
          label="Running"
          value={formatNumber(runningOperations.length)}
          subvalue={runningOperations[0] ? formatActionTitle(runningOperations[0].action) : 'No active job'}
          tone={runningOperations.length ? 'amber' : 'green'}
        />
        <AdminMetricCard
          label="Latest Operation"
          value={latestOperation?.status ?? 'None'}
          subvalue={latestOperation ? formatActionTitle(latestOperation.action) : 'No operation history'}
          tone={latestOperation?.status === 'failed' ? 'red' : 'blue'}
        />
        <AdminMetricCard
          label="Latest Phase"
          value={latestPhase?.symbol || latestPhase?.phase || 'None'}
          subvalue={latestPhase ? `${latestPhase.phase} / ${formatDuration(latestPhase.duration_ms)}` : null}
          tone="violet"
        />
        <AdminMetricCard
          label="Storage Tables"
          value={formatNumber(tableSnapshots.length)}
          subvalue={lastLoadedAt ? `Updated ${lastLoadedAt.toLocaleTimeString()}` : 'Waiting for status'}
          tone="neutral"
        />
      </section>

      {engineProgress && runningEngineScan ? (
        <section className="admin-progress-panel">
          <div className="admin-progress-copy">
            <span>Engine Progress</span>
            <strong>
              {formatNumber(engineProgress.completed_symbols)} / {formatNumber(engineProgress.total_symbols)} done
            </strong>
            <small>
              {engineProgress.latest_symbol || engineProgress.latest_phase || 'Waiting for phase data'}
              {' / '}
              elapsed {formatDuration(engineProgress.elapsed_ms)}
              {' / '}
              left{' '}
              {engineProgress.estimated_remaining_ms === null ||
              engineProgress.estimated_remaining_ms === undefined
                ? 'estimating'
                : formatDuration(engineProgress.estimated_remaining_ms)}
            </small>
          </div>
          <div className="admin-progress-meter" aria-label="Engine scan progress">
            <span style={{ width: `${Math.min(100, Math.max(0, engineProgress.percent_complete))}%` }} />
          </div>
          <strong className="admin-progress-percent">
            {formatNumber(engineProgress.percent_complete, 1)}%
          </strong>
        </section>
      ) : null}

      <main className="admin-dashboard-grid">
        <section className="admin-panel admin-panel--scan">
          <AdminPanelHead eyebrow="Scan Runner" title="Engine Scan" detail={scanConfig.sourceTimeframe} />

          <div className="admin-scan-layout">
            <div className="admin-form-grid">
              <label>
                <span>Root</span>
                <input
                  type="text"
                  placeholder="All roots"
                  value={scanConfig.rootSymbol}
                  onChange={(event) => updateScanConfig('rootSymbol', event.target.value)}
                />
              </label>
              <label>
                <span>Contract</span>
                <input
                  type="text"
                  placeholder="Optional"
                  value={scanConfig.contractSymbol}
                  onChange={(event) => updateScanConfig('contractSymbol', event.target.value)}
                />
              </label>
              <label>
                <span>Timeframe</span>
                <select
                  value={scanConfig.sourceTimeframe}
                  onChange={(event) => updateScanConfig('sourceTimeframe', event.target.value)}
                >
                  {TIMEFRAME_OPTIONS.map((option) => (
                    <option key={option} value={option}>
                      {option}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                <span>Concurrency</span>
                <input
                  type="number"
                  min="1"
                  max="16"
                  step="1"
                  value={scanConfig.scanConcurrency}
                  onChange={(event) => updateScanConfig('scanConcurrency', event.target.value)}
                />
              </label>
            </div>

            <div className="admin-toggle-grid">
              {SCAN_TOGGLES.map(([key, label]) => (
                <label key={key} className="admin-checkbox">
                  <input
                    type="checkbox"
                    checked={Boolean(scanConfig[key])}
                    onChange={(event) => updateScanConfig(key, event.target.checked)}
                  />
                  <span>{label}</span>
                </label>
              ))}
            </div>

            <button
              type="button"
              className="admin-primary-button"
              onClick={startScan}
              disabled={isSubmitting}
            >
              Run Engine Scan
            </button>
          </div>
        </section>

        <aside className="admin-side-stack">
          <section className="admin-panel admin-panel--maintenance">
            <AdminPanelHead eyebrow="Maintenance" title="Actions" />
            <div className="admin-action-list">
              {MAINTENANCE_ACTIONS.map((action) => (
                <AdminActionCard
                  action={action}
                  key={action.action}
                  onRun={startAction}
                  disabled={isSubmitting}
                />
              ))}
            </div>
          </section>

          <section className="admin-panel admin-panel--danger">
            <AdminPanelHead eyebrow="Danger Zone" title="Clear Engine" />
            <div className="admin-danger-body">
              <div className="admin-danger-action">
                <p>Phase 1 route runs, route results, and yearly route cache only.</p>
                <button type="button" onClick={clearPhase1Routes} disabled={isSubmitting}>
                  Clear Phase 1 Routes
                </button>
              </div>
              <div className="admin-danger-action admin-danger-action--critical">
                <p>Generated engine tables, forward observations, and rollups.</p>
                <button type="button" onClick={clearEngine} disabled={isSubmitting}>
                  Clear Engine
                </button>
              </div>
            </div>
          </section>
        </aside>

        <section className="admin-panel admin-panel--tables">
          <AdminPanelHead
            eyebrow="Entry / Exit"
            title="Test Definitions"
            detail={`${enabledEntryExitTests.length} enabled / ${entryExitTests.length} total`}
          />
          <div className="admin-entry-exit-toolbar">
            <div>
              <strong>Run enabled tests on all families</strong>
              <small>Standard run: futures source, all years, max 1k setups per family, forward window is 5x pattern length.</small>
            </div>
            <button type="button" onClick={runEntryExitTests} disabled={isSubmitting}>
              Run Entry / Exit Tests
            </button>
          </div>
          <div className="admin-table-shell admin-entry-exit-table">
            <table className="admin-table">
              <thead>
                <tr>
                  <th>Status</th>
                  <th>Test</th>
                  <th>Entry</th>
                  <th>Stop</th>
                  <th>Target</th>
                  <th>Hold</th>
                </tr>
              </thead>
              <tbody>
                {entryExitTests.length ? (
                  entryExitTests.map((test) => (
                    <tr key={test.test_id}>
                      <td>
                        <span
                          className={`admin-status-pill ${
                            test.is_enabled ? 'admin-status-pill--completed' : 'admin-status-pill--queued'
                          }`}
                        >
                          {test.is_enabled ? 'enabled' : 'off'}
                        </span>
                      </td>
                      <td title={test.notes || test.test_id}>{test.test_name}</td>
                      <td>{test.entry_mode}</td>
                      <td>{test.stop_mode}</td>
                      <td>{formatNumber(test.target_r, 2)}R</td>
                      <td>{formatNumber(test.max_hold_multiple)}x</td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan="6">Run Entry / Exit Tests once to create the test definition table.</td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </section>

        <section className="admin-panel admin-panel--tables">
          <AdminPanelHead eyebrow="Storage" title="Table Health" detail={`${tableSnapshots.length} tracked`} />
          <div className="admin-table-card-grid">
            {tableSnapshots.length ? (
              tableSnapshots.map((table) => (
                <div className="admin-table-card" key={table.table_name}>
                  <span>{table.table_name}</span>
                  <strong>{formatNumber(table.exact_rows)}</strong>
                  <small>{formatBytes(table.total_bytes)} / {formatDateTime(table.refreshed_at)}</small>
                </div>
              ))
            ) : (
              <div className="admin-empty-state">No storage snapshots yet.</div>
            )}
          </div>
        </section>

        <section className="admin-panel admin-panel--cache">
          <AdminPanelHead eyebrow="Caches" title="Readiness" detail={`${cacheStates.length} states`} />
          <div className="admin-cache-list">
            {cacheStates.length ? (
              cacheStates.map((cache) => (
                <div className="admin-cache-row" key={cache.cache_name}>
                  <span className={cache.is_ready ? 'admin-cache-dot admin-cache-dot--ready' : 'admin-cache-dot'} />
                  <div>
                    <strong>{cache.cache_name}</strong>
                    <small>{cache.note || (cache.is_ready ? 'ready' : 'not ready')}</small>
                  </div>
                  <time>{formatDateTime(cache.updated_at)}</time>
                </div>
              ))
            ) : (
              <div className="admin-empty-state">No cache state rows yet.</div>
            )}
          </div>
        </section>

        <section className="admin-panel admin-panel--history">
          <AdminPanelHead eyebrow="History" title="Operation Runs" />
          <div className="admin-table-shell">
            <table className="admin-table">
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Action</th>
                  <th>Status</th>
                  <th>Scope</th>
                  <th>Duration</th>
                  <th>Started</th>
                </tr>
              </thead>
              <tbody>
                {(status?.operations ?? []).length ? (
                  status.operations.map((operation) => (
                    <tr key={operation.id}>
                      <td>{operation.id}</td>
                      <td>{formatActionTitle(operation.action)}</td>
                      <td>
                        <span className={`admin-status-pill admin-status-pill--${operation.status}`}>
                          {operation.status}
                        </span>
                      </td>
                      <td>{operationScope(operation)}</td>
                      <td>{formatDuration(operation.duration_ms)}</td>
                      <td>{formatDateTime(operation.started_at)}</td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan="6">No admin operations have been started yet.</td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </section>

        <section className="admin-panel admin-panel--logs">
          <AdminPanelHead eyebrow="Output" title="Latest Log Tail" detail={latestOperation?.exit_code !== null && latestOperation?.exit_code !== undefined ? `exit ${latestOperation.exit_code}` : null} />
          <pre>{latestLog}</pre>
        </section>
      </main>
    </div>
  );
};

export default AdminRunsPage;
