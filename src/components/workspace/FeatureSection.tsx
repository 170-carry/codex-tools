import type { ReactNode } from "react";
import { WorkspaceIcon } from "./WorkspaceIcon";

export function FeatureSection({
  title,
  className = "",
  children,
  defaultOpen = false,
}: {
  title: string;
  className?: string;
  children: ReactNode;
  defaultOpen?: boolean;
}) {
  return (
    <details className={`featureDisclosure ${className}`} open={defaultOpen}>
      <summary>
        <span>{title}</span>
        <WorkspaceIcon name="chevron" />
      </summary>
      <div className="featureDisclosureBody">{children}</div>
    </details>
  );
}
