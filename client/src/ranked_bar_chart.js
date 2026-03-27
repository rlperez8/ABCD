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

ChartJS.register(BarElement, CategoryScale, LinearScale, Tooltip, Legend);

const buildRankedData = (data, metric) => {
  const mapped = data.map(d => ({
    label: `${d.harmonic_type} | ${d.bin}`,
    value:
      metric === "avg_return"
        ? (d.avg_return || 0) * 100
        : metric === "win_rate"
        ? (d.win_rate || 0) * 100
        : d.count || 0,
    raw: d
  }));

  // sort descending
  mapped.sort((a, b) => b.value - a.value);

  return {
    labels: mapped.map(d => d.label),
    datasets: [
      {
        label: metric,
        data: mapped.map(d => d.value),
        backgroundColor: "rgb(99, 179, 237)",
      }
    ],
    rawData: mapped
  };
};

const RankedChart = ({ accuracy = [] }) => {
  const [metric, setMetric] = useState("avg_return");

  console.log(accuracy)
  

  const chartData = buildRankedData(accuracy, metric);

  const options = {
    indexAxis: "y", // horizontal bars
    responsive: true,
    plugins: {
      legend: {
        display: false
      },
      tooltip: {
        backgroundColor: "rgb(40, 42, 54)",
        titleColor: "white",
        bodyColor: "white",
        callbacks: {
          label: function (context) {
            const d = chartData.rawData[context.dataIndex].raw;

            return [
              `Avg Return: ${(d.avg_return * 100).toFixed(2)}%`,
              `Win %: ${(d.win_rate * 100).toFixed(2)}%`,
              `Count: ${d.count}`
            ];
          }
        }
      }
    },
    scales: {
      x: {
        ticks: { color: "white" },
        grid: { color: "rgba(255,255,255,0.1)" },
        title: {
          display: true,
          text:
            metric === "avg_return"
              ? "Avg Return (%)"
              : metric === "win_rate"
              ? "Win %"
              : "Trade Count",
          color: "white"
        }
      },
      y: {
        ticks: { color: "white" },
        grid: { display: false }
      }
    }
  };

  return (
    <div className="accuracy" style={{ background: "rgb(21,22,26)", borderRadius: "8px", padding: "10px" }}>
      
      {/* 🔹 Toggle */}
      <div style={{ marginBottom: "10px" }}>
        <button onClick={() => setMetric("avg_return")}>Avg Return</button>
        <button onClick={() => setMetric("win_rate")}>Win %</button>
        <button onClick={() => setMetric("count")}>Count</button>
      </div>

      <Bar data={chartData} options={options} />
    </div>
  );
};

export default RankedChart;