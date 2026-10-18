import React from "react";
import { useAetherpassageSetting } from "../hooks/useAetherpassageSetting.ts";

/** Under the Aetherpassage offer: switch the prompt off (it is then declined for you) or back on. */
export const AetherpassageSwitch: React.FC = () => {
  const { ask, toggle } = useAetherpassageSetting();
  return (
    <label
      data-testid="aetherpassage-switch"
      style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, margin: "8px 0", cursor: "pointer" }}
    >
      <input type="checkbox" data-testid="aetherpassage-switch-input" checked={!ask} onChange={toggle} />
      <span>Don't ask about Aetherpassage (decline it for me) until I switch it back on</span>
    </label>
  );
};
