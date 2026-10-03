export type WorkspaceIconName =
  | "accounts"
  | "analytics"
  | "proxy"
  | "settings"
  | "refresh"
  | "plus"
  | "filter"
  | "search"
  | "more"
  | "info"
  | "close"
  | "chevron"
  | "check"
  | "switch"
  | "sun"
  | "moon"
  | "export"
  | "edit"
  | "login"
  | "warmup"
  | "delete";

const paths: Record<WorkspaceIconName, string> = {
  accounts:
    "M16 21v-2a4 4 0 0 0-8 0v2 M16 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0 M20 8a3 3 0 0 1 0 6 M22 21v-2a4 4 0 0 0-3-3.87",
  analytics: "M4 3v17h17 M8 15v-4 M13 15V7 M18 15v-6",
  proxy: "M8 4H4v16h4 M16 4h4v16h-4 M8 12h8 M13 9l3 3-3 3",
  settings:
    "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M9 3l-.6 2.4-2.1 1.2L4 6l-2 3 1.8 1.8v2.4L2 15l2 3 2.3-.6 2.1 1.2L9 21h6l.6-2.4 2.1-1.2L20 18l2-3-1.8-1.8L22 9l-2-3-2.3.6-2.1-1.2L15 3Z",
  refresh: "M20 7a8 8 0 1 0 1 7 M20 3v5h-5",
  plus: "M12 5v14 M5 12h14",
  filter: "M4 7h16 M7 12h10 M10 17h4",
  search: "M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0 M15 15l6 6",
  more: "M5 12h.01 M12 12h.01 M19 12h.01",
  info: "M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0 M12 11v6 M12 7h.01",
  close: "M6 6l12 12 M18 6 6 18",
  chevron: "m9 5 7 7-7 7",
  check: "m5 12 4 4L19 6",
  switch: "M4 8h16 M16 4l4 4-4 4 M20 16H4 M8 12l-4 4 4 4",
  sun: "M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0 M12 2v2 M12 20v2 M2 12h2 M20 12h2 M5 5l1.4 1.4 M17.6 17.6 19 19 M5 19l1.4-1.4 M17.6 6.4 19 5",
  moon: "M20.5 14.6A8.5 8.5 0 0 1 9.4 3.5 8.7 8.7 0 1 0 20.5 14.6Z",
  export: "M12 3v12 M7 8l5-5 5 5 M5 13v7h14v-7",
  edit: "M12 20h9 M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z",
  login: "M10 17l5-5-5-5 M3 12h12 M13 3h7v18h-7",
  warmup:
    "M12 3c1 4-3 5-3 8 0 2 2 3 3 3 3 0 4-2 4-4 3 3 3 5 3 7a7 7 0 0 1-14 0c0-4 2-6 4-8",
  delete: "M3 6h18 M8 6V4h8v2 M19 6l-1 14H6L5 6 M10 11v5 M14 11v5",
};

export function WorkspaceIcon({
  name,
  spinning = false,
  className = "",
}: {
  name: WorkspaceIconName;
  spinning?: boolean;
  className?: string;
}) {
  return (
    <svg
      className={`workspaceIcon${spinning ? " isSpinning" : ""} ${className}`}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={paths[name]} />
    </svg>
  );
}
