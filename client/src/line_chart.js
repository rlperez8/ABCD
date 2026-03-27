// BinLineChart.js
import React, { useState } from "react";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Tooltip,
  Legend,
} from "chart.js";
import { Line } from "react-chartjs-2";

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Tooltip, Legend);

// Helper to pick value based on metric
const getValue = (d, metric) => {
  if (metric === "avg_return") return d.avg_return ?? 0;
  if (metric === "win_rate") return d.win_rate ?? 0;
  if (metric === "count") return d.count ?? 0;
  return 0;
};

export default function BinLineChart({ data = [] }) {
  const bins = [...new Set(data.map(d => d.bin))];
  const types = [...new Set(data.map(d => d.harmonic_type))];

  const [metric, setMetric] = useState("avg_return");

  const chartData = {
    labels: bins,
    datasets: types.map((type, idx) => ({
      label: type,
      data: bins.map(bin => {
        const d = data.find(d => d.bin === bin && d.harmonic_type === type);
        return d ? getValue(d, metric) : 0;
      }),
      borderColor: `hsl(${(idx * 60) % 360}, 70%, 60%)`,
      backgroundColor: `hsla(${(idx * 60) % 360}, 70%, 60%, 0.2)`,
      tension: 0.3,
      fill: true,
      pointRadius: 5,
    })),
  };

  const chartOptions = {
    responsive: true,
    plugins: {
      legend: { position: "top", labels: { color: "white" } },
      tooltip: {
        mode: "index",
        intersect: false,
        backgroundColor: "rgb(40,42,54)",
        titleColor: "white",
        bodyColor: "white",
        callbacks: {
          label: (ctx) => {
            const d = data.find(
              d => d.bin === ctx.label && d.harmonic_type === ctx.dataset.label
            );
            if (!d) return "";
            return [
              `Avg Return: ${(d.avg_return * 100).toFixed(2)}%`,
              `Win %: ${d.win_rate ? (d.win_rate * 100).toFixed(2) : "N/A"}%`,
              `Count: ${d.count}`
            ];
          }
        }
      }
    },
    scales: {
      y: {
        beginAtZero: true,
        ticks: { color: "white" },
        grid: { color: "rgba(255,255,255,0.1)" },
        title: {
          display: true,
          text: metric === "avg_return" ? "Average Return" : metric === "win_rate" ? "Win %" : "Count",
          color: "white"
        },
      },
      x: {
        ticks: { color: "white" },
        grid: { color: "rgba(255,255,255,0.1)" },
        title: { display: true, text: "Bins", color: "white" },
      }
    },
  };

  return (
    <div className='accuracy' style={{ backgroundColor: "rgb(21,22,26)", padding: "10px", borderRadius: "8px" }}>
      <div style={{ marginBottom: "10px" }}>
        <button onClick={() => setMetric("avg_return")}>Avg Return</button>
        <button onClick={() => setMetric("win_rate")}>Win %</button>
        <button onClick={() => setMetric("count")}>Count</button>
      </div>
      <Line data={chartData} options={chartOptions} />
    </div>
  );
}