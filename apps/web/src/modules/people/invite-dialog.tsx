'use client';

import { useForm } from '@tanstack/react-form';
import { UserPlus } from 'lucide-react';
import { useLocale, useTranslations } from 'next-intl';
import { useId, useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
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
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { routing, type Locale } from '@/i18n/routing';

import { inviteSchema, type InviteValues } from './schema';
import { useInvite } from './usePeople';

const LANGUAGE_NAMES: Record<Locale, 'english' | 'french'> = { en: 'english', fr: 'french' };

export function InviteDialog({ onInvited }: { onInvited: (email: string) => void }) {
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
    validators: { onBlur: inviteSchema, onSubmit: inviteSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await invite.mutateAsync(value);
      if (result.ok) {
        onOpenChange(false);
        onInvited(result.data.email);
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
      <DialogContent closeLabel={common('close')}>
        <DialogHeader>
          <DialogTitle>{t('inviteTitle')}</DialogTitle>
          <DialogDescription>{t('inviteDescription')}</DialogDescription>
        </DialogHeader>
        <form
          noValidate
          className="grid gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          {error ? <ErrorAlert error={error} /> : null}
          <form.Field name="email">
            {(field) => (
              <TextField field={field} label={t('email')} type="email" autoComplete="off" required autoFocus />
            )}
          </form.Field>
          <form.Field name="locale">
            {(field) => (
              <div className="grid gap-1.5 pb-5">
                <Label htmlFor={languageId}>{t('language')}</Label>
                <Select value={field.state.value} onValueChange={(value) => field.handleChange(value as Locale)}>
                  <SelectTrigger id={languageId}>
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
              </div>
            )}
          </form.Field>
          <form.Field name="isAdmin">
            {(field) => (
              <div className="flex items-start gap-3 pb-4">
                <Checkbox
                  id={adminId}
                  checked={field.state.value}
                  onCheckedChange={(checked) => field.handleChange(checked === true)}
                  aria-describedby={`${adminId}-hint`}
                  className="mt-0.5"
                />
                <div className="grid gap-1">
                  <Label htmlFor={adminId}>{t('isAdmin')}</Label>
                  <p id={`${adminId}-hint`} className="text-xs text-muted-foreground">
                    {t('isAdminHint')}
                  </p>
                </div>
              </div>
            )}
          </form.Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {common('cancel')}
            </Button>
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <Button type="submit" disabled={isSubmitting}>
                  {isSubmitting ? t('sending') : t('send')}
                </Button>
              )}
            </form.Subscribe>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
