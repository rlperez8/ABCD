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

const formatPercent = (value) => {
  const numericValue = Number(value);
  if (!Number.isFinite(numericValue)) {
    return '0%';
  }

  return `${formatNumber(numericValue, 1)}%`;
};

const formatDate = (value) => {
  if (!value) {
    return 'None';
  }

  return String(value).replace('.000000', '').slice(0, 10);
};

const formatTableName = (value = '') =>
  String(value)
    .replace('futures_contract_', '')
    .replace('_candles', ' candles')
    .replace('pattern_', 'pattern ')
    .replace('prop_strategy_', 'strategy ')
    .replace(/_/g, ' ');

const formatDiskLocation = (value) => {
  if (!value) {
    return 'database drive';
  }

  const driveMatch = String(value).match(/^[A-Za-z]:/);
  return driveMatch ? `${driveMatch[0]} drive` : 'database drive';
};

const StorageMetricCard = ({ label, value, subvalue = null, tone = 'neutral' }) => (
  <div className={`storage-metric-card storage-metric-card--${tone}`}>
    <span>{label}</span>
    <strong>{value}</strong>
    {subvalue ? <small>{subvalue}</small> : null}
  </div>
);

const STORAGE_THEME_OPTIONS = [
  { key: 'raw', label: 'Raw Candles' },
  { key: 'engine', label: 'Engine Output' },
  { key: 'rollups', label: 'Rollups' },
];

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
                <td key={column.key} className={column.className ?? ''}>
                  {column.render ? column.render(row) : row[column.key]}
                </td>
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

const firstDate = (left, right) => {
  if (!left) return right ?? null;
  if (!right) return left;
  return String(left) < String(right) ? left : right;
};

const lastDate = (left, right) => {
  if (!left) return right ?? null;
  if (!right) return left;
  return String(left) > String(right) ? left : right;
};

