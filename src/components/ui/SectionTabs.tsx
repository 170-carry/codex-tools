import { useId, useState, type ReactNode } from "react";

export function SectionTabs({
  label,
  sections,
  initial,
  value,
  onValueChange,
}: {
  label: string;
  sections: { id: string; label: string; count?: number; content: ReactNode }[];
  initial: string;
  value?: string;
  onValueChange?: (value: string) => void;
}) {
  const [internal, setInternal] = useState(initial);
  const active = value ?? internal;
  const setActive = (next: string) => { setInternal(next); onValueChange?.(next); };
  const id = useId();
  return (
    <section className="sectionTabs">
      <div className="sectionTabList" role="tablist" aria-label={label}>
        {sections.map((section, index) => (
          <button
            type="button"
            role="tab"
            key={section.id}
            id={`${id}-${section.id}-tab`}
            aria-controls={`${id}-${section.id}-panel`}
            aria-selected={active === section.id}
            tabIndex={active === section.id ? 0 : -1}
            onClick={() => setActive(section.id)}
            onKeyDown={(event) => {
              const next =
                event.key === "ArrowRight"
                  ? (index + 1) % sections.length
                  : event.key === "ArrowLeft"
                    ? (index + sections.length - 1) % sections.length
                    : event.key === "Home"
                      ? 0
                      : event.key === "End"
                        ? sections.length - 1
                        : null;
              if (next === null) return;
              event.preventDefault();
              setActive(sections[next].id);
              document
                .getElementById(`${id}-${sections[next].id}-tab`)
                ?.focus();
            }}
          >
            {section.label}
            {section.count !== undefined ? <span>{section.count}</span> : null}
          </button>
        ))}
      </div>
      {sections.map((section) => (
        <div
          role="tabpanel"
          key={section.id}
          id={`${id}-${section.id}-panel`}
          aria-labelledby={`${id}-${section.id}-tab`}
          hidden={active !== section.id}
          tabIndex={0}
        >
          {section.content}
        </div>
      ))}
    </section>
  );
}
