import React, { useState } from "react";
import {
  Chart as ChartJS,
  BarElement,
  CategoryScale,
  LinearScale,
  Tooltip,
  Legend
} from "chart.js";
import { Bar } from "react-chartjs-2";
import * as route from './backend_routes.js';

ChartJS.register(BarElement, CategoryScale, LinearScale, Tooltip, Legend);

// 🔥 Helper to get correct value
const getValue = (d, metric) => {
  if (metric === "avg_return") return (d.avg_return || 0) * 100;
  if (metric === "win_rate") return (d.win_rate || 0) * 100;
  if (metric === "count") return d.count || 0;
  return 0;
};

const buildChartData = (data = [], metric) => {
  const bins = [...new Set(data.map(d => d.bin))];

  const grouped = {};
  data.forEach(d => {
    if (!grouped[d.harmonic_type]) grouped[d.harmonic_type] = {};
    grouped[d.harmonic_type][d.bin] = d;
  });

  const colors = [
    "rgb(99, 179, 237)",
    "rgb(250, 115, 0)",
    "rgb(16, 185, 129)",
    "rgb(239, 68, 68)",
    "rgb(250, 204, 21)"
  ];

  const datasets = Object.keys(grouped).map((type, i) => ({
    label: type,
    data: bins.map(bin => {
      const d = grouped[type][bin];
      return d ? getValue(d, metric) : 0;
    }),
    counts: bins.map(bin => grouped[type][bin]?.count || 0),
    rawData: bins.map(bin => grouped[type][bin] || null),
    backgroundColor: colors[i % colors.length],
  }));

  return { labels: bins, datasets };
};

const Accuracy = ({ accuracy = [], set_filtered_patterns }) => {

  const [metric, setMetric] = useState("avg_return");

  const chartData = buildChartData(accuracy, metric);

  const options = {
    responsive: true,

    onClick: async (event, elements, chart) => {
      if (!elements.length) return;

      const element = elements[0];
      const datasetIndex = element.datasetIndex;
      const index = element.index;

      const dataset = chart.data.datasets[datasetIndex];
      const bin = chart.data.labels[index];
      const harmonicType = dataset.label;

      const xabcd_patterns = await route.fetch_abcd_patterns('binance', { 
        bin, 
        harmonicType 
      });

      set_filtered_patterns(xabcd_patterns.patterns);
    },

    plugins: {
      legend: {
        position: "top",
        labels: { color: "white" },
      },
      tooltip: {
        backgroundColor: "rgb(40, 42, 54)",
        titleColor: "white",
        bodyColor: "white",
        callbacks: {
          label: function (context) {
            const d = context.dataset.rawData[context.dataIndex];
            if (!d) return "";

            return [
              `${d.harmonic_type} — ${d.bin}`,
              `Avg Return: ${(d.avg_return * 100).toFixed(2)}%`,
              `Win %: ${d.win_rate ? (d.win_rate * 100).toFixed(2) : "N/A"}%`,
              `Count: ${d.count}`
            ];
          },
        },
      },
    },

    scales: {
      x: {
        ticks: { color: "white" },
        grid: { color: "rgba(255,255,255,0.1)" },
        title: { display: true, text: "Accuracy Bins", color: "white" },
      },
      y: {
        beginAtZero: true,
        ticks: { color: "white" },
        grid: { color: "rgba(255,255,255,0.1)" },
        title: {
          display: true,
          text:
            metric === "avg_return"
              ? "Average Return (%)"
              : metric === "win_rate"
              ? "Win %"
              : "Trade Count",
          color: "white"
        },
      },
    },
  };

  return (
    <div
      className="accuracy"
      style={{
        backgroundColor: "rgb(21, 22, 26)",
        borderRadius: "8px",
        padding: "10px"
      }}
    >

      {/* 🔥 Toggle Buttons */}
      <div style={{ marginBottom: "10px" }}>
        <button onClick={() => setMetric("avg_return")}>Avg Return</button>
        <button onClick={() => setMetric("win_rate")}>Win %</button>
        <button onClick={() => setMetric("count")}>Count</button>
      </div>

      <Bar data={chartData} options={options} />
    </div>
  );
};

export default Accuracy;