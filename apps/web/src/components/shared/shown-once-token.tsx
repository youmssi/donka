'use client';

import { Check, Copy } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useId, useState } from 'react';
import { toast } from 'sonner';

import { Field, FieldLabel } from '@/components/ui/field';
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '@/components/ui/input-group';

/** A token Studio shows once, read-only, with a way to copy it. */
export function ShownOnceToken({ label, token }: { label: string; token: string }) {
  const t = useTranslations('common');
  const [copied, setCopied] = useState(false);
  const id = useId();

  async function copy() {
    try {
      await navigator.clipboard.writeText(token);
      setCopied(true);
      toast.success(t('tokenCopied'));
    } catch {
      toast.error(t('tokenCopyFailed'));
    }
  }

  return (
    <Field className="gap-1.5">
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <InputGroup>
        <InputGroupInput
          id={id}
          readOnly
          value={token}
          className="font-mono text-xs"
          onFocus={(e) => e.target.select()}
        />
        <InputGroupAddon align="inline-end">
          <InputGroupButton
            size="icon-xs"
            aria-label={t('copyToken')}
            title={t('copyToken')}
            onClick={() => void copy()}
          >
            {copied ? <Check aria-hidden /> : <Copy aria-hidden />}
          </InputGroupButton>
        </InputGroupAddon>
      </InputGroup>
    </Field>
  );
}
