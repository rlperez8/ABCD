import React, { useEffect, useMemo, useRef, useState } from 'react';

const COLUMN_LABELS = {
  simulator_result: 'Test Result',
  trade_result: 'Result',
  symbol: 'Symbol',
  market: 'Market',
  size_bucket: 'Size',
  closest_pattern: 'Closest Pattern',
  closest_accuracy: 'Accuracy',
  entry_date: 'Enter Date',
  trade_enter_price: 'Enter Price',
};

const getColumnClassName = (columnKey) =>
  `pattern-library-column--${columnKey.replace(/_/g, '-')}`;

const RESULT_LABELS = {
  won: 'Won',
  lost: 'Lost',
  skipped: 'Skipped',
  open: 'Open',
  pending: '',
};

const HARMONIC_ACCURACY_FIELDS = [
  { label: 'Bat', key: 'bat_accuracy' },
  { label: 'Butterfly', key: 'butterfly_accuracy' },
  { label: 'Gartley', key: 'gartley_accuracy' },
  { label: 'Crab', key: 'crab_accuracy' },
  { label: 'Shark', key: 'shark_accuracy' },
];

const normalizeHarmonicLabel = (value) => {
  if (typeof value !== 'string') {
    return null;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const normalized = trimmed.toLowerCase();
  if (normalized === 'none' || normalized === 'null' || normalized === 'undefined') {
    return null;
  }

  return trimmed;
};

const getClosestPatternMatch = (pattern = {}) => {
  const bestMatch = HARMONIC_ACCURACY_FIELDS.reduce((currentBest, entry) => {
    const value = pattern?.[entry.key];

    if (!Number.isFinite(value)) {
      return currentBest;
    }

    if (!currentBest || value > currentBest.accuracy) {
      return { label: entry.label, accuracy: value };
    }

    return currentBest;
  }, null);

  if (bestMatch) {
    const fallbackLabel = normalizeHarmonicLabel(pattern?.harmonic_type);
    return fallbackLabel ? { label: fallbackLabel, accuracy: bestMatch.accuracy } : bestMatch;
  }

  const fallbackLabel = normalizeHarmonicLabel(pattern?.harmonic_type);
  if (fallbackLabel) {
    return { label: fallbackLabel, accuracy: null };
  }

  return { label: 'Match unavailable', accuracy: null };
};

const getColumnValue = (row, columnKey) => {
  const closestPatternMatch = getClosestPatternMatch(row);

  if (columnKey === 'simulator_result') {
    return row.simulator_result_status;
  }

  if (columnKey === 'closest_pattern') {
    return closestPatternMatch.label;
  }

  if (columnKey === 'closest_accuracy') {
    return closestPatternMatch.accuracy;
  }

  if (columnKey === 'entry_date') {
    return row.entry_date ?? row.reversal_detect_date ?? row.d_confirm_date ?? row.d_date;
  }

  return row[columnKey] ?? row[columnKey.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase())];
};

const getNumericResultStatus = (value) => {
  if (Number(value) === 1) {
    return 'won';
  }

  if (Number(value) === 2) {
    return 'lost';
  }

  return 'open';
};

const getSimulatorStatus = (row = {}) => row.simulator_result_status ?? 'pending';

const getResultLabel = (status) => RESULT_LABELS[status] ?? '';

const formatDateParts = (value) => {
  if (!value) {
    return { date: '', time: '' };
  }

  const text = String(value);
  const [datePart, timePart = ''] = text.replace('T', ' ').split(' ');

  return {
    date: datePart,
    time: timePart ? timePart.slice(0, 5) : '',
  };
};

const formatPrice = (value) => {
  const numericValue = Number(value);
  return Number.isFinite(numericValue) ? numericValue.toFixed(2) : value ?? '';
};

const formatAccuracy = (value) => {
  const numericValue = Number(value);
  return Number.isFinite(numericValue) ? numericValue : null;
};

const getShortPatternId = (pattern = {}) => {
  const value = pattern.pattern_id ?? pattern.pattern_group_id ?? '';
  if (!value) {
    return '';
  }

  const text = String(value);
  return text.length > 10 ? text.slice(-10) : text;
};

const getPatternSelectionKey = (pattern = {}) => {
  if (!pattern) {
    return '';
  }

  if (pattern.pattern_id) {
    return `pattern:${pattern.pattern_id}`;
  }

  return [
    pattern.pattern_group_id ?? '',
    pattern.symbol ?? '',
    pattern.d_date ?? '',
    pattern.trade_enter_price ?? '',
    pattern.market ?? '',
  ].join('|');
};

