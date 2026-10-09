'use client';

import { useTranslations } from 'next-intl';
import { useState, type ReactNode } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
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
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { FieldGroup } from '@/components/ui/field';
import { Spinner } from '@/components/ui/spinner';

import { ProjectFrame } from './project-frame';
import { DESCRIPTION_MAX, NAME_MAX, projectDetailsSchema, type Project } from './schema';
import { useSetArchived, useUpdateProject } from './useProjects';

/** The project's settings; `more` adds other modules' settings (the decision log's). */
export function ProjectSettingsPage({ more }: { more?: (project: Project) => ReactNode }) {
  return <ProjectFrame section="settings">{(project) => <Settings project={project} more={more} />}</ProjectFrame>;
}

function Settings({ project, more }: { project: Project; more?: (project: Project) => ReactNode }) {
  const isOwner = project.role === 'owner';
  return (
    <div className="grid items-start gap-6 lg:grid-cols-3">
      {/* Keyed by the archive state: archiving or restoring starts the form afresh. */}
      <Details key={project.archivedAt ?? 'active'} project={project} />
      {isOwner ? <ArchiveSection project={project} /> : null}
      {more?.(project)}
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

  const form = useAppForm({
    defaultValues: { name: project.name, description: project.description },
    validators: { onChange: projectDetailsSchema, onSubmit: projectDetailsSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await update.mutateAsync(value);
      if (result.ok) toast.success(t('saved'));
      else setError(result.error);
    },
  });

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t('detailsTitle')}</CardTitle>
        <CardDescription>
          {editable ? t('detailsDescription') : isOwner ? t('archivedDetails') : t('readOnlyDetails')}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error ? <ErrorAlert error={error} /> : null}
            <FieldGroup className="gap-2">
              <form.AppField name="name">
                {(field) => (
                  <field.TextField
                    label={fields('name')}
                    required
                    disabled={!editable}
                    messageValues={{ max: NAME_MAX }}
                  />
                )}
              </form.AppField>
              <form.AppField name="description">
                {(field) => (
                  <field.TextField
                    label={fields('description')}
                    multiline
                    disabled={!editable}
                    messageValues={{ max: DESCRIPTION_MAX }}
                  />
                )}
              </form.AppField>
            </FieldGroup>
            {editable ? (
              <form.SubmitButton pendingLabel={common('saving')} className="w-fit">
                {common('save')}
              </form.SubmitButton>
            ) : null}
          </form.Form>
        </form.AppForm>
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
    if (result.ok) toast.success(next ? t('archived') : t('restored'));
    else setError(result.error);
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
            {setArchived.isPending ? <Spinner /> : null}
            {setArchived.isPending ? t('restoring') : t('restore')}
          </Button>
        ) : (
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="outline" className="w-fit" disabled={setArchived.isPending}>
                {setArchived.isPending ? <Spinner /> : null}
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
