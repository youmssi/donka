import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type {
  Contest,
  DecisionRecord,
  ExplainLanguage,
  Explanation,
  IssuedLogToken,
  LogQuery,
  LogSettings,
  LogSettingsChange,
  LogToken,
  NewRetentionRule,
  PersonRecord,
  RecordList,
  Replay,
  RetentionRule,
  ReviewValues,
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

/** Asks the installation's LLM endpoint to explain a record; audited, and sent without the redacted fields. */
export function explainRecord(
  projectId: string,
  id: string,
  language: ExplainLanguage,
): Promise<ActionResult<Explanation>> {
  return attempt(async () => {
    const response = await http.post(`${log(projectId)}/${encodeURIComponent(id)}/explain`, { json: { language } });
    return response.ok ? success(await response.json<Explanation>()) : failure(response);
  });
}

export function getSettings(projectId: string): Promise<ActionResult<LogSettings>> {
  return attempt(async () => {
    const response = await http.get(`${log(projectId)}/settings`);
    return response.ok ? success(await response.json<LogSettings>()) : failure(response);
  });
}

export function saveSettings(projectId: string, settings: LogSettingsChange): Promise<ActionResult<LogSettings>> {
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

// ----- People's requests (DNK-75): owners; identifiers travel in bodies, never in addresses.

const project = (projectId: string) => `projects/${encodeURIComponent(projectId)}`;

/** The project's records about a person, newest first; audited without the identifier. */
export function findPerson(projectId: string, identifier: string): Promise<ActionResult<PersonRecord[]>> {
  return attempt(async () => {
    const response = await http.post(`${project(projectId)}/people/search`, { json: { identifier } });
    return response.ok ? success((await response.json<{ items: PersonRecord[] }>()).items) : failure(response);
  });
}

export interface ReportFile {
  blob: Blob;
  name: string;
}

/** The person's access report as a PDF; explaining each decision can take a while. */
export function personReport(
  projectId: string,
  request: { identifier: string; language: ExplainLanguage; explain: boolean },
): Promise<ActionResult<ReportFile>> {
  return attempt(async () => {
    const response = await http.post(`${project(projectId)}/people/report`, { json: request, timeout: 300_000 });
    if (!response.ok) return failure(response);
    const name = response.headers.get('content-disposition')?.match(/filename="([^"]+)"/)?.[1];
    return success({ blob: await response.blob(), name: name ?? 'access-report.pdf' });
  });
}

/** Removes the person's fields from their records, for good; refused while a retention rule runs. */
export function anonymisePerson(projectId: string, identifier: string): Promise<ActionResult<number>> {
  return attempt(async () => {
    const response = await http.post(`${project(projectId)}/people/anonymise`, { json: { identifier } });
    return response.ok ? success((await response.json<{ records: number }>()).records) : failure(response);
  });
}

export function listRetentionRules(projectId: string): Promise<ActionResult<RetentionRule[]>> {
  return attempt(async () => {
    const response = await http.get(`${project(projectId)}/retention-rules`);
    return response.ok ? success((await response.json<{ items: RetentionRule[] }>()).items) : failure(response);
  });
}

export function createRetentionRule(projectId: string, rule: NewRetentionRule): Promise<ActionResult<RetentionRule>> {
  return attempt(async () => {
    const response = await http.post(`${project(projectId)}/retention-rules`, { json: rule });
    return response.ok ? success(await response.json<RetentionRule>()) : failure(response);
  });
}

export function endRetentionRule(projectId: string, id: string): Promise<ActionResult<RetentionRule>> {
  return attempt(async () => {
    const response = await http.post(`${project(projectId)}/retention-rules/${encodeURIComponent(id)}/end`);
    return response.ok ? success(await response.json<RetentionRule>()) : failure(response);
  });
}

export function listContests(projectId: string, recordId: string): Promise<ActionResult<Contest[]>> {
  return attempt(async () => {
    const response = await http.get(`${log(projectId)}/${encodeURIComponent(recordId)}/contests`);
    return response.ok ? success((await response.json<{ items: Contest[] }>()).items) : failure(response);
  });
}

export function openContest(projectId: string, recordId: string, reason: string): Promise<ActionResult<Contest>> {
  return attempt(async () => {
    const response = await http.post(`${log(projectId)}/${encodeURIComponent(recordId)}/contests`, {
      json: { reason },
    });
    return response.ok ? success(await response.json<Contest>()) : failure(response);
  });
}

export function reviewContest(projectId: string, id: string, review: ReviewValues): Promise<ActionResult<Contest>> {
  return attempt(async () => {
    const response = await http.post(`${project(projectId)}/contests/${encodeURIComponent(id)}/review`, {
      json: {
        status: review.status,
        review: review.review,
        revisedOutcome: review.status === 'revised' ? review.revisedOutcome : null,
      },
    });
    return response.ok ? success(await response.json<Contest>()) : failure(response);
  });
}
