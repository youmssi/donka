'use client';

import { UserPlus } from 'lucide-react';
import { useLocale, useTranslations } from 'next-intl';
import { useState } from 'react';
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
import { FieldGroup } from '@/components/ui/field';
import { routing, type Locale } from '@/i18n/routing';

import { inviteSchema, type InviteValues } from './schema';
import { useInvite } from './usePeople';

const LANGUAGE_NAMES: Record<Locale, 'english' | 'french'> = { en: 'english', fr: 'french' };

export function InviteDialog() {
  const t = useTranslations('people');
  const common = useTranslations('common');
  const locale = useLocale() as Locale;
  const invite = useInvite();
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<ActionError | null>(null);

  const defaultValues: InviteValues = { email: '', locale, isAdmin: false };
  const form = useAppForm({
    defaultValues,
    validators: { onChange: inviteSchema, onSubmit: inviteSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await invite.mutateAsync(value);
      if (result.ok) {
        onOpenChange(false);
        toast.success(t('invited', { email: result.data.email }));
      } else {
        setError(result.error);
      }
    },
  });

  function onOpenChange(next: boolean) {
    setOpen(next);
    if (!next) {
      form.reset();
      setError(null);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        <Button>
          <UserPlus aria-hidden />
          {t('invite')}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('inviteTitle')}</DialogTitle>
          <DialogDescription>{t('inviteDescription')}</DialogDescription>
        </DialogHeader>
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error ? <ErrorAlert error={error} /> : null}
            <FieldGroup className="gap-4">
              <form.AppField name="email">
                {(field) => <field.TextField label={t('email')} type="email" autoComplete="off" required autoFocus />}
              </form.AppField>
              <form.AppField name="locale">
                {(field) => (
                  <field.SelectField
                    label={t('language')}
                    options={routing.locales.map((code) => ({ value: code, label: common(LANGUAGE_NAMES[code]) }))}
                  />
                )}
              </form.AppField>
              <form.AppField name="isAdmin">
                {(field) => <field.CheckboxField label={t('isAdmin')} hint={t('isAdminHint')} />}
              </form.AppField>
            </FieldGroup>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                {common('cancel')}
              </Button>
              <form.SubmitButton pendingLabel={t('sending')}>{t('send')}</form.SubmitButton>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}