const EngineOutputExplorer = ({ setupRoots, setupContracts, setupPatterns, isLoading }) => {
  const [expandedRoot, setExpandedRoot] = useState(null);
  const [expandedContract, setExpandedContract] = useState(null);
  const [expandedTimeframe, setExpandedTimeframe] = useState(null);

  const rootRows = useMemo(() => {
    const rootsBySymbol = new Map();

    setupRoots.forEach((root) => {
      const rootSymbol = root.root_symbol || 'Unknown';
      rootsBySymbol.set(rootSymbol, {
        root_symbol: rootSymbol,
        contract_count: Number(root.contract_count) || 0,
        setup_count: Number(root.setup_count) || 0,
        estimated_bytes: Number(root.estimated_bytes) || 0,
        first_d_date: root.first_d_date ?? null,
        last_d_date: root.last_d_date ?? null,
        contracts: [],
        hasRootSummary: true,
      });
    });

    const findOrCreateRoot = (rootSymbol) => {
      const nextRootSymbol = rootSymbol || 'Unknown';
      const root =
        rootsBySymbol.get(nextRootSymbol) ??
        {
          root_symbol: nextRootSymbol,
          contract_count: 0,
          setup_count: 0,
          estimated_bytes: 0,
          first_d_date: null,
          last_d_date: null,
          contracts: [],
          hasRootSummary: false,
        };

      if (!rootsBySymbol.has(nextRootSymbol)) {
        rootsBySymbol.set(nextRootSymbol, root);
      }

      return root;
    };

    const findOrCreateContract = (root, contractSymbol) => {
      const nextContractSymbol = contractSymbol || 'Unknown';
      let contract = root.contracts.find((item) => item.contract_symbol === nextContractSymbol);

      if (!contract) {
        contract = {
          root_symbol: root.root_symbol,
          contract_symbol: nextContractSymbol,
          timeframe_count: 0,
          setup_count: 0,
          estimated_bytes: 0,
          first_d_date: null,
          last_d_date: null,
          timeframes: [],
          hasContractSummary: false,
        };
        root.contracts.push(contract);
      }

      return contract;
    };

    const findOrCreateTimeframe = (contract, sourceTimeframe) => {
      const nextSourceTimeframe = sourceTimeframe || 'Unknown';
      let timeframe = contract.timeframes.find(
        (item) => item.source_timeframe === nextSourceTimeframe
      );

      if (!timeframe) {
        timeframe = {
          root_symbol: contract.root_symbol,
          contract_symbol: contract.contract_symbol,
          source_timeframe: nextSourceTimeframe,
          setup_count: 0,
          estimated_bytes: 0,
          first_d_date: null,
          last_d_date: null,
          patterns: [],
          hasTimeframeSummary: false,
        };
        contract.timeframes.push(timeframe);
        contract.timeframe_count = contract.timeframes.length;
      }

      return timeframe;
    };

    setupContracts.forEach((row) => {
      const rootSymbol = row.root_symbol || 'Unknown';
      const contractSymbol = row.contract_symbol || 'Unknown';
      const root = findOrCreateRoot(rootSymbol);
      const contract = findOrCreateContract(root, contractSymbol);
      const timeframe = findOrCreateTimeframe(contract, row.source_timeframe || 'Unknown');

      const setupCount = Number(row.setup_count) || 0;
      const estimatedBytes = Number(row.estimated_bytes) || 0;
      timeframe.setup_count += setupCount;
      timeframe.estimated_bytes += estimatedBytes;
      timeframe.first_d_date = firstDate(timeframe.first_d_date, row.first_d_date);
      timeframe.last_d_date = lastDate(timeframe.last_d_date, row.last_d_date);
      timeframe.hasTimeframeSummary = true;

      contract.setup_count += setupCount;
      contract.estimated_bytes += estimatedBytes;
      contract.first_d_date = firstDate(contract.first_d_date, row.first_d_date);
      contract.last_d_date = lastDate(contract.last_d_date, row.last_d_date);
      contract.hasContractSummary = true;
      contract.timeframe_count = contract.timeframes.length;

      if (!root.hasRootSummary) {
        root.setup_count += setupCount;
        root.estimated_bytes += estimatedBytes;
        root.first_d_date = firstDate(root.first_d_date, row.first_d_date);
        root.last_d_date = lastDate(root.last_d_date, row.last_d_date);
      }
    });

    setupPatterns.forEach((row) => {
      const root = findOrCreateRoot(row.root_symbol || 'Unknown');
      const contract = findOrCreateContract(root, row.contract_symbol || 'Unknown');
      const timeframe = findOrCreateTimeframe(contract, row.source_timeframe || 'Unknown');
      const pattern = {
        ...row,
        key: `${root.root_symbol}-${contract.contract_symbol}-${timeframe.source_timeframe}-${row.market}-${row.harmonic_type}`,
      };

      timeframe.patterns.push(pattern);

      if (!timeframe.hasTimeframeSummary) {
        const setupCount = Number(row.setup_count) || 0;
        const estimatedBytes = Number(row.estimated_bytes) || 0;
        timeframe.setup_count += setupCount;
        timeframe.estimated_bytes += estimatedBytes;
        timeframe.first_d_date = firstDate(timeframe.first_d_date, row.first_d_date);
        timeframe.last_d_date = lastDate(timeframe.last_d_date, row.last_d_date);
        contract.setup_count += setupCount;
        contract.estimated_bytes += estimatedBytes;
        contract.first_d_date = firstDate(contract.first_d_date, row.first_d_date);
        contract.last_d_date = lastDate(contract.last_d_date, row.last_d_date);

        if (!root.hasRootSummary) {
          root.setup_count += setupCount;
          root.estimated_bytes += estimatedBytes;
          root.first_d_date = firstDate(root.first_d_date, row.first_d_date);
          root.last_d_date = lastDate(root.last_d_date, row.last_d_date);
        }
      }
    });

    return [...rootsBySymbol.values()]
      .map((root) => ({
        ...root,
        contract_count: Math.max(root.contract_count, root.contracts.length),
        contracts: root.contracts
          .map((contract) => ({
            ...contract,
            timeframes: [...contract.timeframes]
              .sort(
                (left, right) =>
                  (Number(right.setup_count) || 0) - (Number(left.setup_count) || 0)
              )
              .map((timeframe) => ({
                ...timeframe,
                patterns: [...timeframe.patterns].sort(
                  (left, right) =>
                    (Number(right.setup_count) || 0) - (Number(left.setup_count) || 0)
                ),
              })),
          }))
          .sort((left, right) => right.setup_count - left.setup_count),
      }))
      .sort((left, right) => right.setup_count - left.setup_count);
  }, [setupContracts, setupPatterns, setupRoots]);

  const largestRootBytes = useMemo(
    () => Math.max(0, ...rootRows.map((root) => Number(root.estimated_bytes) || 0)),
    [rootRows]
  );
  const largestContractBytes = useMemo(
    () =>
      Math.max(
        0,
        ...rootRows.flatMap((root) =>
          root.contracts.map((contract) => Number(contract.estimated_bytes) || 0)
        )
      ),
    [rootRows]
  );
  const largestTimeframeBytes = useMemo(
    () => Math.max(0, ...setupContracts.map((row) => Number(row.estimated_bytes) || 0)),
    [setupContracts]
  );
  const largestPatternBytes = useMemo(
    () => Math.max(0, ...setupPatterns.map((row) => Number(row.estimated_bytes) || 0)),
    [setupPatterns]
  );

  const toggleRoot = (rootSymbol) => {
    setExpandedRoot((current) => (current === rootSymbol ? null : rootSymbol));
    setExpandedContract(null);
    setExpandedTimeframe(null);
  };

  const toggleContract = (rootSymbol, contractSymbol) => {
    const key = `${rootSymbol}-${contractSymbol}`;
    setExpandedContract((current) => (current === key ? null : key));
    setExpandedTimeframe(null);
  };

  const toggleTimeframe = (rootSymbol, contractSymbol, sourceTimeframe) => {
    const key = `${rootSymbol}-${contractSymbol}-${sourceTimeframe}`;
    setExpandedTimeframe((current) => (current === key ? null : key));
  };

  return (
    <div className="storage-table-shell storage-tree-shell">
      <table className="storage-table storage-tree-table">
        <thead>
          <tr>
            <th>Root</th>
            <th>Contracts</th>
            <th>Setups</th>
            <th>Est. Size</th>
            <th>First</th>
            <th>Last</th>
          </tr>
        </thead>
        <tbody>
          {rootRows.length ? (
            rootRows.map((root) => {
              const isRootOpen = expandedRoot === root.root_symbol;
              return (
                <React.Fragment key={root.root_symbol}>
                  <tr
                    className="storage-tree-row storage-tree-row--root"
                    onClick={() => toggleRoot(root.root_symbol)}
                  >
                    <td>
                      <span className="storage-tree-toggle" aria-hidden="true">
                        {isRootOpen ? '-' : '+'}
                      </span>
                      <strong>{root.root_symbol}</strong>
                    </td>
                    <td>{formatNumber(root.contract_count)}</td>
                    <td>{formatNumber(root.setup_count)}</td>
                    <td>
                      <div className="storage-size-cell">
                        <strong>{formatBytes(root.estimated_bytes)}</strong>
                        <StorageBar value={root.estimated_bytes} max={largestRootBytes} />
                      </div>
                    </td>
                    <td className="storage-table-cell--date">{formatDate(root.first_d_date)}</td>
                    <td className="storage-table-cell--date">{formatDate(root.last_d_date)}</td>
                  </tr>

                  {isRootOpen ? (
                    <tr className="storage-tree-child-row">
                      <td colSpan="6">
                        <table className="storage-table storage-nested-table">
                          <thead>
                            <tr>
                              <th>Contract</th>
                              <th>TFs</th>
                              <th>Setups</th>
                              <th>Est. Size</th>
                              <th>First</th>
                              <th>Last</th>
                            </tr>
                          </thead>
                          <tbody>
                            {root.contracts.map((contract) => {
                              const contractKey = `${root.root_symbol}-${contract.contract_symbol}`;
                              const isContractOpen = expandedContract === contractKey;
                              return (
                                <React.Fragment key={contractKey}>
                                  <tr
                                    className="storage-tree-row storage-tree-row--contract"
                                    onClick={() =>
                                      toggleContract(root.root_symbol, contract.contract_symbol)
                                    }
                                  >
                                    <td>
                                      <span className="storage-tree-toggle" aria-hidden="true">
                                        {isContractOpen ? '-' : '+'}
                                      </span>
                                      <strong>{contract.contract_symbol}</strong>
                                    </td>
                                    <td>{formatNumber(contract.timeframe_count)}</td>
                                    <td>{formatNumber(contract.setup_count)}</td>
                                    <td>
                                      <div className="storage-size-cell">
                                        <strong>{formatBytes(contract.estimated_bytes)}</strong>
                                        <StorageBar
                                          value={contract.estimated_bytes}
                                          max={largestContractBytes}
                                        />
                                      </div>
                                    </td>
                                    <td className="storage-table-cell--date">
                                      {formatDate(contract.first_d_date)}
                                    </td>
                                    <td className="storage-table-cell--date">
                                      {formatDate(contract.last_d_date)}
                                    </td>
                                  </tr>

                                  {isContractOpen ? (
                                    <tr className="storage-tree-child-row storage-tree-child-row--timeframes">
                                      <td colSpan="6">
                                        <table className="storage-table storage-nested-table storage-nested-table--timeframes">
                                          <thead>
                                            <tr>
                                              <th>Timeframe</th>
                                              <th>Patterns</th>
                                              <th>Setups</th>
                                              <th>Est. Size</th>
                                              <th>First</th>
                                              <th>Last</th>
                                            </tr>
                                          </thead>
                                          <tbody>
                                            {contract.timeframes.map((timeframe) => {
                                              const timeframeKey = `${contractKey}-${timeframe.source_timeframe}`;
                                              const isTimeframeOpen = expandedTimeframe === timeframeKey;

                                              return (
                                                <React.Fragment key={timeframeKey}>
                                                  <tr
                                                    className="storage-tree-row storage-tree-row--timeframe"
                                                    onClick={() =>
                                                      toggleTimeframe(
                                                        root.root_symbol,
                                                        contract.contract_symbol,
                                                        timeframe.source_timeframe
                                                      )
                                                    }
                                                  >
                                                    <td>
                                                      <span
                                                        className="storage-tree-toggle"
                                                        aria-hidden="true"
                                                      >
                                                        {isTimeframeOpen ? '-' : '+'}
                                                      </span>
                                                      <strong>
                                                        {timeframe.source_timeframe || 'Unknown'}
                                                      </strong>
                                                    </td>
                                                    <td>{formatNumber(timeframe.patterns.length)}</td>
                                                    <td>{formatNumber(timeframe.setup_count)}</td>
                                                    <td>
                                                      <div className="storage-size-cell">
                                                        <strong>
                                                          {formatBytes(timeframe.estimated_bytes)}
                                                        </strong>
                                                        <StorageBar
                                                          value={timeframe.estimated_bytes}
                                                          max={largestTimeframeBytes}
                                                        />
                                                      </div>
                                                    </td>
                                                    <td className="storage-table-cell--date">
                                                      {formatDate(timeframe.first_d_date)}
                                                    </td>
                                                    <td className="storage-table-cell--date">
                                                      {formatDate(timeframe.last_d_date)}
                                                    </td>
                                                  </tr>

                                                  {isTimeframeOpen ? (
                                                    <tr className="storage-tree-child-row storage-tree-child-row--patterns">
                                                      <td colSpan="6">
                                                        <table className="storage-table storage-nested-table storage-nested-table--patterns">
                                                          <thead>
                                                            <tr>
                                                              <th>Market</th>
                                                              <th>Pattern</th>
                                                              <th>Setups</th>
                                                              <th>Est. Size</th>
                                                              <th>First</th>
                                                              <th>Last</th>
                                                            </tr>
                                                          </thead>
                                                          <tbody>
                                                            {timeframe.patterns.length ? (
                                                              timeframe.patterns.map((pattern) => (
                                                                <tr
                                                                  key={pattern.key}
                                                                  className="storage-tree-row storage-tree-row--pattern"
                                                                >
                                                                  <td>
                                                                    <strong>{pattern.market}</strong>
                                                                  </td>
                                                                  <td>{pattern.harmonic_type}</td>
                                                                  <td>
                                                                    {formatNumber(pattern.setup_count)}
                                                                  </td>
                                                                  <td>
                                                                    <div className="storage-size-cell">
                                                                      <strong>
                                                                        {formatBytes(
                                                                          pattern.estimated_bytes
                                                                        )}
                                                                      </strong>
                                                                      <StorageBar
                                                                        value={
                                                                          pattern.estimated_bytes
                                                                        }
                                                                        max={largestPatternBytes}
                                                                      />
                                                                    </div>
                                                                  </td>
                                                                  <td className="storage-table-cell--date">
                                                                    {formatDate(
                                                                      pattern.first_d_date
                                                                    )}
                                                                  </td>
                                                                  <td className="storage-table-cell--date">
                                                                    {formatDate(pattern.last_d_date)}
                                                                  </td>
                                                                </tr>
                                                              ))
                                                            ) : (
                                                              <tr>
                                                                <td colSpan="6">
                                                                  No pattern breakdown cached for
                                                                  this timeframe yet.
                                                                </td>
                                                              </tr>
                                                            )}
                                                          </tbody>
                                                        </table>
                                                      </td>
                                                    </tr>
                                                  ) : null}
                                                </React.Fragment>
                                              );
                                            })}
                                          </tbody>
                                        </table>
                                      </td>
                                    </tr>
                                  ) : null}
                                </React.Fragment>
                              );
                            })}
                          </tbody>
                        </table>
                      </td>
                    </tr>
                  ) : null}
                </React.Fragment>
              );
            })
          ) : (
            <tr>
              <td colSpan="6">
                {isLoading ? 'Loading engine output...' : 'No engine output rows found.'}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
};

const StorageDashboardPage = () => {
  const [storageSummary, setStorageSummary] = useState(null);
  const [isLoading, setLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');
  const [lastLoadedAt, setLastLoadedAt] = useState(null);
  const [activeTheme, setActiveTheme] = useState('raw');

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
  const setupContracts = useMemo(() => storageSummary?.setup_contracts ?? [], [storageSummary]);
  const setupPatterns = useMemo(() => storageSummary?.setup_patterns ?? [], [storageSummary]);
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
  const diskFreePercent = Number(storageSummary?.disk_free_percent ?? 0);
  const diskTone = diskFreePercent >= 25 ? 'green' : diskFreePercent >= 10 ? 'amber' : 'violet';
  const diskSubvalue = storageSummary?.disk_total_bytes
    ? `${formatPercent(diskFreePercent)} free of ${formatBytes(
        storageSummary.disk_total_bytes
      )} on ${formatDiskLocation(storageSummary.disk_path)}`
    : 'Waiting for drive scan';

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
    {
      key: 'table_name',
      label: 'TF',
      render: (row) => formatTableName(row.table_name).replace(' candles', ''),
    },
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
      key: 'first_ts',
      label: 'First',
      className: 'storage-table-cell--date',
      render: (row) => formatDate(row.first_ts),
    },
    {
      key: 'last_ts',
      label: 'Last',
      className: 'storage-table-cell--date',
      render: (row) => formatDate(row.last_ts),
    },
  ];

  return (
    <div className={`storage-dashboard-page storage-dashboard-page--${activeTheme}`}>
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

      <div className="storage-theme-toggle" role="tablist" aria-label="Storage data theme">
        {STORAGE_THEME_OPTIONS.map((option) => (
          <button
            key={option.key}
            type="button"
            className={activeTheme === option.key ? 'is-active' : ''}
            onClick={() => setActiveTheme(option.key)}
            role="tab"
            aria-selected={activeTheme === option.key}
          >
            {option.label}
          </button>
        ))}
      </div>

      <div className="storage-dashboard-metrics">
        {activeTheme === 'raw' ? (
          <>
            <StorageMetricCard
              label="Raw Candle Rows"
              value={formatNumber(storageSummary?.total_rows)}
              subvalue={formatBytes(storageSummary?.total_bytes)}
              tone="green"
            />
            <StorageMetricCard
              label="Futures Roots"
              value={formatNumber(rootSymbolCount)}
              subvalue={`${formatNumber(storedContractCount)} contracts stored`}
              tone="blue"
            />
          </>
        ) : null}
        {activeTheme === 'engine' ? (
          <>
            <StorageMetricCard
              label="Engine Rows"
              value={formatNumber(storageSummary?.engine_total_rows)}
              subvalue={formatBytes(storageSummary?.engine_total_bytes)}
              tone="amber"
            />
            <StorageMetricCard
              label="Setup Rows"
              value={formatNumber(storageSummary?.setup_total_rows)}
              subvalue={formatBytes(storageSummary?.setup_total_bytes)}
              tone="blue"
            />
          </>
        ) : null}
        {activeTheme === 'rollups' ? (
          <StorageMetricCard
            label="Rollup Rows"
            value={formatNumber(storageSummary?.rollup_total_rows)}
            subvalue={formatBytes(storageSummary?.rollup_total_bytes)}
            tone="violet"
          />
        ) : null}
        <StorageMetricCard
          label="Tracked Storage"
          value={formatBytes(totalTrackedBytes)}
          subvalue={`${formatNumber(totalTrackedRows)} rows across pipeline`}
          tone="blue"
        />
        <StorageMetricCard
          label="Drive Free"
          value={formatBytes(storageSummary?.disk_free_bytes)}
          subvalue={diskSubvalue}
          tone={diskTone}
        />
      </div>

      <div className="storage-dashboard-grid">
        {activeTheme === 'raw' ? (
          <>
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

            <section className="storage-panel storage-panel--wide">
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
          </>
        ) : null}

        {activeTheme === 'engine' ? (
          <>
            <section className="storage-panel storage-panel--compact">
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

            <section className="storage-panel storage-panel--engine-output">
              <div className="storage-panel-head">
                <span>Engine Output</span>
                <strong>Pattern Tree</strong>
              </div>
              <EngineOutputExplorer
                setupRoots={setupRoots}
                setupContracts={setupContracts}
                setupPatterns={setupPatterns}
                isLoading={isLoading}
              />
            </section>
          </>
        ) : null}

        {activeTheme === 'rollups' ? (
          <section className="storage-panel storage-panel--wide">
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
        ) : null}
      </div>
    </div>
  );
};

export default StorageDashboardPage;
