import template from './calendar.html?raw';
import holidays from './holidays.json';
import type { GitActivityReport } from './types';

export type CalendarViewState = { month?: string; repo?: string; who?: string; heat?: string; bulk?: boolean; scrollY?: number; columns?: 'auto' | '1' | '2' | '3'; density?: 'comfortable' | 'compact' };

export const CALENDAR_LAYOUT_KEY = 'codex-tools-git-calendar-layout';
export function readCalendarLayout(): CalendarViewState {
  try {
    const saved = JSON.parse(localStorage.getItem(CALENDAR_LAYOUT_KEY) ?? 'null');
    if (saved && ['auto', '1', '2', '3'].includes(saved.columns) && ['comfortable', 'compact'].includes(saved.density)) return {columns: saved.columns, density: saved.density};
  } catch { /* A saved preference is optional. */ }
  return {columns: 'auto', density: 'comfortable'};
}

export function calendarHtml(report: GitActivityReport, view: CalendarViewState = {}): string {
  // Repository metadata is untrusted. Keep it inside JSON, never executable HTML.
  const json = JSON.stringify({ ...report, ...holidays, view })
    .replace(/</g, '\\u003c').replace(/\u2028/g, '\\u2028').replace(/\u2029/g, '\\u2029');
  return template.replace('__DATA__', () => json);
}
