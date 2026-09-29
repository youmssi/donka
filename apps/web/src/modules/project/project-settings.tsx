'use client';

import { useForm } from '@tanstack/react-form';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
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
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { FieldGroup } from '@/components/ui/field';
import { Spinner } from '@/components/ui/spinner';

import { ProjectFrame } from './project-frame';
import { DESCRIPTION_MAX, NAME_MAX, projectDetailsSchema, type Project } from './schema';
import { useSetArchived, useUpdateProject } from './useProjects';

export function ProjectSettingsPage() {
  return <ProjectFrame section="settings">{(project) => <Settings project={project} />}</ProjectFrame>;
}

function Settings({ project }: { project: Project }) {
  const isOwner = project.role === 'owner';
  return (
    <div className="grid items-start gap-6 lg:grid-cols-3">
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

  const form = useForm({
    defaultValues: { name: project.name, description: project.description },
    validators: { onBlur: projectDetailsSchema, onSubmit: projectDetailsSchema },
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
        <form
          noValidate
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          {error ? <ErrorAlert error={error} /> : null}
          <FieldGroup className="gap-2">
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
          </FieldGroup>
          {editable ? (
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <SubmitButton pending={isSubmitting} pendingLabel={common('saving')} className="w-fit">
                  {common('save')}
                </SubmitButton>
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
