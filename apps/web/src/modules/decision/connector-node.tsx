'use client';

import {
  GraphNode,
  useDecisionGraphActions,
  useDecisionGraphState,
  type CustomNodeSpecification,
} from '@gorules/jdm-editor';
import { formOptions } from '@tanstack/react-form';
import { PlugZap, Settings2 } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import { useAppForm, withForm } from '@/components/shared/form';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { FieldGroup, FieldLegend, FieldSet } from '@/components/ui/field';
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from '@/components/ui/sheet';

import {
  AUTH_TYPES,
  CONNECTOR_KIND,
  connectorConfig,
  connectorSchema,
  connectorValues,
  ON_ERROR,
  PRESETS,
  presetValues,
  RETRIES_MAX,
  SECRET_ENV_PREFIX,
  TIMEOUT_MAX,
  urlHost,
  validSecretName,
  type ConnectorConfig,
  type Preset,
} from './connector';

export interface ConnectorNodeLabels {
  displayName: string;
  shortDescription: string;
}

/**
 * The connector node (`customNode` of kind `donka.connector`), given to the
 * editor through its `customNodes` extension point. The canvas shows what it
 * calls; its settings open in a sheet.
 */
export function connectorNodeSpecification(
  labels: ConnectorNodeLabels,
): CustomNodeSpecification<Partial<ConnectorConfig>, typeof CONNECTOR_KIND> {
  return {
    kind: CONNECTOR_KIND,
    displayName: labels.displayName,
    shortDescription: labels.shortDescription,
    icon: <PlugZap size={16} aria-hidden />,
    generateNode: ({ index }) => ({ name: `connector${index}`, config: { preset: 'bureau-score' } }),
    renderNode: ({ specification, id, selected, data }) => (
      <GraphNode id={id} specification={specification} name={data.name} isSelected={selected}>
        <ConnectorSummary id={id} />
      </GraphNode>
    ),
  };
}

/** A node's settings, read from the graph's store (the canvas only hands renderers its name). */
function useConnectorConfig(id: string): Partial<ConnectorConfig> | undefined {
  return useDecisionGraphState(
    ({ decisionGraph }) =>
      (decisionGraph.nodes.find((node) => node.id === id)?.content as { config?: Partial<ConnectorConfig> } | undefined)
        ?.config,
  );
}

function ConnectorSummary({ id }: { id: string }) {
  const t = useTranslations('connector');
  const config = useConnectorConfig(id);
  const [open, setOpen] = useState(false);
  const preset = config?.preset === 'bureau-score' ? 'bureau-score' : 'http';
  return (
    <div className="grid min-w-0 gap-1.5 text-xs">
      <Badge variant="secondary" className="justify-self-start">
        {t(`preset.${preset}`)}
      </Badge>
      {config?.url ? (
        <span className="min-w-0 truncate font-mono" title={config.url}>
          {urlHost(config.url)}
        </span>
      ) : (
        <span className="text-muted-foreground">{t('notSetUp')}</span>
      )}
      {config?.outputKey ? (
        <span className="text-muted-foreground">{t('addsTo', { key: config.outputKey })}</span>
      ) : null}
      <Button variant="outline" size="xs" className="nodrag justify-self-start" onClick={() => setOpen(true)}>
        <Settings2 aria-hidden />
        {t('settings')}
      </Button>
      {open ? <ConnectorSheet id={id} config={config} onClose={() => setOpen(false)} /> : null}
    </div>
  );
}

