import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type { Updates } from './schema';

/** Whether a newer Donka release is published (administrators only, DNK-64). */
export function getUpdates(): Promise<ActionResult<Updates>> {
  return attempt(async () => {
    const response = await http.get('updates');
    return response.ok ? success(await response.json<Updates>()) : failure(response);
  });
}
