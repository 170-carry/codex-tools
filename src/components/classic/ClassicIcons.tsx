export function ActionIcon({
  type,
}: {
  type: "login" | "warmup" | "export" | "delete" | "switch" | "edit";
}) {
  if (type === "delete") {
    return (
      <svg
        className="actionIconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M3 6h18" />
        <path d="M8 6V4h8v2" />
        <path d="m19 6-1 14H6L5 6" />
        <path d="M10 11v6" />
        <path d="M14 11v6" />
      </svg>
    );
  }

  if (type === "export") {
    return (
      <svg
        className="actionIconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <rect x="8" y="8" width="10" height="12" rx="2" />
        <path d="M6 16H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v1" />
      </svg>
    );
  }

  if (type === "login") {
    return (
      <svg
        className="actionIconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M20 6 9 17l-5-5" />
      </svg>
    );
  }

  if (type === "warmup") {
    return (
      <svg
        className="actionIconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M13 2c1 4-2 5-2 8 0 1.7 1.3 3 3 3 2.5 0 4-2.1 3.5-4.5C20 10.5 21 13 21 16a9 9 0 0 1-18 0c0-4 2.2-7.1 5.5-9-.5 3 1 4.5 2.5 5.5" />
      </svg>
    );
  }

  if (type === "edit") {
    return (
      <svg
        className="actionIconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M12 20h9" />
        <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
      </svg>
    );
  }

  return (
    <svg
      className="actionIconGlyph"
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M12 5v14" />
      <path d="m19 12-7 7-7-7" />
    </svg>
  );
}
export function SearchIcon() {
  return (
    <svg
      className="searchIcon"
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <circle cx="11" cy="11" r="7" />
      <path d="m16 16 4 4" />
    </svg>
  );
}
