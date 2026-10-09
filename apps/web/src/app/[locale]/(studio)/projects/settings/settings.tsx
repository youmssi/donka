'use client';

import { OutcomeSettings } from '@/modules/decision-log';
import { CopyProject } from '@/modules/pack';
import { ProjectSettingsPage, type Project } from '@/modules/project';
import { CiTokens, useReleaseList } from '@/modules/release';

/** The project's settings with the decision log's, the CI tokens and copies, composed here so no module imports another. */
export function Settings() {
  return (
    <ProjectSettingsPage
      more={(project) => (
        <>
          <OutcomeSettings project={project} />
          <CiTokens project={project} />
          <Copies project={project} />
        </>
      )}
    />
  );
}

/** Copies and exports start from the drafts or one of the latest releases. */
function Copies({ project }: { project: Project }) {
  const releases = useReleaseList(project.id, 0);
  const items = releases.data?.ok ? releases.data.data.items : [];
  return <CopyProject project={project} releases={items} />;
}
