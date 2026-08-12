export function hasActiveQuotaDisplay(taskbarEnabled: boolean, trayEnabled: boolean): boolean {
  return taskbarEnabled || trayEnabled;
}

export function canDisableQuotaDisplay(otherDisplayEnabled: boolean): boolean {
  return otherDisplayEnabled;
}

export async function applyLiveQuotaDisplayUpdate<TPatch>(options: {
  patch: TPatch;
  applyLocal: () => void;
  rollbackLocal: () => void;
  persist: (patch: TPatch) => Promise<void>;
}): Promise<boolean> {
  options.applyLocal();
  try {
    await options.persist(options.patch);
    return true;
  } catch {
    options.rollbackLocal();
    return false;
  }
}
