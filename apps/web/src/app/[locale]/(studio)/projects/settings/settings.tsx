'use client';

import { OutcomeSettings } from '@/modules/decision-log';
import { ProjectSettingsPage } from '@/modules/project';

/** The project's settings with the decision log's, composed here so neither module imports the other. */
export function Settings() {
  return <ProjectSettingsPage more={(project) => <OutcomeSettings project={project} />} />;
}
