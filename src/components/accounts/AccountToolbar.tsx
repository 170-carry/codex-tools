import { useEffect, useRef, useState } from "react";
import type { StatusFilter, UiCopy } from "./types";
import { WorkspaceIcon } from "../workspace/WorkspaceIcon";
import { getCompactTableCopy } from "../../i18n/compactTableCopy";

export function AccountToolbar({
  query,
  onQuery,
  status,
  onStatus,
  plan,
  onPlan,
  text,
  locale,
  onClose,
}: {
  query: string;
  onQuery: (query: string) => void;
  status: StatusFilter;
  onStatus: (status: StatusFilter) => void;
  plan: string;
  onPlan: (plan: string) => void;
  text: UiCopy;
  locale: string;
  onClose: () => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const active = status !== "all" || plan !== "all";
  const filterLabel = locale === "zh-CN" ? "筛选" : "Filter";
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    inputRef.current?.focus();
  }, []);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target))
        setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
        trigger.current?.focus();
      }
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);
  return (
    <div className="accountToolbar" id="account-search-toolbar">
      <label className="accountSearch">
        <WorkspaceIcon name="search" />
        <input
          ref={inputRef}
          data-account-search
          value={query}
          onChange={(event) => onQuery(event.currentTarget.value)}
          placeholder={text.searchPlaceholder}
          aria-label={text.searchPlaceholder}
        />
        {query ? (
          <button
            type="button"
            aria-label={locale === "zh-CN" ? "清除搜索" : "Clear search"}
            onClick={() => onQuery("")}
          >
            <WorkspaceIcon name="close" />
          </button>
        ) : (
          <kbd>⌘F</kbd>
        )}
      </label>
      <div className="accountFilterRoot" ref={root}>
        <button
          ref={trigger}
          type="button"
          className={`filterButton${active ? " isActive" : ""}`}
          onClick={() => setOpen(!open)}
          aria-expanded={open}
          aria-controls="account-filters"
        >
          <WorkspaceIcon name="filter" />
          <span>{filterLabel}</span>
          {active ? <i /> : null}
        </button>
        {open ? (
          <div className="accountFilterPanel" id="account-filters">
            <label>
              {text.allStatuses}
              <select
                value={status}
                onChange={(event) =>
                  onStatus(event.currentTarget.value as StatusFilter)
                }
              >
                <option value="all">{text.allStatuses}</option>
                {(
                  ["using", "available", "low", "exhausted", "issue"] as const
                ).map((value, index) => (
                  <option value={value} key={value}>
                    {
                      [
                        text.statusUsing,
                        text.statusAvailable,
                        text.statusLow,
                        text.statusExhausted,
                        text.statusIssue,
                      ][index]
                    }
                  </option>
                ))}
              </select>
            </label>
            <label>
              {text.allPlans}
              <select
                value={plan}
                onChange={(event) => onPlan(event.currentTarget.value)}
              >
                <option value="all">{text.allPlans}</option>
                {[
                  "pro",
                  "plus",
                  "team",
                  "enterprise",
                  "business",
                  "api",
                  "free",
                  "unknown",
                ].map((value) => (
                  <option value={value} key={value}>
                    {value.toUpperCase()}
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              onClick={() => {
                onStatus("all");
                onPlan("all");
                setOpen(false);
              }}
            >
              {locale === "zh-CN" ? "清除筛选" : "Clear filters"}
            </button>
          </div>
        ) : null}
      </div>
      <button
        type="button"
        className="toolbarIconButton closeAccountSearch"
        aria-label={getCompactTableCopy(locale).closeSearch}
        title={getCompactTableCopy(locale).closeSearch}
        onClick={() => {
          onQuery("");
          onStatus("all");
          onPlan("all");
          onClose();
        }}
      >
        <WorkspaceIcon name="close" />
      </button>
    </div>
  );
}
