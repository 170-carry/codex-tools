export function NavigationIcon({
  name,
}: {
  name: "accounts" | "analytics" | "proxy" | "settings";
}) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="18"
      height="18"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {name === "accounts" ? (
        <>
          <circle cx="9" cy="8" r="3" />
          <path d="M3 20v-2a6 6 0 0 1 12 0v2M16 5a3 3 0 0 1 0 6m2 3a5 5 0 0 1 3 4v2" />
        </>
      ) : name === "analytics" ? (
        <>
          <path d="M4 3v17h17M9 16v-5m5 5V6m5 10V9" />
        </>
      ) : name === "proxy" ? (
        <>
          <rect x="3" y="3" width="18" height="7" rx="2" />
          <rect x="3" y="14" width="18" height="7" rx="2" />
          <path d="M7 6.5h.01M7 17.5h.01M11 6.5h6M11 17.5h6" />
        </>
      ) : (
        <>
          <path d="m9 3-.5 2-2 1-2-.5-2 3 1.5 1.5v3L2.5 15l2 3 2-.5 2 1 .5 2h4l.5-2 2-1 2 .5 2-3-1.5-2v-3L19.5 8l-2-3-2 .5-2-1L13 3Z" />
          <circle cx="11" cy="12" r="3" />
        </>
      )}
    </svg>
  );
}
