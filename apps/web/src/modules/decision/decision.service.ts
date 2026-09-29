import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type { CreateDecisionValues, Decision, DecisionSummary, SimulationResult } from './schema';

const base = (projectId: string) => `projects/${encodeURIComponent(projectId)}/decisions`;
const one = (projectId: string, id: string) => `${base(projectId)}/${encodeURIComponent(id)}`;

export function listDecisions(projectId: string): Promise<ActionResult<DecisionSummary[]>> {
  return attempt(async () => {
    const response = await http.get(base(projectId));
    return response.ok ? success((await response.json<{ items: DecisionSummary[] }>()).items) : failure(response);
  });
}

export function getDecision(projectId: string, id: string): Promise<ActionResult<Decision>> {
  return attempt(async () => {
    const response = await http.get(one(projectId, id));
    return response.ok ? success(await response.json<Decision>()) : failure(response);
  });
}

export function createDecision(projectId: string, values: CreateDecisionValues): Promise<ActionResult<Decision>> {
  return attempt(async () => {
    const response = await http.post(base(projectId), { json: values });
    return response.ok ? success(await response.json<Decision>()) : failure(response);
  });
}

/** Saves the draft from `revision`; `DECISION_CONFLICT` when someone saved since. */
export function saveDecision(
  projectId: string,
  id: string,
  content: unknown,
  revision: number,
): Promise<ActionResult<DecisionSummary>> {
  return attempt(async () => {
    const response = await http.put(one(projectId, id), { json: { content, revision } });
    return response.ok ? success(await response.json<DecisionSummary>()) : failure(response);
  });
}

export function deleteDecision(projectId: string, id: string): Promise<ActionResult<null>> {
  return attempt(async () => {
    const response = await http.delete(one(projectId, id));
    return response.ok ? success(null) : failure(response);
  });
}

/** Runs the decision with the project's other decisions; `content` is the editor's graph. */
export function simulateDecision(
  projectId: string,
  id: string,
  content: unknown,
  context: unknown,
): Promise<ActionResult<SimulationResult>> {
  return attempt(async () => {
    const response = await http.post(`${one(projectId, id)}/simulate`, { json: { content, context } });
    return response.ok ? success(await response.json<SimulationResult>()) : failure(response);
  });
}
