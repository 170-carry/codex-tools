import { formatQuotaTime } from "../../utils/quotaTime";

export function QuotaResetTime({
  resetAt,
  label,
}: {
  resetAt: number | null | undefined;
  label: string;
}) {
  const value = formatQuotaTime(resetAt);
  if (!value)
    return (
      <span className="quotaReset" aria-label={`${label} —`}>
        —
      </span>
    );
  const description = `${label} ${value.date} ${value.time} (${value.timeZone}, ${value.offset})`;
  return (
    <time
      className="quotaReset"
      dateTime={value.iso}
      title={description}
      aria-label={description}
    >
      <span>{value.date}</span>
      <span>{value.time}</span>
    </time>
  );
}
