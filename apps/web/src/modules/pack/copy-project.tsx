'use client';

import { Copy, Download } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Field, FieldDescription, FieldLabel } from '@/components/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Spinner } from '@/components/ui/spinner';
import { useRouter } from '@/i18n/navigation';
import { useCurrentUser } from '@/modules/identity';
import { newProjectSchema, projectHome, type Project } from '@/modules/project';

import { testOutcome } from './imported';
import { NameAndKey, newProjectOptions } from './name-and-key';
import { DRAFTS, releaseOf } from './schema';
import { useDuplicateProject, useExportProject } from './usePacks';

export interface ReleaseChoice {
  id: string;
  version: string;
}

/**
 * Copy the project into a new one (administrators) or export it as a pack file (owners), from
 * its drafts or from a release. Neither carries releases, tokens, decision records or members.
 */
export function CopyProject({ project, releases }: { project: Project; releases: ReleaseChoice[] }) {
  const t = useTranslations('packs');
  const user = useCurrentUser();
  const [source, setSource] = useState(DRAFTS);
  const canExport = project.role === 'owner';
  if (!user.isAdmin && !canExport) return null;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('copyTitle')}</CardTitle>
        <CardDescription>{t('copyDescription')}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <Field className="gap-2">
          <FieldLabel htmlFor={`${project.id}-source`}>{t('source')}</FieldLabel>
          <Select value={source} onValueChange={setSource}>
            <SelectTrigger id={`${project.id}-source`} className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={DRAFTS}>{t('drafts')}</SelectItem>
              {releases.map((release) => (
                <SelectItem key={release.id} value={release.id}>
                  {t('release', { version: release.version })}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <FieldDescription>{t('sourceHint')}</FieldDescription>
        </Field>
        <div className="flex flex-wrap gap-2">
          {user.isAdmin ? <DuplicateDialog project={project} source={source} /> : null}
          {canExport ? <ExportButton project={project} source={source} /> : null}
        </div>
      </CardContent>
    </Card>
  );
}

function DuplicateDialog({ project, source }: { project: Project; source: string }) {
  const t = useTranslations('packs');
  const common = useTranslations('common');
  const router = useRouter();
  const duplicate = useDuplicateProject(project.id);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<ActionError | null>(null);
  const [keyEdited, setKeyEdited] = useState(false);

  const form = useAppForm({
    ...newProjectOptions,
    validators: { onChange: newProjectSchema, onSubmit: newProjectSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await duplicate.mutateAsync({ values: value, releaseId: releaseOf(source) });
      if (!result.ok) {
        setError(result.error);
        return;
      }
      const { passed, failing } = testOutcome(result.data);
      const name = result.data.project.name;
      if (failing) toast.warning(t('importedFailing', { name, failing }));
      else toast.success(t('duplicated', { name, passed }));
      onOpenChange(false);
      router.push(projectHome(result.data.project.key));
    },
  });

  function onOpenChange(next: boolean) {
    setOpen(next);
    if (!next) {
      form.reset();
      setError(null);
      setKeyEdited(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        <Button variant="outline">
          <Copy aria-hidden />
          {t('duplicate')}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('duplicateTitle', { name: project.name })}</DialogTitle>
          <DialogDescription>{t('duplicateDescription')}</DialogDescription>
        </DialogHeader>
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error ? <ErrorAlert error={error} /> : null}
            <NameAndKey form={form} keyEdited={keyEdited} onKeyEdited={() => setKeyEdited(true)} />
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                {common('cancel')}
              </Button>
              <form.SubmitButton pendingLabel={t('duplicating')}>{t('duplicate')}</form.SubmitButton>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}

function ExportButton({ project, source }: { project: Project; source: string }) {
  const t = useTranslations('packs');
  const exportProject = useExportProject(project.id);
  const [error, setError] = useState<ActionError | null>(null);

  async function run() {
    setError(null);
    const result = await exportProject.mutateAsync(releaseOf(source));
    if (!result.ok) {
      setError(result.error);
      return;
    }
    const url = URL.createObjectURL(result.data.blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = result.data.name;
    link.click();
    URL.revokeObjectURL(url);
    toast.success(t('exported', { file: result.data.name }));
  }

  return (
    <>
      <Button variant="outline" disabled={exportProject.isPending} onClick={() => void run()}>
        {exportProject.isPending ? <Spinner /> : <Download aria-hidden />}
        {exportProject.isPending ? t('exporting') : t('export')}
      </Button>
      {error ? (
        <div className="basis-full">
          <ErrorAlert error={error} />
        </div>
      ) : null}
    </>
  );
}
