import React from 'react';
import DashboardCardFrame from '../dashboard/DashboardCardFrame';

const FeatureGroup = ({
  title,
  options = [],
  selectedOptions = [],
  onToggleOption,
  onSelectAll,
  onClearAll,
}) => (
  <div className="strategy-pool-group">
    <div className="strategy-pool-group-header">
      <div className="strategy-pool-group-title">{title}</div>
      <div className="strategy-pool-group-actions">
        <button type="button" className="strategy-pool-action" onClick={onSelectAll}>
          All
        </button>
        <button type="button" className="strategy-pool-action" onClick={onClearAll}>
          None
        </button>
      </div>
    </div>

    <div className="strategy-pool-options">
      {options.map((option) => {
        const isSelected = selectedOptions.includes(option);

        return (
          <label
            key={option}
            className={[
              'strategy-pool-option',
              isSelected ? 'strategy-pool-option--selected' : '',
            ]
              .filter(Boolean)
              .join(' ')}
          >
            <input
              type="checkbox"
              checked={isSelected}
              onChange={() => onToggleOption?.(option)}
            />
            <span>{option}</span>
          </label>
        );
      })}
    </div>
  </div>
);

const StrategyVariationPoolCard = ({
  featureGroups = [],
  strategyCount = 0,
  totalStrategyCount = 0,
  embedded = false,
  defaultFitActive = false,
  onToggleDefaultFit,
}) => {
  const content = (
    <div className="strategy-variation-pool">
      <div className="strategy-pool-preset-bar">
        <button
          type="button"
          className={[
            'strategy-pool-preset-button',
            defaultFitActive ? 'strategy-pool-preset-button--active' : '',
          ]
            .filter(Boolean)
            .join(' ')}
          aria-pressed={defaultFitActive}
          onClick={onToggleDefaultFit}
        >
          Default Fit
        </button>
      </div>

      <div className="strategy-variation-pool-layout">
        {featureGroups.map((group) => (
          <FeatureGroup
            key={group.title}
            title={group.title}
            options={group.options}
            selectedOptions={group.selectedOptions}
            onToggleOption={group.onToggleOption}
            onSelectAll={group.onSelectAll}
            onClearAll={group.onClearAll}
          />
        ))}
      </div>

      <div className="strategy-pool-summary">
        <span className="strategies-summary-pill">
          {strategyCount} / {totalStrategyCount} cohorts available
        </span>
      </div>
    </div>
  );

  if (embedded) {
    return <div className="strategy-variation-pool-embedded">{content}</div>;
  }

  return (
    <DashboardCardFrame
      title="Variation Pool"
      subtitle="Choose which strategy features are allowed in the cohort list."
      controls={<span className="dashboard-card-label">Curated</span>}
      bodyClassName="strategy-variation-pool-body"
    >
      {content}
    </DashboardCardFrame>
  );
};

export default StrategyVariationPoolCard;
