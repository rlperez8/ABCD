// BinCumulativeLineChart.js
import React from "react";
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

export default function BinCumulativeLineChart({ data = [] }) {
  const bins = [...new Set(data.map(d => d.bin))];
  const types = [...new Set(data.map(d => d.harmonic_type))];

  // Helper
  const getValue = (d) => d.avg_return ?? 0;

  const chartData = {
    labels: bins,
    datasets: types.map((type, idx) => {
      let cumulative = 0;

      const values = bins.map(bin => {
        const d = data.find(
          d => d.bin === bin && d.harmonic_type === type
        );

        const val = d ? getValue(d) : 0;
        cumulative += val;
        return cumulative;
      });

      return {
        label: type,
        data: values,
        borderColor: `hsl(${(idx * 60) % 360}, 70%, 60%)`,
        backgroundColor: `hsla(${(idx * 60) % 360}, 70%, 60%, 0.2)`,
        tension: 0.3,
        fill: true,
        pointRadius: 4,
      };
    }),
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
            return `Cumulative Return: ${(ctx.raw * 100).toFixed(2)}%`;
          },
        },
      },
    },
    scales: {
      y: {
        beginAtZero: true,
        ticks: {
          color: "white",
          callback: (val) => `${(val * 100).toFixed(0)}%`,
        },
        grid: { color: "rgba(255,255,255,0.1)" },
        title: {
          display: true,
          text: "Cumulative Return",
          color: "white",
        },
      },
      x: {
        ticks: { color: "white" },
        grid: { color: "rgba(255,255,255,0.1)" },
        title: {
          display: true,
          text: "Bins",
          color: "white",
        },
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
      <Line data={chartData} options={chartOptions} />
    </div>
  );
}