const getPatternMatchKeys = (pattern = {}) => {
  const keys = [];

  if (pattern.pattern_id) {
    keys.push(`id:${pattern.pattern_id}`);
  }

  if (pattern.pattern_group_id) {
    keys.push(`group:${pattern.pattern_group_id}`);
    [pattern.entry_date, pattern.reversal_detect_date, pattern.d_confirm_date, pattern.d_date]
      .filter(Boolean)
      .forEach((dateValue) => {
        keys.push(`group-date:${pattern.pattern_group_id}|${String(dateValue).slice(0, 19)}`);
        keys.push(`group-day:${pattern.pattern_group_id}|${String(dateValue).slice(0, 10)}`);
      });
  }

  return keys;
};

const renderCellContent = (row, columnKey) => {
  const content = getColumnValue(row, columnKey);
  const closestPatternMatch = getClosestPatternMatch(row);

  if (columnKey === 'simulator_result') {
    const status = getSimulatorStatus(row);
    const label = getResultLabel(status);
    return label ? (
      <span className={`pattern-result-pill pattern-result-pill--${status}`}>{label}</span>
    ) : (
      <span className="pattern-result-placeholder" aria-label="Not tested" />
    );
  }

  if (columnKey === 'trade_result') {
    const status = getNumericResultStatus(content);
    return (
      <span className={`pattern-result-pill pattern-result-pill--trade pattern-result-pill--${status}`}>
        {getResultLabel(status)}
      </span>
    );
  }

  if (columnKey === 'symbol') {
    const shortId = getShortPatternId(row);
    return (
      <span className="pattern-trade-cell">
        <strong>{content ?? 'N/A'}</strong>
        {shortId ? <small>{shortId}</small> : null}
      </span>
    );
  }

  if (columnKey === 'market') {
    const market = typeof content === 'string' ? content : 'Unknown';
    return (
      <span className={`pattern-chip pattern-chip--${market.toLowerCase()}`}>
        {market}
      </span>
    );
  }

  if (columnKey === 'size_bucket') {
    return <span className="pattern-chip pattern-chip--neutral">{content ?? 'Unknown'}</span>;
  }

  if (columnKey === 'closest_pattern') {
    return (
      <span className="pattern-match-cell">
        <strong>{closestPatternMatch.label}</strong>
        <small>{row.reversal_type ?? 'None'}</small>
      </span>
    );
  }

  if (columnKey === 'closest_accuracy') {
    const accuracy = formatAccuracy(content);
    return (
      <span className="pattern-accuracy-cell">
        <span>{accuracy === null ? 'N/A' : `${accuracy.toFixed(1)}%`}</span>
        <span className="pattern-accuracy-track">
          <span
            className="pattern-accuracy-fill"
            style={{ width: `${Math.max(0, Math.min(100, accuracy ?? 0))}%` }}
          />
        </span>
      </span>
    );
  }

  if (columnKey === 'entry_date') {
    const { date, time } = formatDateParts(content);
    return (
      <span className="pattern-date-cell">
        <strong>{date}</strong>
        {time ? <small>{time}</small> : null}
      </span>
    );
  }

  if (columnKey === 'trade_enter_price') {
    return <span className="pattern-price-cell">{formatPrice(content)}</span>;
  }

  return typeof content === 'number' ? content.toFixed(2) : content;
};

