'use client';

import { ArrowRight, Check } from 'lucide-react';
import { useTranslations } from 'next-intl';

import { cn } from '@/components/shared/utils';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Link } from '@/i18n/navigation';
import { useCurrentUser } from '@/modules/identity';
import { ImportPackDialog } from '@/modules/pack';
import { projectHref, type ProjectSection } from '@/modules/project';

import type { Step } from './schema';
import { useOnboarding } from './useOnboarding';

/** Where each step after the first is done, in the project the checklist leads to. */
const WHERE: Record<Exclude<Step, 'project'>, ProjectSection> = {
  simulation: 'decisions',
  version: 'decisions',
  staging: 'releases',
  token: 'environments',
};

/**
 * The way from an empty Studio to a decision a Runtime answers, read from what really happened
 * in the person's projects. It disappears once every step is done.
 */
export function GetStarted() {
  const t = useTranslations('onboarding');
  const user = useCurrentUser();
  const result = useOnboarding().data;
  if (!result?.ok || result.data.complete) return null;
  const { steps, project } = result.data;
  const done = steps.filter((step) => step.done).length;
  // The first step not done yet is the one to do now.
  const next = steps.find((step) => !step.done)?.step;

  return (
    <Card aria-labelledby="get-started-title">
      <CardHeader>
        <CardTitle id="get-started-title" role="heading" aria-level={2}>
          {t('title')}
        </CardTitle>
        <CardDescription>
          {t('description')} {t('progress', { done, total: steps.length })}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <ol className="grid gap-3">
          {steps.map(({ step, done }, index) => (
            <li key={step} className="flex items-start gap-3">
              <span
                className={cn(
                  'mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full border text-xs font-medium',
                  done && 'border-primary bg-primary text-primary-foreground',
                )}
                aria-hidden
              >
                {done ? <Check className="size-3.5" /> : index + 1}
              </span>
              <div className="grid min-w-0 flex-1 gap-1">
                <p className={cn('text-sm font-medium', done && 'text-muted-foreground line-through')}>
                  {t(`${step}.title`)}
                  <span className="sr-only">{done ? t('done') : t('toDo')}</span>
                </p>
                {done ? null : <p className="text-sm text-muted-foreground">{t(`${step}.description`)}</p>}
                {done || step !== next ? null : step === 'project' ? (
                  user.isAdmin ? (
                    <div>
                      <ImportPackDialog />
                    </div>
                  ) : (
                    <p className="text-sm">{t('project.askAdmin')}</p>
                  )
                ) : project ? (
                  <Button asChild size="sm" variant="outline" className="w-fit">
                    <Link href={projectHref(WHERE[step], project.key)}>
                      {t(`${step}.action`, { project: project.name })}
                      <ArrowRight aria-hidden />
                    </Link>
                  </Button>
                ) : null}
              </div>
            </li>
          ))}
        </ol>
      </CardContent>
    </Card>
  );
}
