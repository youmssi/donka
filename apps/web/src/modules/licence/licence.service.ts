import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type { Licence } from './schema';

/** What the installation's licence allows (DNK-77). */
export function getLicence(): Promise<ActionResult<Licence>> {
  return attempt(async () => {
    const response = await http.get('licence');
    return response.ok ? success(await response.json<Licence>()) : failure(response);
  });
}
