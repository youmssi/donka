'use client';

import { OutcomeSettings } from '@/modules/decision-log';
import { ProjectSettingsPage } from '@/modules/project';
import { CiTokens } from '@/modules/release';

/** The project's settings with the decision log's and the CI tokens, composed here so no module imports another. */
export function Settings() {
  return (
    <ProjectSettingsPage
      more={(project) => (
        <>
          <OutcomeSettings project={project} />
          <CiTokens project={project} />
        </>
      )}
    />
  );
}
