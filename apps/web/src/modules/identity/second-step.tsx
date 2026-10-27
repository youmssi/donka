'use client';

import { Check, Copy, Download } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';
import { encode } from 'uqr';

import type { ActionError, ActionResult } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { ShownOnceToken } from '@/components/shared/shown-once-token';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Label } from '@/components/ui/label';
import { Skeleton } from '@/components/ui/skeleton';

import { codeSchema, type CodeValues, type Enrolment } from './schema';

/**
 * Asks for a code and submits it. `recovery` says a recovery code is accepted
 * too, which changes the label and drops the digits-only keyboard.
 */
export function CodeForm({
  submit,
  submitLabel,
  recovery = false,
  autoFocus = true,
  onError,
}: {
  submit: (values: CodeValues) => Promise<ActionError | null>;
  submitLabel: string;
  recovery?: boolean;
  autoFocus?: boolean;
  onError?: (error: ActionError) => void;
}) {
  const t = useTranslations('secondStep');
  const [error, setError] = useState<ActionError | null>(null);
  const form = useAppForm({
    defaultValues: { code: '' } satisfies CodeValues,
    validators: { onSubmit: codeSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const failed = await submit(value);
      if (failed) {
        setError(failed);
        onError?.(failed);
        form.setFieldValue('code', '');
      }
    },
  });

  return (
    <form.AppForm>
      <form.Form className="grid gap-4">
        {error ? <ErrorAlert error={error} /> : null}
        <form.AppField name="code">
          {(field) => (
            <field.TextField
              label={recovery ? t('codeOrRecovery') : t('code')}
              hint={recovery ? t('codeOrRecoveryHint') : undefined}
              autoComplete="one-time-code"
              inputMode={recovery ? 'text' : 'numeric'}
              spellCheck={false}
              className="font-mono"
              required
              autoFocus={autoFocus}
            />
          )}
        </form.AppField>
        <form.SubmitButton pendingLabel={t('checking')} className="w-full">
          {submitLabel}
        </form.SubmitButton>
      </form.Form>
    </form.AppForm>
  );
}

/** The key as a QR code, drawn as SVG so it follows the page and never leaves the browser. */
export function QrCode({ value, label }: { value: string; label: string }) {
  const qr = useMemo(() => encode(value, { ecc: 'M', border: 2 }), [value]);
  const cells: string[] = [];
  qr.data.forEach((row, y) =>
    row.forEach((dark, x) => {
      if (dark) cells.push(`M${x} ${y}h1v1h-1z`);
    }),
  );
  return (
    <svg
      role="img"
      aria-label={label}
      viewBox={`0 0 ${qr.size} ${qr.size}`}
      className="size-48 rounded-md bg-white p-1"
      shapeRendering="crispEdges"
    >
      <path d={cells.join('')} fill="#000" />
    </svg>
  );
}

/**
 * Setting up a second step: start (QR code and key), then confirm with a first
 * code. `confirm` returns the recovery codes.
 */
export function EnrolmentSteps({
  start,
  confirm,
  onConfirmed,
}: {
  start: () => Promise<ActionResult<Enrolment>>;
  confirm: (values: CodeValues) => Promise<ActionResult<string[]>>;
  onConfirmed: (codes: string[]) => void;
}) {
  const t = useTranslations('secondStep');
  const [enrolment, setEnrolment] = useState<Enrolment | null>(null);
  const [error, setError] = useState<ActionError | null>(null);

  useEffect(() => {
    let cancelled = false;
    void start().then((result) => {
      if (cancelled) return;
      if (result.ok) setEnrolment(result.data);
      else setError(result.error);
    });
    return () => {
      cancelled = true;
    };
    // Start once per mount: starting again would replace the key being scanned.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (error) return <ErrorAlert error={error} />;
  if (!enrolment) return <Skeleton className="h-72 w-full" />;

  return (
    <div className="grid gap-4">
      <ol className="grid list-decimal gap-1 pl-5 text-sm text-muted-foreground">
        <li>{t('stepApp')}</li>
        <li>{t('stepScan')}</li>
        <li>{t('stepCode')}</li>
      </ol>
      <div className="flex justify-center">
        <QrCode value={enrolment.otpauthUrl} label={t('qrLabel')} />
      </div>
      <ShownOnceToken label={t('keyLabel')} token={enrolment.key} />
      <CodeForm
        submitLabel={t('turnOn')}
        autoFocus={false}
        submit={async (values) => {
          const result = await confirm(values);
          if (!result.ok) return result.error;
          onConfirmed(result.data);
          return null;
        }}
      />
    </div>
  );
}

/** Recovery codes, shown once: the person copies or downloads them, then confirms. */
export function RecoveryCodes({
  codes,
  onDone,
  doneLabel,
}: {
  codes: string[];
  onDone: () => void;
  doneLabel: string;
}) {
  const t = useTranslations('secondStep');
  const [saved, setSaved] = useState(false);
  const [copied, setCopied] = useState(false);
  const text = codes.join('\n');

  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      toast.success(t('codesCopied'));
    } catch {
      toast.error(t('codesCopyFailed'));
    }
  }

  function download() {
    const url = URL.createObjectURL(new Blob([`${t('codesFileHeader')}\n\n${text}\n`], { type: 'text/plain' }));
    const link = document.createElement('a');
    link.href = url;
    link.download = 'donka-recovery-codes.txt';
    link.click();
    URL.revokeObjectURL(url);
  }

  return (
    <div className="grid gap-4">
      <p className="text-sm text-muted-foreground">{t('codesIntro', { count: codes.length })}</p>
      <ul
        aria-label={t('codesLabel')}
        className="grid grid-cols-2 gap-2 rounded-md border bg-muted/40 p-3 font-mono text-sm"
      >
        {codes.map((code) => (
          <li key={code} className="text-center">
            {code}
          </li>
        ))}
      </ul>
      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="outline" size="sm" onClick={() => void copy()}>
          {copied ? <Check aria-hidden /> : <Copy aria-hidden />}
          {t('copyCodes')}
        </Button>
        <Button type="button" variant="outline" size="sm" onClick={download}>
          <Download aria-hidden />
          {t('downloadCodes')}
        </Button>
      </div>
      <div className="flex items-center gap-2">
        <Checkbox id="codes-saved" checked={saved} onCheckedChange={(value) => setSaved(value === true)} />
        <Label htmlFor="codes-saved">{t('codesSaved')}</Label>
      </div>
      <Button type="button" disabled={!saved} onClick={onDone}>
        {doneLabel}
      </Button>
    </div>
  );
}
