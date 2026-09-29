'use client';

import { useForm } from '@tanstack/react-form';
import { CircleCheck } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { TextField } from '@/components/shared/form/text-field';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';

import { ProjectFrame } from './project-frame';
import { DESCRIPTION_MAX, NAME_MAX, projectDetailsSchema, type Project } from './schema';
import { useSetArchived, useUpdateProject } from './useProjects';

export function ProjectSettingsPage() {
  return <ProjectFrame tab="settings">{(project) => <Settings project={project} />}</ProjectFrame>;
}

function Settings({ project }: { project: Project }) {
  const isOwner = project.role === 'owner';
  return (
    <div className="grid gap-6">
      {/* Keyed by the archive state: archiving or restoring starts the form afresh. */}
      <Details key={project.archivedAt ?? 'active'} project={project} />
      {isOwner ? <ArchiveSection project={project} /> : null}
    </div>
  );
}

function Details({ project }: { project: Project }) {
  const isOwner = project.role === 'owner';
  const editable = isOwner && !project.archivedAt;
  const t = useTranslations('project');
  const fields = useTranslations('projects');
  const common = useTranslations('common');
  const update = useUpdateProject(project.id);
  const [error, setError] = useState<ActionError | null>(null);
  const [saved, setSaved] = useState(false);

  const form = useForm({
    defaultValues: { name: project.name, description: project.description },
    validators: { onBlur: projectDetailsSchema, onSubmit: projectDetailsSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      setSaved(false);
      const result = await update.mutateAsync(value);
      if (result.ok) setSaved(true);
      else setError(result.error);
    },
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('detailsTitle')}</CardTitle>
        <CardDescription>
          {editable ? t('detailsDescription') : isOwner ? t('archivedDetails') : t('readOnlyDetails')}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form
          noValidate
          className="grid max-w-xl gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          {error ? <ErrorAlert error={error} /> : null}
          {saved ? (
            <Alert variant="success" aria-live="polite">
              <CircleCheck aria-hidden />
              <AlertDescription>{t('saved')}</AlertDescription>
            </Alert>
          ) : null}
          <form.Field name="name">
            {(field) => (
              <TextField
                field={field}
                label={fields('name')}
                required
                disabled={!editable}
                messageValues={{ max: NAME_MAX }}
              />
            )}
          </form.Field>
          <form.Field name="description">
            {(field) => (
              <TextField
                field={field}
                label={fields('description')}
                multiline
                disabled={!editable}
                messageValues={{ max: DESCRIPTION_MAX }}
              />
            )}
          </form.Field>
          {editable ? (
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <Button type="submit" disabled={isSubmitting} className="w-fit">
                  {isSubmitting ? common('saving') : common('save')}
                </Button>
              )}
            </form.Subscribe>
          ) : null}
        </form>
      </CardContent>
    </Card>
  );
}

function ArchiveSection({ project }: { project: Project }) {
  const t = useTranslations('project');
  const common = useTranslations('common');
  const setArchived = useSetArchived(project.id);
  const [error, setError] = useState<ActionError | null>(null);
  const archived = Boolean(project.archivedAt);

  async function apply(next: boolean) {
    setError(null);
    const result = await setArchived.mutateAsync(next);
    if (!result.ok) setError(result.error);
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{archived ? t('restoreTitle') : t('archiveTitle')}</CardTitle>
        <CardDescription>{archived ? t('restoreDescription') : t('archiveDescription')}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        {error ? <ErrorAlert error={error} /> : null}
        {archived ? (
          <Button
            variant="outline"
            className="w-fit"
            disabled={setArchived.isPending}
            onClick={() => void apply(false)}
          >
            {setArchived.isPending ? t('restoring') : t('restore')}
          </Button>
        ) : (
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="outline" className="w-fit" disabled={setArchived.isPending}>
                {setArchived.isPending ? t('archiving') : t('archive')}
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>{t('archiveConfirmTitle', { name: project.name })}</AlertDialogTitle>
                <AlertDialogDescription>{t('archiveConfirm')}</AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
                <AlertDialogAction onClick={() => void apply(true)}>{t('archive')}</AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        )}
      </CardContent>
    </Card>
  );
}
