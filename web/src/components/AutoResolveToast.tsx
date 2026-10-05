import React, { useEffect, useState } from 'react';
import './AutoResolveToast.css';

export interface AutoResolveNotification {
  id: string;
  decisionType: string;
  selectedValue: string;
}

interface AutoResolveToastProps {
  notification: AutoResolveNotification;
  onDismiss: (id: string) => void;
}

/**
 * Toast notification for auto-resolved single-choice decisions.
 * Displays in the bottom-left corner and auto-dismisses after 3.5 seconds.
 */
export const AutoResolveToast: React.FC<AutoResolveToastProps> = ({
  notification,
  onDismiss,
}) => {
  const [isDismissing, setIsDismissing] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => {
      setIsDismissing(true);
      // Give animation time to finish before removing from DOM
      const animationTimer = setTimeout(() => {
        onDismiss(notification.id);
      }, 300);
      return () => clearTimeout(animationTimer);
    }, 3500); // 3.5 seconds

    return () => clearTimeout(timer);
  }, [notification.id, onDismiss]);

  return (
    <div
      className={`auto-resolve-toast ${isDismissing ? 'dismissing' : ''}`}
      role="status"
      aria-live="polite"
      aria-atomic="true"
    >
      <span className="toast-decision-type">{notification.decisionType}</span>
      {' '}auto-selected:{' '}
      <span className="toast-value">{notification.selectedValue}</span>
    </div>
  );
};

interface AutoResolveToastContainerProps {
  notifications: AutoResolveNotification[];
  onDismiss: (id: string) => void;
}

/**
 * Container for stacking auto-resolve toast notifications.
 * Positioned in the bottom-left corner of the screen.
 */
export const AutoResolveToastContainer: React.FC<
  AutoResolveToastContainerProps
> = ({ notifications, onDismiss }) => {
  if (notifications.length === 0) {
    return null;
  }

  return (
    <div className="auto-resolve-toast-container">
      {notifications.map((notification) => (
        <AutoResolveToast
          key={notification.id}
          notification={notification}
          onDismiss={onDismiss}
        />
      ))}
    </div>
  );
};
