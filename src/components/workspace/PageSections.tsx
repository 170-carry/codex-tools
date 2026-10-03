import { useId, useState, type ReactNode, type KeyboardEvent } from "react";

export type PageSection = { id: string; label: string; content: ReactNode };

export function PageSections({
  label,
  sections,
  initialId,
}: {
  label: string;
  sections: PageSection[];
  initialId?: string;
}) {
  const prefix = useId();
  const [selected, setSelected] = useState(initialId ?? sections[0]?.id);
  const active = sections.some((section) => section.id === selected)
    ? selected
    : sections[0]?.id;
  const onKeyDown = (
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? sections.length - 1
          : (index + (event.key === "ArrowLeft" ? -1 : 1) + sections.length) %
            sections.length;
    setSelected(sections[next].id);
    const tabs =
      event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>(
        "[role=tab]",
      );
    tabs?.[next]?.focus();
  };
  return (
    <div className="pageSections">
      <nav className="pageSectionTabs" role="tablist" aria-label={label}>
        {sections.map((section, index) => (
          <button
            key={section.id}
            type="button"
            role="tab"
            id={`${prefix}-${section.id}-tab`}
            aria-controls={`${prefix}-${section.id}-panel`}
            aria-selected={active === section.id}
            tabIndex={active === section.id ? 0 : -1}
            onClick={() => setSelected(section.id)}
            onKeyDown={(event) => onKeyDown(event, index)}
          >
            {section.label}
          </button>
        ))}
      </nav>
      {sections.map((section) => (
        <div
          key={section.id}
          className="pageSectionPanel"
          id={`${prefix}-${section.id}-panel`}
          role="tabpanel"
          aria-labelledby={`${prefix}-${section.id}-tab`}
          hidden={active !== section.id}
        >
          {section.content}
        </div>
      ))}
    </div>
  );
}
