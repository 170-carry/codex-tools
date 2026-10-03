import type { AccountSummary } from "../../types/app";
import { formatPlan, planTone } from "../../utils/usage";

export function AccountPlanTags({
  variants,
  selectedId,
  labels,
  onSelect,
}: {
  variants: AccountSummary[];
  selectedId: string;
  labels: Record<string, string>;
  onSelect: (id: string) => void;
}) {
  const interactive = variants.length > 1;
  return (
    <span className={`accountPlanVariants${interactive ? " hasVariants" : ""}`}>
      {variants.map((variant) => {
        const plan = variant.planType || variant.usage?.planType;
        const tone = planTone(plan);
        const label = ["pro", "plus", "free"].includes(tone)
          ? tone.toUpperCase()
          : formatPlan(plan, labels);
        const selected = variant.id === selectedId;
        const className = `planChip tone-${tone}${interactive && selected ? " isSelected" : ""}`;
        return interactive ? (
          <button
            key={variant.id}
            type="button"
            className={className}
            data-plan={tone}
            aria-pressed={selected}
            onClick={() => onSelect(variant.id)}
          >
            {label}
          </button>
        ) : (
          <span key={variant.id} className={className} data-plan={tone}>
            {label}
          </span>
        );
      })}
    </span>
  );
}
