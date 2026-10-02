import { readErrorBody } from './client';

/**
 * Error codes the web app has a translated message for (`errors.<code>` in the
 * catalogs). NETWORK and UNEXPECTED are the web app's own.
 */
export const KNOWN_ERROR_CODES = [
  'INVALID_CREDENTIALS',
  'INVALID_SETUP_LINK',
  'UNAUTHENTICATED',
  'FORBIDDEN',
  'EMAIL_TAKEN',
  'DATABASE_UNAVAILABLE',
  'INVALID_REQUEST',
  'PROJECT_NOT_FOUND',
  'PROJECT_KEY_TAKEN',
  'PROJECT_ARCHIVED',
  'ALREADY_MEMBER',
  'MEMBER_NOT_FOUND',
  'LAST_OWNER',
  'NO_SUCH_USER',
  'DECISION_NOT_FOUND',
  'DECISION_KEY_TAKEN',
  'DECISION_CONFLICT',
  'INVALID_DECISION',
  'EVALUATION_FAILED',
  'VERSION_NOT_FOUND',
  'VERSION_UNCHANGED',
  'SCENARIO_NOT_FOUND',
  'SCENARIO_NAME_TAKEN',
  'RELEASE_NOT_FOUND',
  'RELEASE_CONFLICT',
  'UNVERSIONED_DECISIONS',
  'NOTHING_TO_RELEASE',
  'APPROVAL_REQUIRED',
  'NOT_RETRYABLE',
  'DEPLOYMENT_NOT_FOUND',
  'ENVIRONMENT_NOT_FOUND',
  'TOKEN_NOT_FOUND',
  'APPROVAL_NOT_FOUND',
  'NOT_ON_STAGING',
  'NO_APPROVER',
  'APPROVAL_PENDING',
  'SELF_APPROVAL',
  'APPROVAL_DECIDED',
  'NOT_REQUESTER',
  'NETWORK',
  'UNEXPECTED',
] as const;

export type ErrorCode = (typeof KNOWN_ERROR_CODES)[number];

export interface ActionError {
  /** Key of the message to show, under `errors` in the catalogs. */
  code: ErrorCode;
  /** Quote it when reporting a problem; absent when the server was not reached. */
  requestId?: string;
  /** Fields the server refused, by field name. */
  fieldErrors?: Record<string, string>;
  /** Machine-readable context of some errors (who saved first, the engine's error…). */
  details?: unknown;
}

/** What every service call returns: expected failures are values, never exceptions. */
export type ActionResult<T> = { ok: true; data: T } | { ok: false; error: ActionError };

export const success = <T>(data: T): ActionResult<T> => ({ ok: true, data });

function isKnown(code: string): code is ErrorCode {
  return (KNOWN_ERROR_CODES as readonly string[]).includes(code);
}

/** Turns a failed response into an ActionError the UI can translate. */
export async function failure(response: Response): Promise<{ ok: false; error: ActionError }> {
  const body = await readErrorBody(response);
  const code = body && isKnown(body.code) ? body.code : 'UNEXPECTED';
  return {
    ok: false,
    error: {
      code,
      requestId: body?.requestId ?? response.headers.get('x-request-id') ?? undefined,
      fieldErrors: body?.fields ?? undefined,
      details: body?.details ?? undefined,
    },
  };
}

/** Runs a request, mapping "server unreachable" (network error, timeout) to NETWORK. */
export async function attempt<T>(run: () => Promise<ActionResult<T>>): Promise<ActionResult<T>> {
  try {
    return await run();
  } catch {
    return { ok: false, error: { code: 'NETWORK' } };
  }
}
