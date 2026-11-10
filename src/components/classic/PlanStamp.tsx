/** Compact outlined stamp; the parent button owns its accessible name. */
export function PlanStamp({ label }: { label: string }) {
  const width = Math.max(42, label.length * 6.5 + 16);
  return <svg className="planStamp" viewBox={`0 0 ${width} 24`} style={{ width }} aria-hidden="true">
    <rect x="1" y="2" width={width - 2} height="20" rx="3" fill="none" stroke="currentColor" strokeWidth="1.2" />
    <path d={`M6 5H${width - 6}M6 19H${width - 6}`} fill="none" stroke="currentColor" strokeWidth=".5" opacity=".55" />
    <text x={width / 2} y="12.5" textAnchor="middle" dominantBaseline="middle" fill="currentColor" fontSize="9" fontWeight="700" letterSpacing=".6">{label}</text>
  </svg>;
}
