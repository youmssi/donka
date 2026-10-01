import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type {
  CreateDecisionValues,
  Decision,
  DecisionSummary,
  MatchMode,
  Restored,
  Scenario,
  ScenarioList,
  SimulationResult,
  TestResultList,
  Version,
  VersionDetail,
  VersionList,
} from './schema';

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

export const VERSIONS_PAGE_SIZE = 20;

export function listVersions(projectId: string, id: string, offset: number): Promise<ActionResult<VersionList>> {
  return attempt(async () => {
    const response = await http.get(`${one(projectId, id)}/versions`, {
      searchParams: { limit: VERSIONS_PAGE_SIZE, offset },
    });
    return response.ok ? success(await response.json<VersionList>()) : failure(response);
  });
}

export function getVersion(projectId: string, id: string, number: number): Promise<ActionResult<VersionDetail>> {
  return attempt(async () => {
    const response = await http.get(`${one(projectId, id)}/versions/${number}`);
    return response.ok ? success(await response.json<VersionDetail>()) : failure(response);
  });
}

/** Saves the draft at `revision` as the next version. */
export function saveVersion(
  projectId: string,
  id: string,
  message: string,
  revision: number,
): Promise<ActionResult<Version>> {
  return attempt(async () => {
    const response = await http.post(`${one(projectId, id)}/versions`, { json: { message, revision } });
    return response.ok ? success(await response.json<Version>()) : failure(response);
  });
}

/** Restores a version: it becomes the draft and a new version. */
export function restoreVersion(
  projectId: string,
  id: string,
  number: number,
  revision: number,
): Promise<ActionResult<Restored>> {
  return attempt(async () => {
    const response = await http.post(`${one(projectId, id)}/versions/${number}/restore`, { json: { revision } });
    return response.ok ? success(await response.json<Restored>()) : failure(response);
  });
}

const scenarios = (projectId: string) => `projects/${encodeURIComponent(projectId)}/test-scenarios`;

export const SCENARIOS_PAGE_SIZE = 100;

/** The project's scenarios by decision and name, a page at a time. */
export function listScenarios(projectId: string, offset: number): Promise<ActionResult<ScenarioList>> {
  return attempt(async () => {
    const response = await http.get(scenarios(projectId), { searchParams: { limit: SCENARIOS_PAGE_SIZE, offset } });
    return response.ok ? success(await response.json<ScenarioList>()) : failure(response);
  });
}

/** What the API takes for a scenario: the form's JSON text parsed. */
export interface ScenarioInput {
  name: string;
  input: Record<string, unknown>;
  expected: Record<string, unknown>;
  match: MatchMode;
}

export function createScenario(
  projectId: string,
  decisionId: string,
  values: ScenarioInput,
): Promise<ActionResult<Scenario>> {
  return attempt(async () => {
    const response = await http.post(scenarios(projectId), { json: { decisionId, ...values } });
    return response.ok ? success(await response.json<Scenario>()) : failure(response);
  });
}

export function updateScenario(projectId: string, id: string, values: ScenarioInput): Promise<ActionResult<Scenario>> {
  return attempt(async () => {
    const response = await http.put(`${scenarios(projectId)}/${encodeURIComponent(id)}`, { json: values });
    return response.ok ? success(await response.json<Scenario>()) : failure(response);
  });
}

export function deleteScenario(projectId: string, id: string): Promise<ActionResult<null>> {
  return attempt(async () => {
    const response = await http.delete(`${scenarios(projectId)}/${encodeURIComponent(id)}`);
    return response.ok ? success(null) : failure(response);
  });
}

/** How every scenario of the project went when this version was saved. */
export function getTestResults(projectId: string, id: string, number: number): Promise<ActionResult<TestResultList>> {
  return attempt(async () => {
    const response = await http.get(`${one(projectId, id)}/versions/${number}/test-results`);
    return response.ok ? success(await response.json<TestResultList>()) : failure(response);
  });
}
