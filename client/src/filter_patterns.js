// src/hooks/usePatternFilter.js
export const usePatternFilter = (patterns) => {

  const filterByMarket = (market) =>
    patterns.filter(p => p.market === market);

  const filterBySymbol = (symbol) =>
    patterns.filter(p => p.symbol === symbol);

  const filterByRetracement = (min, max, key) =>
    patterns.filter(p => p[key] >= min && p[key] <= max);

  return {
    filterByMarket,
    filterBySymbol,
    filterByRetracement
  };
};