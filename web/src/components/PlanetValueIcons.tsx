import React from "react";

export const ResourceIcon: React.FC<{ style?: React.CSSProperties }> = ({ style }) => (
  <svg
    viewBox="0 0 16 16"
    width="13"
    height="13"
    fill="currentColor"
    aria-hidden="true"
    style={{ display: "inline-block", verticalAlign: "-2px", color: "#fbbf24", ...style }}
  >
    <path d="M8 1 L14 7 L8 15 L2 7 Z" />
  </svg>
);

export const InfluenceIcon: React.FC<{ style?: React.CSSProperties }> = ({ style }) => (
  <svg
    viewBox="0 0 16 16"
    width="13"
    height="13"
    fill="currentColor"
    aria-hidden="true"
    style={{ display: "inline-block", verticalAlign: "-2px", color: "#38bdf8", ...style }}
  >
    <path d="M2 13h12v1.5H2zm1.5-2l2-6 2.5 3 2.5-3 2 6H3.5z" />
  </svg>
);
