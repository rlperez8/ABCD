import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { fetchCandleStorageSummary } from '../../services/patternApi';

const formatNumber = (value, maximumFractionDigits = 0) => {
  const numericValue = Number(value);
  if (!Number.isFinite(numericValue)) {
    return '0';
  }

  return new Intl.NumberFormat('en-US', {
    maximumFractionDigits,
  }).format(numericValue);
};

const formatBytes = (bytes) => {
  const numericBytes = Number(bytes);
  if (!Number.isFinite(numericBytes) || numericBytes <= 0) {
    return '0 B';
  }

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = numericBytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${formatNumber(value, unitIndex === 0 ? 0 : 2)} ${units[unitIndex]}`;
};

const formatDate = (value) => {
  if (!value) {
    return 'None';
  }

  return String(value).replace('.000000', '');
};

const formatTableName = (value = '') =>
  String(value)
    .replace('futures_contract_', '')
    .replace('_candles', ' candles')
    .replace('pattern_', 'pattern ')
    .replace('prop_strategy_', 'strategy ')
    .replace(/_/g, ' ');

const StorageMetricCard = ({ label, value, subvalue = null, tone = 'neutral' }) => (
  <div className={`storage-metric-card storage-metric-card--${tone}`}>
    <span>{label}</span>
    <strong>{value}</strong>
    {subvalue ? <small>{subvalue}</small> : null}
  </div>
);

const StorageBar = ({ value = 0, max = 0 }) => {
  const width = max > 0 ? Math.max(2, Math.min(100, (Number(value) / max) * 100)) : 0;

  return (
    <div className="storage-bar" aria-hidden="true">
      <span style={{ width: `${width}%` }} />
    </div>
  );
};

const StorageTable = ({ columns, rows, emptyMessage }) => (
  <div className="storage-table-shell">
    <table className="storage-table">
      <thead>
        <tr>
          {columns.map((column) => (
            <th key={column.key}>{column.label}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.length ? (
          rows.map((row, rowIndex) => (
            <tr key={row.key ?? rowIndex}>
              {columns.map((column) => (
                <td key={column.key}>{column.render ? column.render(row) : row[column.key]}</td>
              ))}
            </tr>
          ))
        ) : (
          <tr>
            <td colSpan={columns.length}>{emptyMessage}</td>
          </tr>
        )}
      </tbody>
    </table>
  </div>
);

const StorageDashboardPage = () => {
  const [storageSummary, setStorageSummary] = useState(null);
  const [isLoading, setLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');
  const [lastLoadedAt, setLastLoadedAt] = useState(null);

  const loadStorageSummary = useCallback(async () => {
    try {
      setLoading(true);
      setErrorMessage('');
      const result = await fetchCandleStorageSummary({
        includeStockSymbols: false,
      });

      if (!result) {
        setErrorMessage('Storage scan failed.');
        return;
      }

      setStorageSummary(result);
      setLastLoadedAt(new Date());
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadStorageSummary();
  }, [loadStorageSummary]);

  const tables = useMemo(() => storageSummary?.tables ?? [], [storageSummary]);
  const futuresRoots = useMemo(() => storageSummary?.futures_roots ?? [], [storageSummary]);
  const engineTables = useMemo(() => storageSummary?.engine_tables ?? [], [storageSummary]);
  const rollupTables = useMemo(() => storageSummary?.rollup_tables ?? [], [storageSummary]);
  const setupRoots = useMemo(() => storageSummary?.setup_roots ?? [], [storageSummary]);
  const setupMarkets = useMemo(() => storageSummary?.setup_markets ?? [], [storageSummary]);
  const scannedTables = useMemo(
    () => [...tables, ...engineTables, ...rollupTables],
    [engineTables, rollupTables, tables]
  );
  const largestTableBytes = useMemo(
    () => Math.max(0, ...scannedTables.map((table) => Number(table.total_bytes) || 0)),
    [scannedTables]
  );
  const largestFuturesBytes = useMemo(
    () => Math.max(0, ...futuresRoots.map((root) => Number(root.estimated_bytes) || 0)),
    [futuresRoots]
  );
  const largestSetupRootBytes = useMemo(
    () => Math.max(0, ...setupRoots.map((root) => Number(root.estimated_bytes) || 0)),
    [setupRoots]
  );
  const largestSetupMarketBytes = useMemo(
    () => Math.max(0, ...setupMarkets.map((market) => Number(market.estimated_bytes) || 0)),
    [setupMarkets]
  );
  const rootSymbolCount = useMemo(
    () => new Set(futuresRoots.map((root) => root.root_symbol)).size,
    [futuresRoots]
  );
  const storedContractCount = useMemo(() => {
    const contractsByRoot = new Map();
    futuresRoots.forEach((root) => {
      const currentCount = contractsByRoot.get(root.root_symbol) ?? 0;
      contractsByRoot.set(root.root_symbol, Math.max(currentCount, Number(root.contract_count) || 0));
    });
    return [...contractsByRoot.values()].reduce((total, count) => total + count, 0);
  }, [futuresRoots]);
  const totalTrackedBytes =
    Number(storageSummary?.total_bytes ?? 0) +
    Number(storageSummary?.engine_total_bytes ?? 0) +
    Number(storageSummary?.rollup_total_bytes ?? 0);
  const totalTrackedRows =
    Number(storageSummary?.total_rows ?? 0) +
    Number(storageSummary?.engine_total_rows ?? 0) +
    Number(storageSummary?.rollup_total_rows ?? 0);

  const tableColumns = [
    {
      key: 'table_name',
      label: 'Table',
      render: (row) => formatTableName(row.table_name),
    },
    {
      key: 'exact_rows',
      label: 'Rows',
      render: (row) => formatNumber(row.exact_rows),
    },
    {
      key: 'total_bytes',
      label: 'Size',
      render: (row) => (
        <div className="storage-size-cell">
          <strong>{formatBytes(row.total_bytes)}</strong>
          <StorageBar value={row.total_bytes} max={largestTableBytes} />
        </div>
      ),
    },
    {
      key: 'bytes_per_row',
      label: 'B/Row',
      render: (row) => formatNumber(row.bytes_per_row, 1),
    },
  ];

  const futuresColumns = [
    { key: 'root_symbol', label: 'Root' },
    {
      key: 'contract_count',
      label: 'Ctr',
      render: (row) => formatNumber(row.contract_count),
    },
    {
      key: 'candle_count',
      label: 'Candles',
      render: (row) => formatNumber(row.candle_count),
    },
    {
      key: 'estimated_bytes',
      label: 'Est. Size',
      render: (row) => (
        <div className="storage-size-cell">
          <strong>{formatBytes(row.estimated_bytes)}</strong>
          <StorageBar value={row.estimated_bytes} max={largestFuturesBytes} />
        </div>
      ),
    },
    {
      key: 'range',
      label: 'Range',
      render: (row) => `${formatDate(row.first_ts)} to ${formatDate(row.last_ts)}`,
    },
  ];

  const setupRootColumns = [
    { key: 'root_symbol', label: 'Root' },
    {
      key: 'contract_count',
      label: 'Ctr',
      render: (row) => formatNumber(row.contract_count),
    },
    {
      key: 'setup_count',
      label: 'Setups',
      render: (row) => formatNumber(row.setup_count),
    },
    {
      key: 'estimated_bytes',
      label: 'Est. Size',
      render: (row) => (
        <div className="storage-size-cell">
          <strong>{formatBytes(row.estimated_bytes)}</strong>
          <StorageBar value={row.estimated_bytes} max={largestSetupRootBytes} />
        </div>
      ),
    },
    {
      key: 'range',
      label: 'Range',
      render: (row) => `${formatDate(row.first_d_date)} to ${formatDate(row.last_d_date)}`,
    },
  ];

  const setupMarketColumns = [
    { key: 'market', label: 'Market' },
    { key: 'harmonic_type', label: 'Pattern' },
    {
      key: 'setup_count',
      label: 'Setups',
      render: (row) => formatNumber(row.setup_count),
    },
    {
      key: 'estimated_bytes',
      label: 'Est. Size',
      render: (row) => (
        <div className="storage-size-cell">
          <strong>{formatBytes(row.estimated_bytes)}</strong>
          <StorageBar value={row.estimated_bytes} max={largestSetupMarketBytes} />
        </div>
      ),
    },
  ];

  return (
    <div className="storage-dashboard-page">
      <div className="storage-dashboard-header">
        <div>
          <span>Database</span>
          <strong>Futures Data Storage</strong>
          <small>{lastLoadedAt ? `Last scan ${lastLoadedAt.toLocaleTimeString()}` : 'Waiting for scan'}</small>
        </div>

        <div className="storage-dashboard-actions">
          <button type="button" onClick={loadStorageSummary} disabled={isLoading}>
            {isLoading ? 'Scanning' : 'Refresh'}
          </button>
        </div>
      </div>

      {errorMessage ? <div className="storage-dashboard-error">{errorMessage}</div> : null}

      <div className="storage-dashboard-metrics">
        <StorageMetricCard
          label="Tracked Storage"
          value={formatBytes(totalTrackedBytes)}
          subvalue={`${formatNumber(totalTrackedRows)} rows across pipeline`}
          tone="blue"
        />
        <StorageMetricCard
          label="Raw Candles"
          value={formatNumber(storageSummary?.total_rows)}
          subvalue={formatBytes(storageSummary?.total_bytes)}
          tone="green"
        />
        <StorageMetricCard
          label="Engine Output"
          value={formatNumber(storageSummary?.engine_total_rows)}
          subvalue={formatBytes(storageSummary?.engine_total_bytes)}
          tone="amber"
        />
        <StorageMetricCard
          label="Rollups"
          value={formatNumber(storageSummary?.rollup_total_rows)}
          subvalue={formatBytes(storageSummary?.rollup_total_bytes)}
          tone="violet"
        />
        <StorageMetricCard
          label="Futures Roots"
          value={formatNumber(rootSymbolCount)}
          subvalue={`${formatNumber(storedContractCount)} contracts stored`}
          tone="blue"
        />
      </div>

      <div className="storage-dashboard-grid">
        <section className="storage-panel">
          <div className="storage-panel-head">
            <span>Raw Data</span>
            <strong>Candle Tables</strong>
          </div>
          <StorageTable
            columns={tableColumns}
            rows={tables}
            emptyMessage={isLoading ? 'Scanning candle tables...' : 'No candle tables found.'}
          />
        </section>

        <section className="storage-panel">
          <div className="storage-panel-head">
            <span>Engine Output</span>
            <strong>Pattern Tables</strong>
          </div>
          <StorageTable
            columns={tableColumns}
            rows={engineTables}
            emptyMessage={isLoading ? 'Scanning engine tables...' : 'No engine tables found.'}
          />
        </section>

        <section className="storage-panel">
          <div className="storage-panel-head">
            <span>Rollups</span>
            <strong>Summary Tables</strong>
          </div>
          <StorageTable
            columns={tableColumns}
            rows={rollupTables}
            emptyMessage={isLoading ? 'Scanning rollup tables...' : 'No rollup tables found.'}
          />
        </section>

        <section className="storage-panel storage-panel--futures">
          <div className="storage-panel-head">
            <span>Raw Data</span>
            <strong>Futures by Root</strong>
          </div>
          <StorageTable
            columns={futuresColumns}
            rows={futuresRoots.map((root) => ({
              ...root,
              key: `${root.table_name}-${root.root_symbol}`,
            }))}
            emptyMessage={isLoading ? 'Scanning futures roots...' : 'No futures roots found.'}
          />
        </section>

        <section className="storage-panel storage-panel--setup-roots">
          <div className="storage-panel-head">
            <span>Engine Output</span>
            <strong>Setups by Root</strong>
          </div>
          <StorageTable
            columns={setupRootColumns}
            rows={setupRoots.map((root) => ({
              ...root,
              key: `${root.table_name}-${root.root_symbol}`,
            }))}
            emptyMessage={isLoading ? 'Scanning setup roots...' : 'No setup roots found.'}
          />
        </section>

        <section className="storage-panel storage-panel--setup-markets">
          <div className="storage-panel-head">
            <span>Engine Output</span>
            <strong>Setup Mix</strong>
          </div>
          <StorageTable
            columns={setupMarketColumns}
            rows={setupMarkets.map((market) => ({
              ...market,
              key: `${market.table_name}-${market.market}-${market.harmonic_type}`,
            }))}
            emptyMessage={isLoading ? 'Scanning setup groups...' : 'No setup groups found.'}
          />
        </section>
      </div>
    </div>
  );
};

export default StorageDashboardPage;
