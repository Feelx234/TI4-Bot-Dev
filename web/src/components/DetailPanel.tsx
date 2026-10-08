import React, { useEffect, useRef, useState } from "react";
import { Drawer } from "../primitives/index.ts";

/** Shared, persistent inspection surface for map and card details. */
export const DetailPanel: React.FC<{
  title: string;
  onClose: () => void;
  children: React.ReactNode;
  testId?: string;
  closeTestId?: string;
}> = ({
  title,
  onClose,
  children,
  testId = "detail-panel",
  closeTestId = "close-detail-button",
}) => {
  const closeRef = useRef<HTMLButtonElement>(null);
  // Phone only (the fold control and styles exist in the compact layout): the panel covers most of the
  // map, so it folds to a slim bar with the title that keeps the map tappable. Stays folded while the
  // title changes (tapping another system), and is gone with the panel.
  const [folded, setFolded] = useState(false);
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeRef.current?.focus();
    return () => {
      if (document.activeElement === closeRef.current) previous?.focus();
    };
  }, []);
  return (
    <Drawer
      open
      onClose={onClose}
      modal={false}
      ariaLabel={`${title} details`}
      data-testid={testId}
      className={`detail-panel panel${folded ? " detail-panel--folded" : ""}`}
    >
      <div className="detail-panel__header">
        <h2>{title}</h2>
        <button
          type="button"
          className="button button--secondary button--icon detail-panel__fold"
          data-testid={`${testId}-fold`}
          aria-expanded={!folded}
          aria-label={folded ? `Show ${title} details` : `Fold ${title} details`}
          onClick={() => setFolded((value) => !value)}
        >
          {folded ? "▴" : "▾"}
        </button>
        <button
          ref={closeRef}
          type="button"
          className="button button--secondary button--icon"
          data-testid={closeTestId}
          aria-label={`Close ${title} details`}
          onClick={onClose}
        >
          ✕
        </button>
      </div>
      <div className="detail-panel__body">{children}</div>
    </Drawer>
  );
};
