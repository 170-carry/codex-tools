export function ProxyHelpTip({
  label,
  children,
}: {
  label: string;
  children: string;
}) {
  return (
    <span className="proxyHelpTip">
      <button
        type="button"
        className="proxyHelpButton"
        aria-label={label}
        title={children}
      >
        ?
      </button>
      <span className="proxyHelpBubble" role="tooltip">
        {children}
      </span>
    </span>
  );
}
