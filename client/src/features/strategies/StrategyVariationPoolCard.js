import React from 'react';
import DashboardCardFrame from '../dashboard/DashboardCardFrame';

const FeatureGroup = ({
  title,
  options = [],
  selectedOptions = [],
  onToggleOption,
}) => {
  const isFiltered = selectedOptions.length !== options.length;
  const previewOptions = selectedOptions.slice(0, 3);
  const hiddenSelectionCount = Math.max(selectedOptions.length - previewOptions.length, 0);

  return (
    <div
      className={[
        'strategy-pool-group',
        'strategy-pool-group--open',
        isFiltered ? 'strategy-pool-group--filtered' : '',
      ].filter(Boolean).join(' ')}
    >
      <div className="strategy-pool-group-header">
        <div className="strategy-pool-group-toggle">
          <span className="strategy-pool-group-title">{title}</span>
          <span className="strategy-pool-group-count">
            {selectedOptions.length}/{options.length}
          </span>
        </div>
      </div>

      <div className="strategy-pool-selected-strip" aria-label={`${title} active filters`}>
        {previewOptions.map((option) => (
          <span className="strategy-pool-selected-chip" key={option}>
            {option}
          </span>
        ))}
        {hiddenSelectionCount ? (
          <span className="strategy-pool-selected-chip strategy-pool-selected-chip--more">
            +{hiddenSelectionCount}
          </span>
        ) : null}
      </div>

      <div className="strategy-pool-options">
        {options.map((option) => {
          const isSelected = selectedOptions.includes(option);

          return (
            <button
              type="button"
              key={option}
              className={[
                'strategy-pool-option',
                isSelected ? 'strategy-pool-option--selected' : '',
              ]
                .filter(Boolean)
                .join(' ')}
              aria-pressed={isSelected}
              onClick={() => onToggleOption?.(option)}
            >
              <span className="strategy-pool-option-mark" aria-hidden="true" />
              <span>{option}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
};

const StrategyVariationPoolCard = ({
  featureGroups = [],
  strategyCount = 0,
  totalStrategyCount = 0,
  embedded = false,
  defaultFitActive = false,
  onToggleDefaultFit,
}) => {
  const activeGroupCount = featureGroups.filter(
    (group) => group.selectedOptions.length !== group.options.length
  ).length;

  const content = (
    <div className="strategy-variation-pool">
      <div className="strategy-pool-menu-header">
        <div className="strategy-pool-menu-toggle">
          <span>Family Filter</span>
          <span className="strategy-pool-menu-count">{activeGroupCount} active</span>
        </div>
      </div>

      <div className="strategy-pool-summary-card">
        <div>
          <span>Families Shown</span>
          <strong>{strategyCount} / {totalStrategyCount}</strong>
        </div>
        <div>
          <span>Filtered Groups</span>
          <strong>{activeGroupCount}</strong>
        </div>
      </div>

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
          />
        ))}
      </div>

      <div className="strategy-pool-summary">
        <span className="strategies-summary-pill">
          {strategyCount} / {totalStrategyCount} families
        </span>
      </div>
    </div>
  );

  if (embedded) {
    return <div className="strategy-variation-pool-embedded">{content}</div>;
  }

  return (
    <DashboardCardFrame
      title="Family Filter"
      subtitle="Feature controls"
      controls={<span className="dashboard-card-label">Curated</span>}
      bodyClassName="strategy-variation-pool-body"
    >
      {content}
    </DashboardCardFrame>
  );
};

export default StrategyVariationPoolCard;
