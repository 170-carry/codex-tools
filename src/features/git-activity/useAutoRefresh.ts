import { useEffect, useEffectEvent, useRef, useState } from 'react';

const STORAGE_KEY = 'codex-tools-git-auto-refresh';
type Preferences = { onOpen: boolean; minutes: number };
const defaults: Preferences = { onOpen: true, minutes: 15 };

function readPreferences(): Preferences {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null');
    if (saved && typeof saved.onOpen === 'boolean' && [0, 5, 15, 30, 60].includes(saved.minutes)) return saved;
  } catch { /* Use defaults if local preferences are unavailable. */ }
  return defaults;
}

export function useAutoRefresh({ ready, hasSources, paused, finishedAt, refresh }: {
  ready: boolean;
  hasSources: boolean;
  paused: boolean;
  finishedAt: number;
  refresh: () => Promise<boolean>;
}) {
  const [preferences, setPreferences] = useState(readPreferences);
  const initialized = useRef(false);
  const attempt = useEffectEvent(async () => {
    if (paused) return false;
    return refresh();
  });
  useEffect(() => {
    if (!ready) return;
    const firstOpen = !initialized.current;
    initialized.current = true;
    if (!hasSources) return;
    const interval = preferences.minutes > 0 ? preferences.minutes * 60_000 : Infinity;
    let due = firstOpen && preferences.onOpen ? Date.now() : Date.now() + interval;
    let busy = false;
    let active = true;
    const tick = async () => {
      if (!active || busy || Date.now() < due) return;
      busy = true;
      try {
        if (await attempt()) due = Date.now() + interval;
      } finally { busy = false; }
    };
    const initial = window.setTimeout(() => void tick(), 0);
    const timer = window.setInterval(() => void tick(), 10_000);
    return () => { active = false; clearTimeout(initial); clearInterval(timer); };
  }, [ready, hasSources, preferences.onOpen, preferences.minutes, finishedAt]);

  function updatePreferences(next: Preferences) {
    setPreferences(next);
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(next)); } catch { /* Current session still works. */ }
  }
  return { preferences, updatePreferences };
}
