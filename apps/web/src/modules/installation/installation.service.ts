import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type { Installation, OrganisationValues } from './schema';

/** Whether the first-run assistant is done, and the organisation's name (anyone). */
export function getInstallation(): Promise<ActionResult<Installation>> {
  return attempt(async () => {
    const response = await http.get('installation');
    return response.ok ? success(await response.json<Installation>()) : failure(response);
  });
}

/** Finishes the first-run assistant (an administrator with a second sign-in step). */
export function finishFirstRun(values: OrganisationValues): Promise<ActionResult<Installation>> {
  return attempt(async () => {
    const response = await http.post('installation/first-run', { json: values });
    return response.ok ? success(await response.json<Installation>()) : failure(response);
  });
}

/** Renames the organisation (administrators). */
export function renameOrganisation(values: OrganisationValues): Promise<ActionResult<Installation>> {
  return attempt(async () => {
    const response = await http.put('installation/organisation', { json: values });
    return response.ok ? success(await response.json<Installation>()) : failure(response);
  });
}
