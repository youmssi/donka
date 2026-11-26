import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { useMemo } from 'react';

import { useCurrentTour, usePageTour, type PageTour } from '@/components/shared/tour';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';

import { useStoryMessages } from './fixtures';

function TourPreview() {
  const m = useStoryMessages();
  const tour = useMemo<PageTour>(
    () => ({
      id: 'component-preview',
      steps: [
        { element: '#preview-tour-project', title: m.projects.title, description: m.projects.emptyAdmin },
        { element: '#preview-tour-release', title: m.releases.title, description: m.releases.empty },
      ],
    }),
    [m],
  );
  usePageTour(tour);
  const current = useCurrentTour();
  return (
    <div className="grid max-w-xl gap-4">
      <Button disabled={!current.available} onClick={current.start}>
        {m.help.tour}
      </Button>
      <Card id="preview-tour-project">
        <CardHeader>
          <CardTitle>{m.projects.title}</CardTitle>
        </CardHeader>
        <CardContent>{m.projects.emptyAdmin}</CardContent>
      </Card>
      <Card id="preview-tour-release">
        <CardHeader>
          <CardTitle>{m.releases.title}</CardTitle>
        </CardHeader>
        <CardContent>{m.releases.empty}</CardContent>
      </Card>
    </div>
  );
}

const meta = { title: 'Shared/Tour', component: TourPreview } satisfies Meta<typeof TourPreview>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Replayable: Story = {};
