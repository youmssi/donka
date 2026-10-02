import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type {
  DecisionRecord,
  IssuedLogToken,
  LogQuery,
  LogSettings,
  LogToken,
  RecordList,
  Replay,
  TokenValues,
} from './schema';

export const PAGE_SIZE = 50;

const log = (projectId: string) => `projects/${encodeURIComponent(projectId)}/decision-log`;

export function searchRecords(projectId: string, query: LogQuery, offset: number): Promise<ActionResult<RecordList>> {
  return attempt(async () => {
    const params = new URLSearchParams();
    for (const [name, value] of Object.entries(query)) if (value) params.set(name, value);
    params.set('limit', String(PAGE_SIZE));
    params.set('offset', String(offset));
    const response = await http.get(log(projectId), { searchParams: params });
    return response.ok ? success(await response.json<RecordList>()) : failure(response);
  });
}

/** Opens a record; Studio records the opening in the project's audit log. */
export function getRecord(projectId: string, id: string): Promise<ActionResult<DecisionRecord>> {
  return attempt(async () => {
    const response = await http.get(`${log(projectId)}/${encodeURIComponent(id)}`);
    return response.ok ? success(await response.json<DecisionRecord>()) : failure(response);
  });
}

export function replayRecord(projectId: string, id: string): Promise<ActionResult<Replay>> {
  return attempt(async () => {
    const response = await http.post(`${log(projectId)}/${encodeURIComponent(id)}/replay`);
    return response.ok ? success(await response.json<Replay>()) : failure(response);
  });
}

export function getSettings(projectId: string): Promise<ActionResult<LogSettings>> {
  return attempt(async () => {
    const response = await http.get(`${log(projectId)}/settings`);
    return response.ok ? success(await response.json<LogSettings>()) : failure(response);
  });
}

export function saveSettings(projectId: string, settings: LogSettings): Promise<ActionResult<LogSettings>> {
  return attempt(async () => {
    const response = await http.put(`${log(projectId)}/settings`, { json: settings });
    return response.ok ? success(await response.json<LogSettings>()) : failure(response);
  });
}

export function listTokens(): Promise<ActionResult<LogToken[]>> {
  return attempt(async () => {
    const response = await http.get('decision-log/tokens');
    return response.ok ? success((await response.json<{ items: LogToken[] }>()).items) : failure(response);
  });
}

export function issueToken(values: TokenValues): Promise<ActionResult<IssuedLogToken>> {
  return attempt(async () => {
    const response = await http.post('decision-log/tokens', { json: values });
    return response.ok ? success(await response.json<IssuedLogToken>()) : failure(response);
  });
}

export function revokeToken(id: string): Promise<ActionResult<null>> {
  return attempt(async () => {
    const response = await http.delete(`decision-log/tokens/${encodeURIComponent(id)}`);
    return response.ok ? success(null) : failure(response);
  });
}
