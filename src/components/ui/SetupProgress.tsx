export function SetupProgress({steps, current}: {steps: string[]; current: number}) {
  return <ol className="uiSetupProgress">{steps.map((step, index) => <li key={step} aria-current={index === current ? "step" : undefined} className={index <= current ? "isReached" : ""}><span aria-hidden="true">{index < current ? "✓" : index + 1}</span>{step}</li>)}</ol>;
}
