import React, { useState } from 'react';

const FilterDropdown = ({
  label,
  onSelect,
  options,
  expandedMenu,
  setExpandedMenu,
  initialValue = '',
  value,
}) => {
  const [internalValue, setInternalValue] = useState(initialValue);
  const isOpen = expandedMenu === label;
  const selectedValue = value ?? internalValue;

  return (
    <div className="setting-container">
      <div className="setting-value-outer">
        <div className="setting-value-inner" onClick={() => setExpandedMenu(isOpen ? '' : label)}>
          <div className="value-name">{selectedValue || label}</div>
          <div className="value-arrow">
            <img className="value-arrow-img" src="/images/dropdown.png" alt="" />
          </div>
        </div>

        {isOpen && (
          <div className="setting-dropdown">
            {options.map((option) => (
              <div
                key={option}
                className="setting-dropdown-item"
                onClick={() => {
                  setInternalValue(option);
                  onSelect(option);
                  setExpandedMenu('');
                }}
              >
                {option}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

export default FilterDropdown;
