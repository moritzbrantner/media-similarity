import type { IdentityKind, IdentityMutationResponse } from "../types";

import { parseResponse } from "./client";

export { type IdentityKind, type IdentityMutationResponse };

export async function renameIdentity(
  kind: IdentityKind,
  id: string,
  label: string,
): Promise<IdentityMutationResponse> {
  const response = await fetch(`${identityRoute(kind)}/${encodeURIComponent(id)}`, {
    body: JSON.stringify({ label }),
    headers: { "Content-Type": "application/json" },
    method: "PUT",
  });
  return parseResponse<IdentityMutationResponse>(response);
}

export async function mergeIdentities(
  kind: IdentityKind,
  targetId: string,
  sourceIds: string[],
): Promise<IdentityMutationResponse> {
  const response = await fetch(`${identityRoute(kind)}/${encodeURIComponent(targetId)}/merge`, {
    body: JSON.stringify({ source_ids: sourceIds }),
    headers: { "Content-Type": "application/json" },
    method: "POST",
  });
  return parseResponse<IdentityMutationResponse>(response);
}

function identityRoute(kind: IdentityKind) {
  return kind === "person" ? "/api/identities/people" : "/api/identities/speakers";
}
