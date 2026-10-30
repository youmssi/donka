'use client';

import { SecondStepSetup, useCurrentUser } from '@/modules/identity';
import { FirstRunAssistant } from '@/modules/installation';
import { PackPicker } from '@/modules/pack';

/** The first-run assistant, with the second step and the packs from their own modules. */
export function Assistant() {
  const user = useCurrentUser();
  return (
    <FirstRunAssistant
      isAdmin={user.isAdmin}
      secondStep={(next, label) => <SecondStepSetup onDone={next} doneLabel={label} />}
      packs={(next, label) => <PackPicker onDone={next} doneLabel={label} />}
    />
  );
}
