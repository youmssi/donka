'use client';

import { SetupPasswordForm } from '@/modules/identity';
import { useInstallation } from '@/modules/installation';

/** During the first run, the first administrator goes on to the first-run assistant. */
export function SetupPassword() {
  const result = useInstallation().data;
  const firstRun = result?.ok === true && result.data.firstRun;
  return <SetupPasswordForm next={firstRun ? '/first-run' : undefined} />;
}
