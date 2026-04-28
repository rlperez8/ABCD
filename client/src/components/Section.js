import React from 'react';

const Section = ({ children }) => {
  return (
    <div className="rp-section">
      <div className="margin-">
        <div className="table_body_main">{children}</div>
      </div>
    </div>
  );
};

export default Section;
