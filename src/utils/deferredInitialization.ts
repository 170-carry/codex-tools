export function createDeferredInitialization() {
  let started = false;
  return async (state: { settingsLoaded: boolean; recoveryRequired: boolean }, tasks: (() => Promise<unknown>)[]): Promise<boolean> => {
    if (state.recoveryRequired) { started = false; return false; }
    if (!state.settingsLoaded || started) return false;
    started = true;
    await Promise.allSettled(tasks.map((task) => task()));
    return true;
  };
}