function ConnectorSheet({
  id,
  config,
  onClose,
}: {
  id: string;
  config: Partial<ConnectorConfig> | undefined;
  onClose: () => void;
}) {
  const t = useTranslations('connector');
  const common = useTranslations('common');
  const { updateNode } = useDecisionGraphActions();
  const disabled = useDecisionGraphState(({ disabled }) => disabled === true);
  const name = useDecisionGraphState(({ decisionGraph }) => decisionGraph.nodes.find((node) => node.id === id)?.name);

  const form = useAppForm({
    ...connectorForm,
    defaultValues: connectorValues(config),
    onSubmit: ({ value }) => {
      updateNode(id, (draft) => {
        draft.content = { kind: CONNECTOR_KIND, config: connectorConfig(value) };
        return draft;
      });
      onClose();
    },
  });

  function applyPreset(preset: Preset) {
    const values = presetValues(preset);
    for (const key of Object.keys(values) as (keyof typeof values)[]) {
      form.setFieldValue(key, values[key]);
    }
  }

  return (
    <Sheet open onOpenChange={(open) => (open ? null : onClose())}>
      <SheetContent className="w-full gap-0 sm:max-w-lg">
        <SheetHeader className="border-b">
          <SheetTitle>{t('title')}</SheetTitle>
          <SheetDescription>{name ? t('description', { name }) : null}</SheetDescription>
        </SheetHeader>
        <form.AppForm>
          <form.Form className="flex min-h-0 flex-1 flex-col">
            <div className="min-h-0 flex-1 overflow-y-auto p-4">
              <FieldGroup className="gap-6">
                <ServiceSection form={form} disabled={disabled} onPreset={applyPreset} />
                <AuthenticationSection form={form} disabled={disabled} />
                <RequestSection form={form} disabled={disabled} />
                <ReliabilitySection form={form} disabled={disabled} />
                <SimulationSection form={form} disabled={disabled} />
              </FieldGroup>
            </div>
            <SheetFooter className="flex-row justify-end border-t">
              <Button type="button" variant="outline" onClick={onClose}>
                {disabled ? common('close') : common('cancel')}
              </Button>
              {disabled ? null : <Button type="submit">{t('apply')}</Button>}
            </SheetFooter>
          </form.Form>
        </form.AppForm>
      </SheetContent>
    </Sheet>
  );
}

/** What the sections below share with the sheet's form: its fields and their rules. */
const connectorForm = formOptions({
  defaultValues: connectorValues(undefined),
  validators: { onChange: connectorSchema, onSubmit: connectorSchema },
});

const ServiceSection = withForm({
  ...connectorForm,
  props: {
    disabled: false,
    onPreset: undefined as ((preset: Preset) => void) | undefined,
  },
  render: function Render({ form, disabled, onPreset }) {
    const t = useTranslations('connector');
    return (
      <FieldSet className="gap-1">
        <FieldLegend>{t('service')}</FieldLegend>
        <form.AppField name="preset">
          {(field) => (
            <field.SelectField
              className="pb-5"
              label={t('presetLabel')}
              hint={t('presetHint')}
              options={PRESETS.map((value) => ({ value, label: t(`preset.${value}`) }))}
              disabled={disabled}
              onPick={(value) => onPreset?.(value as Preset)}
            />
          )}
        </form.AppField>
        <form.AppField name="url">
          {(field) => (
            <field.TextField
              label={t('url')}
              hint={t('urlHint')}
              placeholder="https://"
              inputMode="url"
              className="font-mono"
              disabled={disabled}
            />
          )}
        </form.AppField>
      </FieldSet>
    );
  },
});

const AuthenticationSection = withForm({
  ...connectorForm,
  props: { disabled: false },
  render: function Render({ form, disabled }) {
    const t = useTranslations('connector');
    return (
      <FieldSet className="gap-1">
        <FieldLegend>{t('authentication')}</FieldLegend>
        <form.AppField name="authType">
          {(field) => (
            <field.SelectField
              className="pb-5"
              label={t('authType')}
              options={AUTH_TYPES.map((value) => ({ value, label: t(`auth.${value}`) }))}
              disabled={disabled}
            />
          )}
        </form.AppField>
        <form.Subscribe selector={(state) => state.values.authType}>
          {(authType) =>
            authType === 'none' ? null : (
              <>
                {authType === 'header' ? (
                  <form.AppField name="header">
                    {(field) => (
                      <field.TextField
                        label={t('header')}
                        placeholder="X-Api-Key"
                        className="font-mono"
                        disabled={disabled}
                      />
                    )}
                  </form.AppField>
                ) : null}
                <form.AppField name="secret">
                  {(field) => (
                    <field.TextField
                      label={t('secret')}
                      hint={t('secretHint', {
                        variable: `${SECRET_ENV_PREFIX}${validSecretName(String(field.state.value)) ? String(field.state.value) : 'NAME'}`,
                      })}
                      placeholder="BUREAU_API_KEY"
                      className="font-mono"
                      autoComplete="off"
                      spellCheck={false}
                      disabled={disabled}
                    />
                  )}
                </form.AppField>
              </>
            )
          }
        </form.Subscribe>
      </FieldSet>
    );
  },
});

