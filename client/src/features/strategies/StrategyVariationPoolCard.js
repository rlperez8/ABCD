import React, { useState } from 'react';
import DashboardCardFrame from '../dashboard/DashboardCardFrame';

const FeatureGroup = ({
  title,
  options = [],
  selectedOptions = [],
  onToggleOption,
}) => {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="strategy-pool-group">
      <div className="strategy-pool-group-header">
        <button
          type="button"
          className="strategy-pool-group-toggle"
          aria-expanded={isOpen}
          onClick={() => setIsOpen((current) => !current)}
        >
          <span className="strategy-pool-group-title">{title}</span>
          <span className="strategy-pool-group-count">
            {selectedOptions.length}/{options.length}
          </span>
          <span className="strategy-pool-group-icon">{isOpen ? '-' : '+'}</span>
        </button>
      </div>

      {isOpen ? (
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
                <span>{option}</span>
              </button>
            );
          })}
        </div>
      ) : null}
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
  const [isMenuOpen, setIsMenuOpen] = useState(true);

  const content = (
    <div className="strategy-variation-pool">
      <div className="strategy-pool-menu-header">
        <button
          type="button"
          className="strategy-pool-menu-toggle"
          aria-expanded={isMenuOpen}
          onClick={() => setIsMenuOpen((current) => !current)}
        >
          <span>Pattern Identity</span>
          <span className="strategy-pool-menu-icon">{isMenuOpen ? '-' : '+'}</span>
        </button>
      </div>

      {isMenuOpen ? (
        <>
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
        </>
      ) : null}

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
      title="Pattern Identity"
      subtitle="Family feature controls"
      controls={<span className="dashboard-card-label">Curated</span>}
      bodyClassName="strategy-variation-pool-body"
    >
      {content}
    </DashboardCardFrame>
  );
};

export default StrategyVariationPoolCard;
