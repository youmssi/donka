'use client';

import { CircleAlert } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import type { RefusedConnector } from './schema';

/** Connectors the installation's Runtime would refuse to call; the version was still saved. */
export function RefusedConnectors({
  number,
  connectors,
  onDismiss,
}: {
  number: number;
  connectors: RefusedConnector[];
  onDismiss: () => void;
}) {
  const t = useTranslations('connector');
  return (
    <Alert role="status">
      <CircleAlert aria-hidden />
      <AlertTitle>{t('refusedTitle', { number })}</AlertTitle>
      <AlertDescription>
        <ul className="list-disc ps-4">
          {connectors.map((connector) => (
            <li key={`${connector.node}-${connector.host}-${connector.secret ?? ''}`}>
              {connector.secret
                ? t('refusedSecret', { node: connector.node, secret: connector.secret, host: connector.host })
                : t('refusedHost', { node: connector.node, host: connector.host })}
            </li>
          ))}
        </ul>
        <p className="mt-1">{t('refusedFix')}</p>
        <div className="mt-2 flex gap-2">
          <Button size="sm" variant="ghost" onClick={onDismiss}>
            {t('refusedDismiss')}
          </Button>
        </div>
      </AlertDescription>
    </Alert>
  );
}