const RequestSection = withForm({
  ...connectorForm,
  props: { disabled: false },
  render: function Render({ form, disabled }) {
    const t = useTranslations('connector');
    return (
      <FieldSet className="gap-1">
        <FieldLegend>{t('requestResponse')}</FieldLegend>
        <form.AppField name="body">
          {(field) => (
            <field.TextField
              label={t('body')}
              hint={t('bodyHint', { example: '{{ applicant.nationalId }}' })}
              multiline
              rows={5}
              className="font-mono text-xs"
              spellCheck={false}
              disabled={disabled}
            />
          )}
        </form.AppField>
        <form.AppField name="outputKey">
          {(field) => (
            <field.TextField
              label={t('outputKey')}
              hint={t('outputKeyHint')}
              className="font-mono"
              disabled={disabled}
            />
          )}
        </form.AppField>
      </FieldSet>
    );
  },
});

const ReliabilitySection = withForm({
  ...connectorForm,
  props: { disabled: false },
  render: function Render({ form, disabled }) {
    const t = useTranslations('connector');
    return (
      <FieldSet className="gap-1">
        <FieldLegend>{t('reliability')}</FieldLegend>
        <div className="grid gap-x-3 sm:grid-cols-2">
          <form.AppField name="timeoutMs">
            {(field) => (
              <field.TextField
                label={t('timeout')}
                hint={t('timeoutHint')}
                inputMode="numeric"
                messageValues={{ max: TIMEOUT_MAX }}
                disabled={disabled}
              />
            )}
          </form.AppField>
          <form.AppField name="retries">
            {(field) => (
              <field.TextField
                label={t('retries')}
                hint={t('retriesHint')}
                inputMode="numeric"
                messageValues={{ max: RETRIES_MAX }}
                disabled={disabled}
              />
            )}
          </form.AppField>
        </div>
        <form.AppField name="onError">
          {(field) => (
            <field.SelectField
              className="pb-5"
              label={t('onError')}
              options={ON_ERROR.map((value) => ({ value, label: t(`onErrorOption.${value}`) }))}
              disabled={disabled}
            />
          )}
        </form.AppField>
        <form.Subscribe selector={(state) => state.values.onError}>
          {(onError) =>
            onError === 'fallback' ? (
              <form.AppField name="fallback">
                {(field) => (
                  <field.TextField
                    label={t('fallback')}
                    hint={t('fallbackHint')}
                    multiline
                    rows={3}
                    className="font-mono text-xs"
                    spellCheck={false}
                    disabled={disabled}
                  />
                )}
              </form.AppField>
            ) : null
          }
        </form.Subscribe>
      </FieldSet>
    );
  },
});

const SimulationSection = withForm({
  ...connectorForm,
  props: { disabled: false },
  render: function Render({ form, disabled }) {
    const t = useTranslations('connector');
    return (
      <FieldSet className="gap-1">
        <FieldLegend>{t('simulation')}</FieldLegend>
        <form.AppField name="mock">
          {(field) => (
            <field.TextField
              label={t('mock')}
              hint={t('mockHint')}
              multiline
              rows={4}
              className="font-mono text-xs"
              spellCheck={false}
              disabled={disabled}
            />
          )}
        </form.AppField>
      </FieldSet>
    );
  },
});
