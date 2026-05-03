import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import DashboardCardFrame from '../dashboard/DashboardCardFrame';

const formatPercent = (value) =>
  Number.isFinite(value) ? `${value.toFixed(2)}%` : 'N/A';

const SORT_COLUMNS = {
  rank: {
    label: 'Rank',
    getValue: (_, index) => index + 1,
    isNumeric: true,
    defaultDirection: 'asc',
  },
  route: {
    label: 'Route',
    getValue: (strategy) => strategy.outcomeModel ?? '',
    isNumeric: false,
    defaultDirection: 'asc',
  },
  family: {
    label: 'Family',
    getValue: (strategy) => strategy.familyName ?? '',
    isNumeric: false,
    defaultDirection: 'asc',
  },
  market: {
    label: 'Market',
    getValue: (strategy) => strategy.market ?? '',
    isNumeric: false,
    defaultDirection: 'asc',
  },
  pattern: {
    label: 'Pattern',
    getValue: (strategy) => strategy.harmonicType ?? '',
    isNumeric: false,
    defaultDirection: 'asc',
  },
  bin: {
    label: 'Bin',
    getValue: (strategy) => strategy.bin ?? '',
    isNumeric: false,
    defaultDirection: 'asc',
  },
  reversal: {
    label: 'Reversal',
    getValue: (strategy) => strategy.reversalType ?? '',
    isNumeric: false,
    defaultDirection: 'asc',
  },
  size: {
    label: 'Size',
    getValue: (strategy) => strategy.sizeBucket ?? '',
    isNumeric: false,
    defaultDirection: 'asc',
  },
  time: {
    label: 'Time',
    getValue: (strategy) => strategy.timeBin ?? '',
    isNumeric: false,
    defaultDirection: 'asc',
  },
  xMode: {
    label: 'X Mode',
    getValue: (strategy) => strategy.xStrictness ?? '',
    isNumeric: false,
    defaultDirection: 'asc',
  },
  worstYear: {
    label: 'Worst',
    getValue: (strategy) => strategy.worstYearExpectancy ?? Number.NEGATIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'desc',
  },
  downYears: {
    label: 'Down',
    getValue: (strategy) => strategy.downYears ?? Number.POSITIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'asc',
  },
  expectancy: {
    label: 'Exp',
    getValue: (strategy) => strategy?.comparison?.summary?.expectancy ?? Number.NEGATIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'desc',
  },
  score: {
    label: 'Score',
    getValue: (strategy) => strategy?.score ?? Number.NEGATIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'desc',
  },
  winRate: {
    label: 'Win',
    getValue: (strategy) => strategy?.comparison?.summary?.win_rate ?? Number.NEGATIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'desc',
  },
  avgReturn: {
    label: 'Avg',
    getValue: (strategy) => strategy?.comparison?.summary?.avg_return ?? Number.NEGATIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'desc',
  },
  closed: {
    label: 'Closed',
    getValue: (strategy) => strategy?.comparison?.summary?.closed_count ?? Number.NEGATIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'desc',
  },
  activeWeeks: {
    label: 'Act Wks',
    getValue: (strategy) => strategy?.weeklyCadence?.activeWeeks ?? Number.NEGATIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'desc',
  },
  avgSetupsPerWeek: {
    label: 'Avg/Wk',
    getValue: (strategy) => strategy?.weeklyCadence?.avgSetupsPerWeek ?? Number.NEGATIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'desc',
  },
  maxSetupsPerWeek: {
    label: 'Max/Wk',
    getValue: (strategy) => strategy?.weeklyCadence?.maxSetupsPerWeek ?? Number.NEGATIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'desc',
  },
  zeroWeekRate: {
    label: 'Zero Wk',
    getValue: (strategy) => strategy?.weeklyCadence?.zeroSetupWeekRate ?? Number.POSITIVE_INFINITY,
    isNumeric: true,
    defaultDirection: 'asc',
  },
};

const compareValues = (leftValue, rightValue, isNumeric, direction) => {
  const modifier = direction === 'asc' ? 1 : -1;

  if (isNumeric) {
    return ((Number(leftValue) || 0) - (Number(rightValue) || 0)) * modifier;
  }

  return String(leftValue).localeCompare(String(rightValue)) * modifier;
};

