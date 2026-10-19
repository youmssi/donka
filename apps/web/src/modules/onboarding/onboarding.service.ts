import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type { Onboarding, Tour } from './schema';

export function getOnboarding(): Promise<ActionResult<Onboarding>> {
  return attempt(async () => {
    const response = await http.get('onboarding');
    return response.ok ? success(await response.json<Onboarding>()) : failure(response);
  });
}

export function listToursSeen(): Promise<ActionResult<Tour[]>> {
  return attempt(async () => {
    const response = await http.get('me/tours');
    return response.ok ? success((await response.json<{ seen: Tour[] }>()).seen) : failure(response);
  });
}

export function markTourSeen(tour: Tour): Promise<ActionResult<null>> {
  return attempt(async () => {
    const response = await http.put(`me/tours/${encodeURIComponent(tour)}`);
    return response.ok ? success(null) : failure(response);
  });
}
