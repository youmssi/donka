'use client';

import { CircleAlert, Eye, FileQuestion } from 'lucide-react';
import dynamic from 'next/dynamic';
import { useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { useCallback, useMemo } from 'react';

import { ErrorAlert } from '@/components/shared/error-alert';
import { When } from '@/components/shared/format';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Skeleton } from '@/components/ui/skeleton';
import { Spinner } from '@/components/ui/spinner';
import { Link } from '@/i18n/navigation';
import { projectHome, ProjectFrame, type Project } from '@/modules/project';

import { canEditDecisions } from './decisions-page';
import { simulateDecision } from './decision.service';
import { splitKey, type Decision } from './schema';
import { useDecision, useDecisionList, useDecisionSaved } from './useDecisions';
import { useDraft, type DraftStatus } from './useDraft';
import { HistorySheet, SaveVersionDialog, VersionChip } from './versions';

// antd and the engine's WASM load only here, and only in the browser (ADR-004).
const JdmGraph = dynamic(() => import('./jdm-graph').then((module) => module.JdmGraph), {
  ssr: false,
  loading: () => <Skeleton className="size-full rounded-lg" />,
});

/** The editor fills the page: minus the header (3.5rem), page padding (3rem) and toolbar (3rem). */
const EDITOR_HEIGHT = 'h-[calc(100dvh-9.5rem)]';

export function DecisionEditorPage() {
  return (
    <ProjectFrame section="decision" header={() => null}>
      {(project) => <EditorLoader project={project} />}
    </ProjectFrame>
  );
}

/** The decision from `?d=<key>`: found in the project's list, then loaded with its draft. */
function EditorLoader({ project }: { project: Project }) {
  const t = useTranslations('decisions');
  const common = useTranslations('common');
  const key = useSearchParams().get('d') ?? '';
  const list = useDecisionList(project.id);
  const id = list.data?.ok ? list.data.data.find((decision) => decision.key === key)?.id : undefined;
  const decision = useDecision(project.id, id);
  // Stable between renders: the editor rebuilds its node types when this changes.
  const listed = list.data?.ok ? list.data.data : null;
  const others = useMemo(() => (listed ?? []).map((d) => d.key).filter((k) => k !== key), [listed, key]);

  const failed = list.data && !list.data.ok ? list.data : decision.data && !decision.data.ok ? decision.data : null;
  if (failed) {
    return (
      <ErrorAlert
        error={failed.error}
        title={t('errorTitle')}
        action={
          <Button variant="outline" size="sm" className="mt-2" onClick={() => void list.refetch()}>
            {common('retry')}
          </Button>
        }
      />
    );
  }
  if (list.data?.ok && !id) {
    return (
      <Empty className="border border-dashed">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <FileQuestion />
          </EmptyMedia>
          <EmptyTitle>{t('notFoundTitle')}</EmptyTitle>
          <EmptyDescription>{t('notFound', { key })}</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button asChild variant="outline">
            <Link href={projectHome(project.key)}>{t('backToDecisions')}</Link>
          </Button>
        </EmptyContent>
      </Empty>
    );
  }
  if (!decision.data?.ok) return <Skeleton className={`${EDITOR_HEIGHT} w-full rounded-lg`} />;
  // Keyed by the decision: opening another one starts a fresh draft.
  return <Editor key={decision.data.data.id} project={project} decision={decision.data.data} others={others} />;
}

function Editor({ project, decision, others }: { project: Project; decision: Decision; others: string[] }) {
  const t = useTranslations('decisions');
  const errors = useTranslations('errors');
  const onSaved = useDecisionSaved(project.id);
  const editable = canEditDecisions(project);
  const draft = useDraft(project.id, decision, editable, onSaved);
  const { folder, name } = splitKey(decision.key);

  const decisionNodeLabels = useMemo(
    () => ({ displayName: t('nodeName'), shortDescription: t('nodeDescription'), choose: t('nodeChoose') }),
    [t],
  );

  const simulate = useCallback(
    (graph: unknown, context: unknown) => simulateDecision(project.id, decision.id, graph, context),
    [project.id, decision.id],
  );

  return (
    <div className="grid gap-3">
      <div className="flex min-h-9 flex-wrap items-center gap-x-3 gap-y-1">
        <h1 className="font-mono text-base">
          <span className="text-muted-foreground">{folder}</span>
          <span className="font-semibold">{name}</span>
        </h1>
        <VersionChip {...draft.version} />
        <SaveStatus status={draft.status} onRetry={draft.retry} />
        <div className="ml-auto flex items-center gap-2">
          <HistorySheet
            projectId={project.id}
            decision={decision}
            draft={draft}
            editable={editable}
            callable={others}
            decisionNodeLabels={decisionNodeLabels}
          />
          {editable ? <SaveVersionDialog projectId={project.id} decision={decision} draft={draft} /> : null}
        </div>
      </div>
      <div className={`${EDITOR_HEIGHT} min-h-96 overflow-hidden rounded-lg border`}>
        <JdmGraph
          value={draft.graph}
          onChange={draft.change}
          disabled={!editable}
          simulatorTitle={t('simulator')}
          callable={others}
          decisionNodeLabels={decisionNodeLabels}
          simulate={simulate}
          failureMessage={(result) => errors(result.error.code)}
        />
      </div>
      <ConflictDialog status={draft.status} onKeepMine={draft.keepMine} onLoadTheirs={draft.loadTheirs} />
    </div>
  );
}

/** Where the draft stands; silent when all is saved, clear when it is not. */
function SaveStatus({ status, onRetry }: { status: DraftStatus; onRetry: () => void }) {
  const t = useTranslations('decisions');
  const errors = useTranslations('errors');
  const common = useTranslations('common');
  return (
    <p className="flex items-center gap-2 text-sm text-muted-foreground" role="status" aria-live="polite">
      {status.kind === 'saved' ? (
        <>
          {t('saved')} <When value={status.at} as="ago" />
        </>
      ) : status.kind === 'dirty' ? (
        t('unsaved')
      ) : status.kind === 'saving' ? (
        <>
          <Spinner /> {t('saving')}
        </>
      ) : status.kind === 'readOnly' ? (
        <Badge variant="secondary">
          <Eye aria-hidden />
          {t('readOnly')}
        </Badge>
      ) : status.kind === 'error' ? (
        <span className="flex items-center gap-2 text-destructive">
          <CircleAlert className="size-4" aria-hidden />
          {t('notSaved', { reason: errors(status.error.code) })}
          <Button variant="outline" size="xs" onClick={onRetry}>
            {common('retry')}
          </Button>
        </span>
      ) : (
        <span className="text-destructive">{t('conflictShort')}</span>
      )}
    </p>
  );
}

/** Someone else saved while this person was editing: they choose, nothing is lost silently. */
function ConflictDialog({
  status,
  onKeepMine,
  onLoadTheirs,
}: {
  status: DraftStatus;
  onKeepMine: () => void;
  onLoadTheirs: () => Promise<void>;
}) {
  const t = useTranslations('decisions');
  const conflict = status.kind === 'conflict' ? status.conflict : null;
  return (
    <AlertDialog open={status.kind === 'conflict'}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t('conflictTitle')}</AlertDialogTitle>
          <AlertDialogDescription>
            {conflict?.updatedBy
              ? t.rich('conflictBy', {
                  email: conflict.updatedBy.email,
                  when: () => <When value={conflict.updatedAt} as="ago" />,
                })
              : t('conflict')}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={() => void onLoadTheirs()}>{t('loadTheirs')}</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={onKeepMine}>
            {t('keepMine')}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
