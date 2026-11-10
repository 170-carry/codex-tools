export type AccountProvider = "codex" | "claude";
export function ProviderSwitch({
  provider,
  onChange,
}: {
  provider: AccountProvider;
  onChange: (provider: AccountProvider) => void;
}) {
  return (
    <div className="providerSwitch" role="group" aria-label="Codex / Claude">
      {(["codex", "claude"] as const).map((value) => (
        <button
          type="button"
          key={value}
          aria-pressed={provider === value}
          title={value === "codex" ? "Codex" : "Claude"}
          aria-label={value === "codex" ? "Codex" : "Claude"}
          className={provider === value ? "isActive" : ""}
          onClick={() => onChange(value)}
        >
          <img src={`/providers/${value}.svg`} alt="" aria-hidden="true" />
          <span>{value === "codex" ? "Codex" : "Claude"}</span>
        </button>
      ))}
    </div>
  );
}
