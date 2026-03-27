import React, { useState } from "react";

// 🔥 value selector
const getValue = (d, metric) => {
  if (metric === "avg_return") return d.avg_return ?? null;
  if (metric === "win_rate") return d.win_rate ?? null;
  if (metric === "count") return d.count ?? null;
  return null;
};

const getColor = (value, metric, data) => {
  if (value == null) return "rgb(40,42,54)";

  // COUNT → blue scale
  if (metric === "count") {
    const max = Math.max(...data.map(d => d.count || 0), 1);
    const intensity = value / max;
    return `rgba(99,179,237,${intensity})`;
  }

  // RETURN / WIN RATE → red/green
  const percent = value * 100;
  const max = 10;
  const min = -10;

  const normalized = Math.max(min, Math.min(max, percent)) / max;

  if (normalized > 0) {
    return `rgba(16,185,129,${Math.abs(normalized)})`;
  } else {
    return `rgba(239,68,68,${Math.abs(normalized)})`;
  }
};

const Heatmap = ({ data = [], set_filtered_patterns }) => {
  const [metric, setMetric] = useState("avg_return");

  if (!data.length) return <div style={{ color: "white" }}>No data</div>;

  const bins = [...new Set(data.map(d => d.bin))];
  const types = [...new Set(data.map(d => d.harmonic_type))];

  const lookup = {};
  data.forEach(d => {
    if (!lookup[d.harmonic_type]) lookup[d.harmonic_type] = {};
    lookup[d.harmonic_type][d.bin] = d;
  });

  return (
    <div className="accuracy" style={{ background: "rgb(21,22,26)", padding: "10px", borderRadius: "8px" }}>

      {/* 🔥 Toggle */}
      <div style={{ marginBottom: "10px" }}>
        <button onClick={() => setMetric("avg_return")}>Avg Return</button>
        <button onClick={() => setMetric("win_rate")}>Win %</button>
        <button onClick={() => setMetric("count")}>Count</button>
      </div>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: `150px repeat(${bins.length}, minmax(60px, 1fr))`,
          gap: "4px",
          color: "white",
          height: "100%",
          width: "100%"
        }}
      >
        {/* Header */}
        <div></div>
        {bins.map(bin => (
          <div key={bin} style={{ textAlign: "center", fontSize: "12px" }}>
            {bin}
          </div>
        ))}

        {/* Rows */}
        {types.map(type => (
          <React.Fragment key={type}>
            <div style={{ fontWeight: "bold" }}>{type}</div>

            {bins.map(bin => {
              const cell = lookup[type]?.[bin];
              const value = cell ? getValue(cell, metric) : null;

              return (
                <div
                  key={bin}
                  onClick={() => {
                    if (!cell) return;
                    set_filtered_patterns?.({
                      bin,
                      harmonicType: type
                    });
                  }}
                  style={{
                    backgroundColor: getColor(value, metric, data),
                    padding: "10px",
                    textAlign: "center",
                    borderRadius: "4px",
                    cursor: cell ? "pointer" : "default",
                    fontSize: "12px"
                  }}
                  title={
                    cell
                      ? `${type} | ${bin}
Avg: ${(cell.avg_return * 100).toFixed(2)}%
Win %: ${cell.win_rate ? (cell.win_rate * 100).toFixed(2) : "N/A"}%
Count: ${cell.count}`
                      : ""
                  }
                >
                  {metric === "count"
                    ? value ?? "-"
                    : value != null
                    ? `${(value * 100).toFixed(1)}%`
                    : "-"}
                </div>
              );
            })}
          </React.Fragment>
        ))}
      </div>
    </div>
  );
};

export default Heatmap;