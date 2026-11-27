import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type { Snapshot } from './schema';

const stability = (projectId: string) => `projects/${encodeURIComponent(projectId)}/stability`;

export interface SnapshotValues {
  decisionKey: string;
  referenceFrom: string;
  referenceUntil: string;
  recentFrom: string;
  recentUntil: string;
  outputField?: string;
  outputBins?: number[];
  inputFields?: string[];
}

export function takeSnapshot(projectId: string, values: SnapshotValues): Promise<ActionResult<Snapshot>> {
  return attempt(async () => {
    const response = await http.post(stability(projectId), { json: values });
    return response.ok ? success(await response.json<Snapshot>()) : failure(response);
  });
}

export function listSnapshots(projectId: string, decisionKey: string): Promise<ActionResult<Snapshot[]>> {
  return attempt(async () => {
    const response = await http.get(`${stability(projectId)}?decisionKey=${encodeURIComponent(decisionKey)}`);
    return response.ok ? success(await response.json<Snapshot[]>()) : failure(response);
  });
}
