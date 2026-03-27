// BinScatterChart.js
import React, { useState } from "react";
import {
  Chart as ChartJS,
  LinearScale,
  PointElement,
  Tooltip,
  Legend,
} from "chart.js";
import { Scatter } from "react-chartjs-2";

ChartJS.register(LinearScale, PointElement, Tooltip, Legend);

export default function BinScatterChart({ data = [] }) {
  const types = [...new Set(data.map(d => d.harmonic_type))];

  const [metricX, setMetricX] = useState("avg_return");
  const [metricY, setMetricY] = useState("win_rate");

  const getValue = (d, metric) => {
    if (metric === "avg_return") return d.avg_return ?? 0;
    if (metric === "win_rate") return d.win_rate ?? 0;
    if (metric === "count") return d.count ?? 0;
    return 0;
  };

  const chartData = {
    datasets: types.map((type, idx) => ({
      label: type,
      data: data
        .filter(d => d.harmonic_type === type)
        .map(d => ({
          x: getValue(d, metricX),
          y: getValue(d, metricY),
          bin: d.bin,
          avg_return: d.avg_return,
          win_rate: d.win_rate,
          count: d.count,
        })),
      backgroundColor: `hsl(${(idx * 60) % 360}, 70%, 60%)`,
      pointRadius: 6,
    })),
  };

  const chartOptions = {
    responsive: true,
    plugins: {
      legend: { position: "top", labels: { color: "white" } },
      tooltip: {
        backgroundColor: "rgb(40,42,54)",
        titleColor: "white",
        bodyColor: "white",
        callbacks: {
          label: (ctx) => {
            const d = ctx.raw;
            return [
              `Bin: ${d.bin}`,
              `Avg Return: ${(d.avg_return * 100).toFixed(2)}%`,
              `Win %: ${d.win_rate ? (d.win_rate * 100).toFixed(2) : "N/A"}%`,
              `Count: ${d.count}`,
            ];
          },
        },
      },
    },
    scales: {
      x: {
        title: {
          display: true,
          text:
            metricX === "avg_return"
              ? "Average Return"
              : metricX === "win_rate"
              ? "Win %"
              : "Count",
          color: "white",
        },
        ticks: { color: "white" },
        grid: { color: "rgba(255,255,255,0.1)" },
      },
      y: {
        title: {
          display: true,
          text:
            metricY === "avg_return"
              ? "Average Return"
              : metricY === "win_rate"
              ? "Win %"
              : "Count",
          color: "white",
        },
        ticks: { color: "white" },
        grid: { color: "rgba(255,255,255,0.1)" },
      },
    },
  };

  return (
    <div
      className="accuracy"
      style={{
        backgroundColor: "rgb(21,22,26)",
        padding: "10px",
        borderRadius: "8px",
      }}
    >
      <div style={{ marginBottom: "10px" }}>
        <span style={{ color: "white", marginRight: "10px" }}>X:</span>
        <button onClick={() => setMetricX("avg_return")}>Avg Return</button>
        <button onClick={() => setMetricX("win_rate")}>Win %</button>
        <button onClick={() => setMetricX("count")}>Count</button>

        <span style={{ color: "white", margin: "0 10px" }}>Y:</span>
        <button onClick={() => setMetricY("avg_return")}>Avg Return</button>
        <button onClick={() => setMetricY("win_rate")}>Win %</button>
        <button onClick={() => setMetricY("count")}>Count</button>
      </div>

      <Scatter data={chartData} options={chartOptions} />
    </div>
  );
}