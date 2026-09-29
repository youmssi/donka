'use client';

import { useForm } from '@tanstack/react-form';
import { UserPlus } from 'lucide-react';
import { useLocale, useTranslations } from 'next-intl';
import { useId, useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Field, FieldContent, FieldDescription, FieldGroup, FieldLabel } from '@/components/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { routing, type Locale } from '@/i18n/routing';

import { inviteSchema, type InviteValues } from './schema';
import { useInvite } from './usePeople';

const LANGUAGE_NAMES: Record<Locale, 'english' | 'french'> = { en: 'english', fr: 'french' };

export function InviteDialog() {
  const t = useTranslations('people');
  const common = useTranslations('common');
  const locale = useLocale() as Locale;
  const invite = useInvite();
  const languageId = useId();
  const adminId = useId();
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<ActionError | null>(null);

  const form = useForm({
    defaultValues: { email: '', locale, isAdmin: false } as InviteValues,
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
        <form
          noValidate
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          {error ? <ErrorAlert error={error} /> : null}
          <FieldGroup className="gap-4">
            <form.Field name="email">
              {(field) => (
                <TextField field={field} label={t('email')} type="email" autoComplete="off" required autoFocus />
              )}
            </form.Field>
            <form.Field name="locale">
              {(field) => (
                <Field className="gap-2">
                  <FieldLabel htmlFor={languageId}>{t('language')}</FieldLabel>
                  <Select value={field.state.value} onValueChange={(value) => field.handleChange(value as Locale)}>
                    <SelectTrigger id={languageId} className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {routing.locales.map((code) => (
                        <SelectItem key={code} value={code}>
                          {common(LANGUAGE_NAMES[code])}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
              )}
            </form.Field>
            <form.Field name="isAdmin">
              {(field) => (
                <Field orientation="horizontal">
                  <Checkbox
                    id={adminId}
                    checked={field.state.value}
                    onCheckedChange={(checked) => field.handleChange(checked === true)}
                    aria-describedby={`${adminId}-hint`}
                  />
                  <FieldContent>
                    <FieldLabel htmlFor={adminId}>{t('isAdmin')}</FieldLabel>
                    <FieldDescription id={`${adminId}-hint`}>{t('isAdminHint')}</FieldDescription>
                  </FieldContent>
                </Field>
              )}
            </form.Field>
          </FieldGroup>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {common('cancel')}
            </Button>
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <SubmitButton pending={isSubmitting} pendingLabel={t('sending')}>
                  {t('send')}
                </SubmitButton>
              )}
            </form.Subscribe>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
