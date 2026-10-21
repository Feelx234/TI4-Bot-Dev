import React from "react";
import { attachmentInfo } from "../presentation/attachmentInfo.ts";
import { InfoPopover } from "./InfoPopover.tsx";
import "./AttachmentInfo.css";

/**
 * An attachment's name that opens a card with its effect on hover, keyboard focus and tap. Used
 * wherever the UI names an attachment on a planet.
 */
export const AttachmentInfo: React.FC<{ id: string; position?: "top" | "bottom" | "left" | "right" }> = ({
  id,
  position = "top",
}) => {
  const info = attachmentInfo(id);
  return (
    <span className="attachment-info" data-testid={`attachment-info-${id}`}>
      <InfoPopover label={`${info.name}: what it does`} trigger={info.name} position={position}>
        <div className="attachment-info__card" data-testid={`attachment-card-${id}`}>
          <strong className="attachment-info__name">{info.name}</strong>
          <p className="attachment-info__text">{info.text}</p>
        </div>
      </InfoPopover>
    </span>
  );
};
