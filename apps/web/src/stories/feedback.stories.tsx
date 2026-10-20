import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { CircleCheck, CircleAlert, LoaderCircle } from 'lucide-react';

import { ErrorAlert } from '@/components/shared/error-alert';
import { PageSkeleton } from '@/components/shared/page-skeleton';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from '@/components/ui/empty';
import { Spinner } from '@/components/ui/spinner';

import { useStoryMessages } from './fixtures';

function FeedbackPreview({ kind }: { kind: 'loading' | 'empty' | 'error' | 'success' | 'rate-limit' | 'pack-error' }) {
  const m = useStoryMessages();
  return (
    <div className="grid max-w-2xl gap-4">
      {kind === 'loading' ? (
        <>
          <PageSkeleton />
          <Button disabled>
            <Spinner />
            {m.common.loading}
          </Button>
        </>
      ) : null}
      {kind === 'empty' ? (
        <Empty>
          <EmptyHeader>
            <EmptyTitle>{m.projects.emptyTitle}</EmptyTitle>
            <EmptyDescription>{m.projects.emptyAdmin}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : null}
      {kind === 'error' ? (
        <ErrorAlert
          error={{ code: 'UNEXPECTED', requestId: 'preview-reference' }}
          title={m.projects.errorTitle}
          action={<Button variant="outline">{m.common.retry}</Button>}
        />
      ) : null}
      {kind === 'rate-limit' ? (
        <ErrorAlert error={{ code: 'RATE_LIMITED', details: { retryAfterSeconds: 42 } }} />
      ) : null}
      {kind === 'pack-error' ? (
        <ErrorAlert error={{ code: 'INVALID_PACK', details: { reason: 'decisions/order-discount.json' } }} />
      ) : null}
      {kind === 'success' ? (
        <>
          <Alert>
            <CircleCheck aria-hidden />
            <AlertTitle>{m.project.saved}</AlertTitle>
            <AlertDescription>{m.projects.description}</AlertDescription>
          </Alert>
          <div className="flex flex-wrap gap-2">
            <Badge>{m.roles.owner}</Badge>
            <Badge variant="secondary">{m.roles.editor}</Badge>
            <Badge variant="outline">{m.roles.viewer}</Badge>
            <Badge variant="destructive">
              <CircleAlert aria-hidden />
              {m.decisionLog.failed}
            </Badge>
            <Badge variant="secondary">
              <LoaderCircle aria-hidden />
              {m.common.loading}
            </Badge>
          </div>
        </>
      ) : null}
    </div>
  );
}

const meta = { title: 'Shared/Feedback', component: FeedbackPreview, args: { kind: 'success' } } satisfies Meta<
  typeof FeedbackPreview
>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Loading: Story = { args: { kind: 'loading' } };
export const EmptyState: Story = { args: { kind: 'empty' } };
export const ErrorState: Story = { args: { kind: 'error' } };
export const Success: Story = {};
export const RateLimit: Story = { args: { kind: 'rate-limit' } };
export const PackError: Story = { args: { kind: 'pack-error' } };
