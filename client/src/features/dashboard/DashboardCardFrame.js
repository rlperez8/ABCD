import React from 'react';

const DashboardCardFrame = ({
  title,
  subtitle,
  controls,
  children,
  bodyClassName = '',
  isCollapsed = false,
}) => (
  <div className={['accuracy', isCollapsed ? 'accuracy--collapsed' : ''].filter(Boolean).join(' ')}>
    <div className="dashboard-card-header">
      <div className="dashboard-card-copy">
        <div className="dashboard-card-title">{title}</div>
        {subtitle ? <div className="dashboard-card-subtitle">{subtitle}</div> : null}
      </div>
      {controls ? <div className="dashboard-card-controls">{controls}</div> : null}
    </div>

    {!isCollapsed ? (
      <div className={`dashboard-card-body ${bodyClassName}`.trim()}>{children}</div>
    ) : null}
  </div>
);

export default DashboardCardFrame;
