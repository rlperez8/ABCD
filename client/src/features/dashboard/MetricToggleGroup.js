import React from 'react';
import { getMetricOptions } from './dashboardMetrics';

const MetricToggleGroup = ({ activeMetric, onChange, metrics }) => (
  <>
    {getMetricOptions(metrics).map(({ metric, label }) => (
      <button
        key={metric}
        className={activeMetric === metric ? 'metric-button is-active' : 'metric-button'}
        onClick={() => onChange(metric)}
        type="button"
      >
        {label}
      </button>
    ))}
  </>
);

export default MetricToggleGroup;