export const rankStrategies = (strategies = [], sortState = { key: 'closed', direction: 'desc' }) => {
  const sortMeta = SORT_COLUMNS[sortState.key] ?? SORT_COLUMNS.expectancy;

  return strategies
    .map((strategy, originalIndex) => ({ strategy, originalIndex }))
    .sort((left, right) => {
      const primaryComparison = compareValues(
        sortMeta.getValue(left.strategy, left.originalIndex),
        sortMeta.getValue(right.strategy, right.originalIndex),
        sortMeta.isNumeric,
        sortState.direction
      );

      if (primaryComparison !== 0) {
        return primaryComparison;
      }

      return (
        (right.strategy?.comparison?.summary?.closed_count ?? 0) -
        (left.strategy?.comparison?.summary?.closed_count ?? 0)
      );
    })
    .map((entry) => entry.strategy);
};

const StrategyLeaderboardCard = ({
  strategies = [],
  selectedStrategyId = '',
  hoveredStrategyId = '',
  isLoading = false,
  sortState: controlledSortState = null,
  onSortChange,
  onSelectStrategy,
  onVisibleRangeChange,
  filtersContent = null,
  bottomContent = null,
}) => {
  const rowRefs = useRef(new Map());
  const tableShellRef = useRef(null);
  const [localSortState, setLocalSortState] = useState({
    key: 'closed',
    direction: 'desc',
  });
  const sortState = controlledSortState ?? localSortState;

  const rankedStrategies = useMemo(() => {
    return rankStrategies(strategies, sortState);
  }, [sortState, strategies]);

  const reportVisibleRange = useCallback(() => {
    const shell = tableShellRef.current;
    if (!shell || !rankedStrategies.length) {
      onVisibleRangeChange?.({ startIndex: 0, endIndex: 0 });
      return;
    }

    const firstRow = shell.querySelector('tbody tr');
    const rowHeight = firstRow?.getBoundingClientRect().height || 28;
    const startIndex = Math.max(0, Math.floor(shell.scrollTop / rowHeight));
    const visibleCount = Math.max(1, Math.ceil(shell.clientHeight / rowHeight));
    const endIndex = Math.min(rankedStrategies.length - 1, startIndex + visibleCount - 1);

    onVisibleRangeChange?.({ startIndex, endIndex });
  }, [onVisibleRangeChange, rankedStrategies.length]);

  useEffect(() => {
    reportVisibleRange();
  }, [reportVisibleRange, sortState]);

  useEffect(() => {
    if (!selectedStrategyId) {
      return;
    }

    rowRefs.current.get(selectedStrategyId)?.scrollIntoView({
      block: 'nearest',
      inline: 'nearest',
    });
  }, [rankedStrategies, selectedStrategyId]);

  const handleSelectStrategyAtIndex = (rowIndex) => {
    const strategy = rankedStrategies[rowIndex];

    if (!strategy) {
      return;
    }

    onSelectStrategy?.(strategy.id);
    requestAnimationFrame(() => {
      rowRefs.current.get(strategy.id)?.focus();
      rowRefs.current.get(strategy.id)?.scrollIntoView({
        block: 'nearest',
        inline: 'nearest',
      });
    });
  };

  const handleSortColumn = (nextKey) => {
    const updateSortState = (current) => {
      const nextMeta = SORT_COLUMNS[nextKey] ?? SORT_COLUMNS.expectancy;

      if (current.key !== nextKey) {
        return {
          key: nextKey,
          direction: nextMeta.defaultDirection,
        };
      }

      return {
        key: nextKey,
        direction: current.direction === 'asc' ? 'desc' : 'asc',
      };
    };

    if (onSortChange) {
      onSortChange(updateSortState(sortState));
      return;
    }

    setLocalSortState(updateSortState);
  };

  const renderHeader = (key) => {
    const column = SORT_COLUMNS[key];
    const isActive = sortState.key === key;
    const indicator = isActive ? (sortState.direction === 'asc' ? ' ▲' : ' ▼') : '';

    return (
      <th key={key}>
        <button
          type="button"
          className="strategy-sort-header-button"
          onClick={() => handleSortColumn(key)}
        >
          {column.label}
          {indicator}
        </button>
      </th>
    );
  };

  return (
    <DashboardCardFrame
      title="Library"
      subtitle="Ranked strategy cohorts"
      controls={<span className="dashboard-card-label">Universe</span>}
      bodyClassName="strategy-leaderboard-body"
    >
      <div className="strategy-leaderboard-list">
        {filtersContent ? (
          <div className="strategy-library-toolbar">{filtersContent}</div>
        ) : null}

        <div className="strategy-library-split">
          <div className="strategy-library-pane">
            {isLoading && !rankedStrategies.length ? (
              <div className="strategy-empty">Loading strategy cohorts...</div>
            ) : null}

            {!rankedStrategies.length && !isLoading ? (
              <div className="strategy-empty">No strategies are loaded yet.</div>
            ) : null}

            {rankedStrategies.length ? (
              <div
                className="strategy-library-table-shell"
                ref={tableShellRef}
                onScroll={reportVisibleRange}
              >
                <table className="strategy-library-table">
                  <thead>
                    <tr>
                      {[
                        'rank',
                        'market',
                        'pattern',
                        'bin',
                        'reversal',
                        'size',
                        'time',
                        'xMode',
                        'worstYear',
                        'downYears',
                        'score',
                        'expectancy',
                        'winRate',
                        'avgReturn',
                        'closed',
                        'activeWeeks',
                        'avgSetupsPerWeek',
                        'maxSetupsPerWeek',
                        'zeroWeekRate',
                      ].map(renderHeader)}
                    </tr>
                  </thead>
                  <tbody>
                    {rankedStrategies.map((strategy, index) => {
                      const summary = strategy?.comparison?.summary;
                      const isSelected = strategy.id === selectedStrategyId;
                      const isHovered = strategy.id === hoveredStrategyId;
                      const marketTone = String(strategy.market ?? '').toLowerCase();

                      return (
                        <tr
                          key={strategy.id}
                          ref={(node) => {
                            if (node) {
                              rowRefs.current.set(strategy.id, node);
                            } else {
                              rowRefs.current.delete(strategy.id);
                            }
                          }}
                          className={[
                            'strategy-library-row',
                            'strategy-family-row',
                            marketTone === 'bullish' ? 'strategy-family-row--bullish' : '',
                            marketTone === 'bearish' ? 'strategy-family-row--bearish' : '',
                            isHovered ? 'strategy-library-row--hovered' : '',
                            isSelected ? 'strategy-library-row--selected' : '',
                          ]
                            .filter(Boolean)
                            .join(' ')}
                          onClick={() => handleSelectStrategyAtIndex(index)}
                          onKeyDown={(event) => {
                            if (event.key === 'Enter' || event.key === ' ') {
                              event.preventDefault();
                              handleSelectStrategyAtIndex(index);
                            }

                            if (event.key === 'ArrowDown') {
                              event.preventDefault();
                              handleSelectStrategyAtIndex(
                                Math.min(index + 1, rankedStrategies.length - 1)
                              );
                            }

                            if (event.key === 'ArrowUp') {
                              event.preventDefault();
                              handleSelectStrategyAtIndex(Math.max(index - 1, 0));
                            }
                          }}
                          tabIndex={0}
                        >
                          <td>{index + 1}</td>
                          <td>{strategy.market}</td>
                          <td>{strategy.harmonicType}</td>
                          <td>{strategy.bin}</td>
                          <td>{strategy.reversalType ?? 'None'}</td>
                          <td>{strategy.sizeBucket}</td>
                          <td>{strategy.timeBin}</td>
                          <td>{strategy.xStrictness ?? 'N/A'}</td>
                          <td>{formatPercent(strategy.worstYearExpectancy)}</td>
                          <td>{strategy.downYears ?? 0}</td>
                          <td>{formatPercent(strategy.score)}</td>
                          <td>{formatPercent(summary?.expectancy)}</td>
                          <td>
                            {Number.isFinite(summary?.win_rate)
                              ? formatPercent(summary.win_rate * 100)
                              : 'N/A'}
                          </td>
                          <td>{formatPercent(summary?.avg_return)}</td>
                          <td>{summary?.closed_count ?? 0}/{summary?.total_count ?? 0}</td>
                          <td>
                            {strategy.weeklyCadence?.activeWeeks ?? 0}/
                            {strategy.weeklyCadence?.totalCalendarWeeks ?? 0}
                          </td>
                          <td>{(strategy.weeklyCadence?.avgSetupsPerWeek ?? 0).toFixed(2)}</td>
                          <td>{strategy.weeklyCadence?.maxSetupsPerWeek ?? 0}</td>
                          <td>{formatPercent((strategy.weeklyCadence?.zeroSetupWeekRate ?? 0) * 100)}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            ) : null}
          </div>

          <div className="strategy-library-pane">
            {bottomContent ? <div className="strategy-library-bottom">{bottomContent}</div> : null}
          </div>
        </div>
      </div>
    </DashboardCardFrame>
  );
};

export default StrategyLeaderboardCard;
