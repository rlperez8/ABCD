import React, { useMemo, useState } from 'react';
import DashboardCardFrame from '../dashboard/DashboardCardFrame';

const formatPercent = (value) =>
  Number.isFinite(value) ? `${value.toFixed(2)}%` : 'N/A';

const getTradePnl = (trade = {}) => {
  const entry = Number(trade.trade_enter_price ?? 0);
  const exit = Number(trade.target_close ?? trade.target_open ?? trade.trade_current_price ?? 0);

  if (!Number.isFinite(entry) || !Number.isFinite(exit) || entry <= 0 || exit <= 0) {
    return 0;
  }

  return trade.market === 'Bearish' ? entry - exit : exit - entry;
};

export default function StrategyContractBreakdownPanel({
  selectedStrategy = null,
  loadedTrades = [],
  totalTradeCount = 0,
}) {
  const [isOpen, setIsOpen] = useState(false);

  const rows = useMemo(() => {
    const bySymbol = new Map();

    loadedTrades.forEach((trade) => {
      const symbol = trade.symbol ?? 'Unknown';
      const current = bySymbol.get(symbol) ?? {
        symbol,
        total: 0,
        closed: 0,
        wins: 0,
        losses: 0,
        pnl: 0,
      };

      current.total += 1;
      if (trade.trade_result === 1 || trade.trade_result === 2) current.closed += 1;
      if (trade.trade_result === 1) current.wins += 1;
      if (trade.trade_result === 2) current.losses += 1;
      current.pnl += getTradePnl(trade);
      bySymbol.set(symbol, current);
    });

    return [...bySymbol.values()].sort((left, right) => {
      if (right.closed !== left.closed) return right.closed - left.closed;
      return left.symbol.localeCompare(right.symbol);
    });
  }, [loadedTrades]);

  return (
    <DashboardCardFrame
      title="Contract Breakdown"
      subtitle="Loaded family trades by contract"
      controls={
        <>
          <span className="dashboard-card-label">Contracts</span>
          <button
            type="button"
            className="dashboard-card-collapse-button"
            aria-expanded={isOpen}
            onClick={() => setIsOpen((current) => !current)}
          >
            {isOpen ? '-' : '+'}
          </button>
        </>
      }
      bodyClassName="strategy-library-card-body"
      isCollapsed={!isOpen}
    >
      <div className="strategy-contract-breakdown">
        {!selectedStrategy ? (
          <div className="strategy-empty-row">Choose a family to inspect its contracts.</div>
        ) : (
          <>
            <div className="strategy-contract-breakdown__meta">
              Showing {loadedTrades.length} loaded trades from {totalTradeCount} total family trades.
            </div>

            {rows.length ? (
              <div className="strategy-contract-breakdown__table-shell">
                <table className="strategy-contract-breakdown__table">
                  <thead>
                    <tr>
                      <th>Contract</th>
                      <th>Total</th>
                      <th>Closed</th>
                      <th>Win %</th>
                      <th>PnL</th>
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((row) => (
                      <tr key={row.symbol}>
                        <td>{row.symbol}</td>
                        <td>{row.total}</td>
                        <td>{row.closed}</td>
                        <td>{formatPercent(row.closed ? (row.wins / row.closed) * 100 : 0)}</td>
                        <td>{row.pnl.toFixed(2)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="strategy-empty-row">No loaded trades to break down yet.</div>
            )}
          </>
        )}
      </div>
    </DashboardCardFrame>
  );
}
