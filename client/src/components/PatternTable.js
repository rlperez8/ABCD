import React, { useEffect, useRef, useState } from 'react';

const COLUMN_LABELS = {
  trade_result: 'Result',
  symbol: 'Symbol',
  market: 'Market',
  size_bucket: 'Size',
  closest_pattern: 'Closest Pattern',
  closest_accuracy: 'Accuracy',
  d_date: 'Enter Date',
  trade_enter_price: 'Enter Price',
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

  if (columnKey === 'closest_pattern') {
    return closestPatternMatch.label;
  }

  if (columnKey === 'closest_accuracy') {
    return closestPatternMatch.accuracy;
  }

  return row[columnKey] ?? row[columnKey.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase())];
};

const renderCellContent = (row, columnKey, isSelected) => {
  const content = getColumnValue(row, columnKey);

  if (columnKey === 'trade_result' && content === 1) {
    return <div className="first_column_box">Won</div>;
  }

  if (columnKey === 'trade_result' && content === 2) {
    return <div className="lost_column_box">Lost</div>;
  }

  if (columnKey === 'trade_result') {
    return <div className="open_column_box">Open</div>;
  }

  if (columnKey === 'symbol') {
    return (
      <div className="table_symbol_stack">
        <div className="table_symbol_cell">{content}</div>
        {isSelected ? <div className="table_row_indicator">In Chart</div> : null}
      </div>
    );
  }

  if (columnKey === 'market') {
    const marketLabel = typeof content === 'string' ? content : 'Unknown';
    const marketClassName =
      marketLabel === 'Bullish'
        ? 'table_market_badge table_market_badge--bullish'
        : marketLabel === 'Bearish'
        ? 'table_market_badge table_market_badge--bearish'
        : 'table_market_badge';

    return <div className={marketClassName}>{marketLabel}</div>;
  }

  if (columnKey === 'closest_pattern' || columnKey === 'size_bucket') {
    return <div className="pattern_match_cell">{content ?? 'Unknown'}</div>;
  }

  if (columnKey === 'closest_accuracy') {
    return (
      <div className="table_accuracy_cell">
        {Number.isFinite(content) ? `${content.toFixed(1)}%` : 'N/A'}
      </div>
    );
  }

  if (columnKey === 'd_date') {
    return <div className="table_meta_cell">{content}</div>;
  }

  if (columnKey === 'trade_enter_price') {
    return (
      <div className="table_price_cell">
        {typeof content === 'number' ? content.toFixed(2) : content}
      </div>
    );
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
  setSelectedRowIndex,
  updateSelectedPattern,
  density = 'default',
  variant = 'default',
  includeSizeColumn = false,
  statusLabel = 'Matched Patterns',
  emptyMessage = 'No patterns found.',
  onSelectPattern = null,
  fixedHeight = null,
}) => {
  const tableBodyRef = useRef(null);
  const rowRefs = useRef(new Map());
  const latestSelectionRequestRef = useRef(0);
  const lastLoadTriggerCountRef = useRef(0);
  const [hoveredRowIndex, setHoveredIndex] = useState(-1);

  const columns = includeSizeColumn
    ? [
        'trade_result',
        'symbol',
        'market',
        'size_bucket',
        'closest_pattern',
        'closest_accuracy',
        'd_date',
        'trade_enter_price',
      ]
    : [
        'trade_result',
        'symbol',
        'market',
        'closest_pattern',
        'closest_accuracy',
        'd_date',
        'trade_enter_price',
      ];

  const hasResolvedTotalCount =
    typeof totalPatternCount === 'number' && Number.isFinite(totalPatternCount) && totalPatternCount >= 0;

  useEffect(() => {
    if (!isLoadingMorePatterns) {
      lastLoadTriggerCountRef.current = patterns.length;
    }
  }, [isLoadingMorePatterns, patterns.length]);

  useEffect(() => {
    if (!Number.isInteger(selectedRowIndex) || selectedRowIndex < 0 || !patterns.length) {
      return;
    }

    rowRefs.current.get(Math.min(selectedRowIndex, patterns.length - 1))?.scrollIntoView({
      block: 'nearest',
      inline: 'nearest',
    });
  }, [patterns.length, selectedRowIndex]);

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
        'table_container',
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
        className={['table_status_bar', density === 'compact' ? 'table_status_bar--compact' : '']
          .filter(Boolean)
          .join(' ')}
      >
        <div className="table_status_primary">
          <span className="table_status_kicker">{statusLabel}</span>
          <span className="table_status_value">
            {(hasResolvedTotalCount ? totalPatternCount : patterns.length).toLocaleString()}
          </span>
        </div>
        <div className="table_status_secondary">
          <span
            className={[
              'table_status_hint',
              isLoadingMorePatterns ? 'table_status_hint--loading' : '',
              !isLoadingMorePatterns && !hasMorePatterns ? 'table_status_hint--complete' : '',
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
        className="table_body_shell table_body_shell--native"
        tabIndex={0}
        onKeyDown={handleTableKeyDown}
        onScroll={handleBodyScroll}
      >
        {patterns?.length > 0 ? (
          <table className="pattern-table">
            <thead>
              <tr>
                {columns.map((column) => (
                  <th className="ticker_column" key={column}>
                    {COLUMN_LABELS[column]}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {patterns.map((pattern, rowIndex) => {
                const isSelected = rowIndex === selectedRowIndex;
                const isHovered = rowIndex === hoveredRowIndex;

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
                      'pattern-table-row',
                      rowIndex % 2 === 0 ? 'pattern-table-row--even' : 'pattern-table-row--odd',
                      isHovered ? 'pattern-table-row--hovered' : '',
                      isSelected ? 'pattern-table-row--selected' : '',
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
                          'pattern-table-cell',
                          columnIndex === 0 ? 'pattern-table-cell--first' : '',
                          columnIndex === columns.length - 1 ? 'pattern-table-cell--last' : '',
                        ]
                          .filter(Boolean)
                          .join(' ')}
                      >
                        {renderCellContent(pattern, column, isSelected)}
                      </td>
                    ))}
                  </tr>
                );
              })}
            </tbody>
          </table>
        ) : (
          <div className="table_body_empty">{emptyMessage}</div>
        )}
      </div>
    </div>
  );
};

export default PatternTable;
