'use client';

import { FileArchive, Package } from 'lucide-react';
import { useLocale, useTranslations } from 'next-intl';
import { useId, useState, type ChangeEvent } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Field, FieldContent, FieldDescription, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
import { Skeleton } from '@/components/ui/skeleton';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useRouter } from '@/i18n/navigation';
import { keyFromName, newProjectSchema, projectHome } from '@/modules/project';

import { testOutcome } from './imported';
import { NameAndKey, newProjectOptions } from './name-and-key';
import { PackSummary } from './pack-summary';
import { localized, type Pack } from './schema';
import { useImportPack, useInspectPack, usePacks } from './usePacks';

type Tab = 'catalogue' | 'file';

/** A new project from a pack of the installation's catalogue, or from a pack file (administrators). */
export function ImportPackDialog() {
  const t = useTranslations('packs');
  const common = useTranslations('common');
  const locale = useLocale();
  const router = useRouter();
  const id = useId();
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<Tab>('catalogue');
  const [chosen, setChosen] = useState<Pack | null>(null);
  const [file, setFile] = useState<File | null>(null);
  const [fileError, setFileError] = useState<ActionError | null>(null);
  const [error, setError] = useState<ActionError | null>(null);
  const [keyEdited, setKeyEdited] = useState(false);
  const packs = usePacks(open);
  const inspect = useInspectPack();
  const importPack = useImportPack();

  const form = useAppForm({
    ...newProjectOptions,
    validators: { onChange: newProjectSchema, onSubmit: newProjectSchema },
    onSubmit: async ({ value }) => {
      const source = tab === 'catalogue' ? chosen?.key : file;
      if (!chosen || !source) return;
      setError(null);
      const result = await importPack.mutateAsync({ pack: source, values: value });
      if (!result.ok) {
        setError(result.error);
        return;
      }
      const { passed, failing } = testOutcome(result.data);
      const name = result.data.project.name;
      if (failing) toast.warning(t('importedFailing', { name, failing }));
      else toast.success(t('imported', { name, passed }));
      onOpenChange(false);
      router.push(projectHome(result.data.project.key));
    },
  });

  /** Proposes the pack's name and key, unless the person typed their own. */
  function choose(pack: Pack | null) {
    const proposed = chosen ? localized(chosen.name, locale) : '';
    setChosen(pack);
    setError(null);
    if (!pack) return;
    const name = form.getFieldValue('name');
    if (!name || name === proposed)
      form.setFieldValue('name', localized(pack.name, locale), { dontRunListeners: true });
    if (!keyEdited) form.setFieldValue('key', keyFromName(pack.key), { dontRunListeners: true });
  }

  function onTabChange(next: string) {
    setTab(next as Tab);
    choose(null);
    setFile(null);
    setFileError(null);
  }

  async function onFile(event: ChangeEvent<HTMLInputElement>) {
    const picked = event.target.files?.[0] ?? null;
    setFile(picked);
    setFileError(null);
    choose(null);
    if (!picked) return;
    const result = await inspect.mutateAsync(picked);
    if (result.ok) choose(result.data);
    else setFileError(result.error);
  }

  function onOpenChange(next: boolean) {
    setOpen(next);
    if (!next) {
      form.reset();
      setTab('catalogue');
      setChosen(null);
      setFile(null);
      setFileError(null);
      setError(null);
      setKeyEdited(false);
    }
  }

  const catalogue = packs.data;
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        <Button variant="outline">
          <Package aria-hidden />
          {t('fromPack')}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[90dvh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t('importTitle')}</DialogTitle>
          <DialogDescription>{t('importDescription')}</DialogDescription>
        </DialogHeader>
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error ? <ErrorAlert error={error} /> : null}
            <Tabs value={tab} onValueChange={onTabChange}>
              <TabsList className="w-full">
                <TabsTrigger value="catalogue">{t('catalogue')}</TabsTrigger>
                <TabsTrigger value="file">{t('file')}</TabsTrigger>
              </TabsList>
              <TabsContent value="catalogue" className="mt-2">
                {!catalogue ? (
                  <div className="grid gap-2" aria-busy>
                    <Skeleton className="h-20" />
                    <Skeleton className="h-20" />
                  </div>
                ) : !catalogue.ok ? (
                  <ErrorAlert error={catalogue.error} />
                ) : catalogue.data.length === 0 ? (
                  <Empty className="border border-dashed">
                    <EmptyHeader>
                      <EmptyMedia variant="icon">
                        <Package />
                      </EmptyMedia>
                      <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
                      <EmptyDescription>{t('empty')}</EmptyDescription>
                    </EmptyHeader>
                  </Empty>
                ) : (
                  <RadioGroup
                    aria-label={t('catalogue')}
                    value={chosen?.key ?? ''}
                    onValueChange={(key) => choose(catalogue.data.find((pack) => pack.key === key) ?? null)}
                  >
                    {catalogue.data.map((pack) => (
                      <FieldLabel key={pack.key} htmlFor={`${id}-${pack.key}`}>
                        <Field orientation="horizontal">
                          <FieldContent>
                            <PackSummary pack={pack} />
                          </FieldContent>
                          <RadioGroupItem value={pack.key} id={`${id}-${pack.key}`} />
                        </Field>
                      </FieldLabel>
                    ))}
                  </RadioGroup>
                )}
              </TabsContent>
              <TabsContent value="file" className="mt-2 grid gap-3">
                <Field className="gap-2">
                  <FieldLabel htmlFor={`${id}-file`}>{t('fileLabel')}</FieldLabel>
                  <Input id={`${id}-file`} type="file" accept=".zip,application/zip" onChange={(e) => void onFile(e)} />
                  <FieldDescription>{t('fileHint')}</FieldDescription>
                </Field>
                {inspect.isPending ? <Skeleton className="h-20" /> : null}
                {fileError ? <ErrorAlert error={fileError} /> : null}
                {file && chosen ? (
                  <div className="flex gap-3 rounded-md border p-3">
                    <FileArchive aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
                    <PackSummary pack={chosen} />
                  </div>
                ) : null}
              </TabsContent>
            </Tabs>
            <NameAndKey form={form} keyEdited={keyEdited} onKeyEdited={() => setKeyEdited(true)} />
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                {common('cancel')}
              </Button>
              <form.SubmitButton pendingLabel={t('importing')} disabled={!chosen}>
                {t('import')}
              </form.SubmitButton>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}
