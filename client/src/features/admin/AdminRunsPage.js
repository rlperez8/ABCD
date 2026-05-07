import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { fetchAdminStatus, runAdminAction } from '../../services/patternApi';

const TIMEFRAME_OPTIONS = ['1m', '3m', '5m', '15m', '30m', '1h', '4h', '12h', '1d'];
const MAINTENANCE_ACTIONS = [
  {
    action: 'rebuild_indexes',
    title: 'Rebuild Indexes',
    description: 'Restore generated engine lookup indexes after fast scans.',
  },
  {
    action: 'refresh_rollups',
    title: 'Refresh Rollups',
    description: 'Rebuild family, yearly, contract-week, and cadence summaries.',
  },
  {
    action: 'refresh_storage',
    title: 'Refresh Storage',
    description: 'Update cached row counts, table sizes, and storage tree breakdowns.',
  },
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

  return `${formatNumber(seconds / 60, 1)}m`;
};

const AdminMetricCard = ({ label, value, subvalue = null, tone = 'neutral' }) => (
  <div className={`admin-metric-card admin-metric-card--${tone}`}>
    <span>{label}</span>
    <strong>{value}</strong>
    {subvalue ? <small>{subvalue}</small> : null}
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
  const [clearConfirmText, setClearConfirmText] = useState('');

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
  const latestOperation = status?.operations?.[0] ?? null;
  const latestPhase = status?.engine_phases?.[0] ?? null;

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

      await loadStatus({ quiet: true });
    } finally {
      setSubmitting(false);
    }
  };

  const startScan = () => {
    startAction('run_engine_scan', scanConfig);
  };

  const clearEngine = () => {
    startAction('clear_engine', { confirmText: clearConfirmText });
  };

  return (
    <div className="admin-runs-page">
      <div className="admin-runs-header">
        <div>
          <span>Local Operations</span>
          <strong>Admin Runs</strong>
          <small>
            {lastLoadedAt ? `Last refresh ${lastLoadedAt.toLocaleTimeString()}` : 'Waiting for status'}
          </small>
        </div>
        <button type="button" onClick={() => loadStatus()} disabled={isLoading}>
          {isLoading ? 'Refreshing' : 'Refresh'}
        </button>
      </div>

      {errorMessage ? <div className="admin-runs-error">{errorMessage}</div> : null}

      <div className="admin-metric-grid">
        <AdminMetricCard
          label="Running"
          value={formatNumber(runningOperations.length)}
          subvalue={runningOperations[0]?.action ?? 'No active admin job'}
          tone={runningOperations.length ? 'amber' : 'green'}
        />
        <AdminMetricCard
          label="Latest Operation"
          value={latestOperation?.status ?? 'None'}
          subvalue={latestOperation?.action ?? 'No operation history'}
          tone={latestOperation?.status === 'failed' ? 'red' : 'blue'}
        />
        <AdminMetricCard
          label="Latest Engine Phase"
          value={latestPhase?.symbol || latestPhase?.phase || 'None'}
          subvalue={latestPhase ? `${latestPhase.phase} / ${formatDuration(latestPhase.duration_ms)}` : null}
          tone="violet"
        />
        <AdminMetricCard
          label="ABCD Path"
          value={status?.abcd_dir ? 'Ready' : 'Missing'}
          subvalue={status?.abcd_dir ?? 'Set ABCD_ADMIN_ABCD_DIR if needed'}
          tone={status?.abcd_dir ? 'green' : 'red'}
        />
      </div>

      <div className="admin-operations-grid">
        <section className="admin-panel admin-panel--maintenance">
          <div className="admin-panel-head">
            <span>Maintenance</span>
            <strong>Safe Actions</strong>
          </div>
          <div className="admin-action-list">
            {MAINTENANCE_ACTIONS.map((action) => (
              <div key={action.action} className="admin-action-row">
                <div>
                  <strong>{action.title}</strong>
                  <span>{action.description}</span>
                </div>
                <button
                  type="button"
                  onClick={() => startAction(action.action)}
                  disabled={isSubmitting}
                >
                  Run
                </button>
              </div>
            ))}
          </div>
        </section>

        <section className="admin-panel admin-panel--scan">
          <div className="admin-panel-head">
            <span>Scan Runner</span>
            <strong>Engine Scan</strong>
          </div>

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
            {[
              ['defaultFitOnly', 'Default Fit'],
              ['skipProcessedSymbols', 'Skip Processed'],
              ['deferRebuildIndexes', 'Defer Indexes'],
              ['skipPropFamilySummaries', 'Skip Rollups'],
            ].map(([key, label]) => (
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
            Run Scan
          </button>
        </section>

        <section className="admin-panel admin-panel--danger">
          <div className="admin-panel-head">
            <span>Danger Zone</span>
            <strong>Clear Engine</strong>
          </div>
          <div className="admin-danger-body">
            <p>Clears generated engine tables and rollups. Type CLEAR ENGINE to enable.</p>
            <input
              type="text"
              value={clearConfirmText}
              onChange={(event) => setClearConfirmText(event.target.value)}
              placeholder="CLEAR ENGINE"
            />
            <button
              type="button"
              onClick={clearEngine}
              disabled={isSubmitting || clearConfirmText !== 'CLEAR ENGINE'}
            >
              Clear Engine Tables
            </button>
          </div>
        </section>
      </div>

      <div className="admin-runs-grid">
        <section className="admin-panel admin-panel--history">
          <div className="admin-panel-head">
            <span>History</span>
            <strong>Operation Runs</strong>
          </div>
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
                      <td>{operation.action}</td>
                      <td>
                        <span className={`admin-status-pill admin-status-pill--${operation.status}`}>
                          {operation.status}
                        </span>
                      </td>
                      <td>
                        {[operation.root_symbol, operation.contract_symbol, operation.source_timeframe]
                          .filter(Boolean)
                          .join(' / ') || 'System'}
                      </td>
                      <td>{formatDuration(operation.duration_ms)}</td>
                      <td>{operation.started_at ?? 'Unknown'}</td>
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
          <div className="admin-panel-head">
            <span>Output</span>
            <strong>Latest Log Tail</strong>
          </div>
          <pre>{latestOperation?.output_tail || latestOperation?.command_text || 'No output yet.'}</pre>
        </section>
      </div>
    </div>
  );
};

export default AdminRunsPage;
