'use client';

import { useCurrentUser } from '@/modules/identity';
import { FirstRunNotice } from '@/modules/installation';
import { GetStarted } from '@/modules/onboarding';

/** Above the projects: finishing the first run for administrators, then the Get started checklist. */
export function HomeIntro() {
  const user = useCurrentUser();
  return (
    <>
      <FirstRunNotice isAdmin={user.isAdmin} />
      <GetStarted />
    </>
  );
}
