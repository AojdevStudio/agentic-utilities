const LEGACY_ROLE_SUFFIXES = ["control-pane", "claude-impl", "codex-impl", "pi-impl", "codex-review", "claude-review"];

export function parseLegacyFleetLabel(label) {
  if (typeof label !== "string") return undefined;
  for (const role of LEGACY_ROLE_SUFFIXES) {
    const suffix = `-${role}`;
    if (label.endsWith(suffix) && label.length > suffix.length) {
      return { key: label.slice(0, -suffix.length), role, source: "legacy-label" };
    }
  }
  return undefined;
}

export function paneFleetIdentity(pane) {
  const tokens = pane?.tokens;
  if (tokens?.fleet_key) {
    return {
      key: tokens.fleet_key,
      role: tokens.fleet_role,
      kind: tokens.fleet_kind,
      owner: tokens.fleet_owner,
      source: "metadata",
    };
  }
  return parseLegacyFleetLabel(pane?.label);
}