const PatternTable = ({
  patterns = [],
  totalPatternCount,
  hasMorePatterns,
  isLoadingMorePatterns,
  onLoadMorePatterns,
  setLoadingPatterns,
  setChartData,
  selectedRowIndex,
  selectedPatternKey = '',
  setSelectedRowIndex,
  updateSelectedPattern,
  density = 'default',
  variant = 'default',
  includeSizeColumn = false,
  includeSimulatorResultColumn = false,
  statusLabel = 'Matched Patterns',
  emptyMessage = 'No patterns found.',
  onSelectPattern = null,
  highlightedPatternKeys = [],
  fixedHeight = null,
}) => {
  const tableBodyRef = useRef(null);
  const rowRefs = useRef(new Map());
  const latestSelectionRequestRef = useRef(0);
  const lastLoadTriggerCountRef = useRef(0);
  const [hoveredRowIndex, setHoveredIndex] = useState(-1);
  const highlightedPatternKeySet = useMemo(
    () => new Set(highlightedPatternKeys),
    [highlightedPatternKeys]
  );

  const columns = [
    ...(includeSimulatorResultColumn ? ['simulator_result'] : []),
    'trade_result',
    'symbol',
    'market',
    ...(includeSizeColumn ? ['size_bucket'] : []),
    'closest_pattern',
    'closest_accuracy',
    'entry_date',
    'trade_enter_price',
  ];

  const hasResolvedTotalCount =
    typeof totalPatternCount === 'number' && Number.isFinite(totalPatternCount) && totalPatternCount >= 0;
  const tableStats = useMemo(() => {
    const initial = {
      loaded: patterns.length,
      total: hasResolvedTotalCount ? totalPatternCount : patterns.length,
      tested: 0,
      won: 0,
      lost: 0,
      skipped: 0,
      open: 0,
    };

    return patterns.reduce((stats, pattern) => {
      const simulatorStatus = getSimulatorStatus(pattern);
      const tradeStatus = getNumericResultStatus(pattern?.trade_result);

      if (simulatorStatus !== 'pending') {
        stats.tested += 1;
        stats[simulatorStatus] += 1;
      }

      if (tradeStatus === 'open') {
        stats.open += 1;
      }

      return stats;
    }, initial);
  }, [hasResolvedTotalCount, patterns, totalPatternCount]);

  useEffect(() => {
    if (!isLoadingMorePatterns) {
      lastLoadTriggerCountRef.current = patterns.length;
    }
  }, [isLoadingMorePatterns, patterns.length]);

  useEffect(() => {
    if (!Number.isInteger(selectedRowIndex) || selectedRowIndex < 0 || !patterns.length) {
      return;
    }

    const selectedKeyIndex = selectedPatternKey
      ? patterns.findIndex((pattern) => getPatternSelectionKey(pattern) === selectedPatternKey)
      : -1;
    const nextScrollIndex =
      selectedKeyIndex >= 0 ? selectedKeyIndex : Math.min(selectedRowIndex, patterns.length - 1);

    rowRefs.current.get(nextScrollIndex)?.scrollIntoView({
      block: 'nearest',
      inline: 'nearest',
    });
  }, [patterns, patterns.length, selectedPatternKey, selectedRowIndex]);

  useEffect(() => {
    if (!highlightedPatternKeySet.size || !patterns.length) {
      return;
    }

    const activeIndex = patterns.findIndex((pattern) =>
      getPatternMatchKeys(pattern).some((key) => highlightedPatternKeySet.has(key))
    );

    if (activeIndex >= 0) {
      rowRefs.current.get(activeIndex)?.scrollIntoView({
        block: 'nearest',
        inline: 'nearest',
      });
    }
  }, [highlightedPatternKeySet, patterns]);

  const handleSelectRow = async (rowIndex) => {
    const nextIndex = Math.max(0, Math.min(rowIndex, (patterns?.length ?? 1) - 1));
    const selected = patterns?.[nextIndex];

    if (!selected) {
      return;
    }

    const requestId = latestSelectionRequestRef.current + 1;
    latestSelectionRequestRef.current = requestId;

    try {
      setLoadingPatterns(true);
      setSelectedRowIndex(nextIndex);
      tableBodyRef.current?.focus();

      await onSelectPattern?.(selected, nextIndex);

      if (latestSelectionRequestRef.current !== requestId) {
        return;
      }

      await updateSelectedPattern(selected, setChartData);
    } catch (error) {
      console.error('Error fetching data:', error);
      setChartData({ candles: [], rust_patterns: null });
    } finally {
      if (latestSelectionRequestRef.current === requestId) {
        setLoadingPatterns(false);
      }
    }
  };

  const handleTableKeyDown = (event) => {
    if (!patterns?.length) {
      return;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      void handleSelectRow(Math.min((selectedRowIndex ?? -1) + 1, patterns.length - 1));
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      void handleSelectRow(Math.max((selectedRowIndex ?? 0) - 1, 0));
    }
  };

  const handleBodyScroll = (event) => {
    const element = event.currentTarget;
    const distanceFromBottom = element.scrollHeight - element.scrollTop - element.clientHeight;

    if (
      hasMorePatterns &&
      !isLoadingMorePatterns &&
      distanceFromBottom < 180 &&
      lastLoadTriggerCountRef.current === patterns.length
    ) {
      lastLoadTriggerCountRef.current = -1;
      onLoadMorePatterns?.();
    }
  };

  return (
    <div
      className={[
        'pattern-library-table-panel',
        density === 'compact' ? 'table_container--compact' : '',
        variant === 'retro' ? 'table_container--retro' : '',
      ]
        .filter(Boolean)
        .join(' ')}
      style={
        fixedHeight
          ? {
              height: fixedHeight,
              minHeight: fixedHeight,
            }
          : undefined
      }
    >
      <div
        className={[
          'pattern-library-status',
          density === 'compact' ? 'pattern-library-status--compact' : '',
        ]
          .filter(Boolean)
          .join(' ')}
      >
        <div className="pattern-library-status__primary">
          <span className="pattern-library-status__label">{statusLabel}</span>
          <span className="pattern-library-status__count">
            {tableStats.loaded.toLocaleString()}
            {hasResolvedTotalCount ? ` / ${tableStats.total.toLocaleString()}` : ''}
          </span>
        </div>
        <div className="pattern-library-status__secondary">
          {includeSimulatorResultColumn ? (
            <div className="pattern-library-status__metrics">
              <span className="pattern-stat pattern-stat--tested">{tableStats.tested.toLocaleString()} tested</span>
              <span className="pattern-stat pattern-stat--won">{tableStats.won.toLocaleString()} won</span>
              <span className="pattern-stat pattern-stat--lost">{tableStats.lost.toLocaleString()} lost</span>
              <span className="pattern-stat pattern-stat--skipped">{tableStats.skipped.toLocaleString()} skipped</span>
            </div>
          ) : null}
          <span
            className={[
              'pattern-library-status__hint',
              isLoadingMorePatterns ? 'pattern-library-status__hint--loading' : '',
              !isLoadingMorePatterns && !hasMorePatterns ? 'pattern-library-status__hint--complete' : '',
            ]
              .filter(Boolean)
              .join(' ')}
          >
            {isLoadingMorePatterns
              ? 'Loading more rows...'
              : !hasResolvedTotalCount
              ? 'Showing loaded rows'
              : hasMorePatterns
              ? 'Scroll for more rows'
              : 'All matched rows loaded'}
          </span>
        </div>
      </div>

      <div
        ref={tableBodyRef}
        className="strategy-library-table-shell pattern-library-table-shell"
        tabIndex={0}
        onKeyDown={handleTableKeyDown}
        onScroll={handleBodyScroll}
      >
        {patterns?.length > 0 ? (
          <table className="strategy-library-table pattern-table pattern-table--library">
            <thead>
              <tr>
                {columns.map((column) => (
                  <th
                    className={[
                      'ticker_column',
                      'pattern-library-header-cell',
                      getColumnClassName(column),
                    ]
                      .filter(Boolean)
                      .join(' ')}
                    key={column}
                  >
                    {COLUMN_LABELS[column]}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {patterns.map((pattern, rowIndex) => {
                const rowSelectionKey = getPatternSelectionKey(pattern);
                const isSelected = selectedPatternKey
                  ? rowSelectionKey === selectedPatternKey
                  : rowIndex === selectedRowIndex;
                const isHovered = rowIndex === hoveredRowIndex;
                const result = Number(pattern?.trade_result);
                const simulatorResult = pattern?.simulator_result_status ?? null;
                const isReplayActive = getPatternMatchKeys(pattern).some((key) =>
                  highlightedPatternKeySet.has(key)
                );

                return (
                  <tr
                    key={`${pattern.pattern_id ?? pattern.pattern_group_id ?? pattern.symbol}-${rowIndex}`}
                    ref={(node) => {
                      if (node) {
                        rowRefs.current.set(rowIndex, node);
                      } else {
                        rowRefs.current.delete(rowIndex);
                      }
                    }}
                    className={[
                      'strategy-library-row',
                      'pattern-library-row',
                      simulatorResult === 'won' || (!includeSimulatorResultColumn && result === 1)
                        ? 'pattern-library-row--won'
                        : '',
                      simulatorResult === 'lost' || (!includeSimulatorResultColumn && result === 2)
                        ? 'pattern-library-row--lost'
                        : '',
                      simulatorResult === 'skipped' ? 'pattern-library-row--skipped' : '',
                      !includeSimulatorResultColumn && result !== 1 && result !== 2
                        ? 'pattern-library-row--open'
                        : '',
                      simulatorResult ? 'pattern-library-row--simulated' : '',
                      isReplayActive ? 'pattern-library-row--replay-active' : '',
                      isHovered ? 'strategy-library-row--hovered' : '',
                      isSelected ? 'strategy-library-row--selected' : '',
                      isSelected ? 'pattern-library-row--selected' : '',
                    ]
                      .filter(Boolean)
                      .join(' ')}
                    onClick={() => void handleSelectRow(rowIndex)}
                    onMouseEnter={() => setHoveredIndex(rowIndex)}
                    onMouseLeave={() => setHoveredIndex(-1)}
                  >
                    {columns.map((column, columnIndex) => (
                      <td
                        key={column}
                        className={[
                          'pattern-library-cell',
                          getColumnClassName(column),
                          column === 'closest_accuracy' || column === 'trade_enter_price'
                            ? 'pattern-library-cell--numeric'
                            : '',
                          columnIndex === 0 ? 'pattern-library-cell--first' : '',
                          columnIndex === columns.length - 1 ? 'pattern-library-cell--last' : '',
                        ]
                          .filter(Boolean)
                          .join(' ')}
                      >
                        {renderCellContent(pattern, column)}
                      </td>
                    ))}
                  </tr>
                );
              })}
            </tbody>
          </table>
        ) : (
          <div className="strategy-empty pattern-library-empty">{emptyMessage}</div>
        )}
      </div>
    </div>
  );
};

export default PatternTable;
