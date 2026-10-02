'use client';

import {
  GraphNode,
  useDecisionGraphActions,
  useDecisionGraphState,
  type CustomNodeSpecification,
} from '@gorules/jdm-editor';
import { useForm, type AnyFieldApi } from '@tanstack/react-form';
import { PlugZap, Settings2 } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useId, useState } from 'react';

import { TextField } from '@/components/shared/form/text-field';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Field, FieldDescription, FieldGroup, FieldLabel, FieldLegend, FieldSet } from '@/components/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
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
  type ConnectorValues,
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

  const form = useForm({
    defaultValues: connectorValues(config),
    validators: { onChange: connectorSchema, onSubmit: connectorSchema },
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
        <form
          noValidate
          className="flex min-h-0 flex-1 flex-col"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          <div className="min-h-0 flex-1 overflow-y-auto p-4">
            <FieldGroup className="gap-6">
              <FieldSet className="gap-1">
                <FieldLegend>{t('service')}</FieldLegend>
                <form.Field name="preset">
                  {(field) => (
                    <SelectField
                      field={field}
                      label={t('presetLabel')}
                      hint={t('presetHint')}
                      options={PRESETS.map((value) => ({ value, label: t(`preset.${value}`) }))}
                      disabled={disabled}
                      onPick={(value) => applyPreset(value as Preset)}
                    />
                  )}
                </form.Field>
                <form.Field name="url">
                  {(field) => (
                    <TextField
                      field={field}
                      label={t('url')}
                      hint={t('urlHint')}
                      placeholder="https://"
                      inputMode="url"
                      className="font-mono"
                      disabled={disabled}
                    />
                  )}
                </form.Field>
              </FieldSet>

              <FieldSet className="gap-1">
                <FieldLegend>{t('authentication')}</FieldLegend>
                <form.Field name="authType">
                  {(field) => (
                    <SelectField
                      field={field}
                      label={t('authType')}
                      options={AUTH_TYPES.map((value) => ({ value, label: t(`auth.${value}`) }))}
                      disabled={disabled}
                    />
                  )}
                </form.Field>
                <form.Subscribe selector={(state) => state.values.authType}>
                  {(authType) =>
                    authType === 'none' ? null : (
                      <>
                        {authType === 'header' ? (
                          <form.Field name="header">
                            {(field) => (
                              <TextField
                                field={field}
                                label={t('header')}
                                placeholder="X-Api-Key"
                                className="font-mono"
                                disabled={disabled}
                              />
                            )}
                          </form.Field>
                        ) : null}
                        <form.Field name="secret">
                          {(field) => (
                            <TextField
                              field={field}
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
                        </form.Field>
                      </>
                    )
                  }
                </form.Subscribe>
              </FieldSet>

              <FieldSet className="gap-1">
                <FieldLegend>{t('requestResponse')}</FieldLegend>
                <form.Field name="body">
                  {(field) => (
                    <TextField
                      field={field}
                      label={t('body')}
                      hint={t('bodyHint', { example: '{{ applicant.nationalId }}' })}
                      multiline
                      rows={5}
                      className="font-mono text-xs"
                      spellCheck={false}
                      disabled={disabled}
                    />
                  )}
                </form.Field>
                <form.Field name="outputKey">
                  {(field) => (
                    <TextField
                      field={field}
                      label={t('outputKey')}
                      hint={t('outputKeyHint')}
                      className="font-mono"
                      disabled={disabled}
                    />
                  )}
                </form.Field>
              </FieldSet>

              <FieldSet className="gap-1">
                <FieldLegend>{t('reliability')}</FieldLegend>
                <div className="grid gap-x-3 sm:grid-cols-2">
                  <form.Field name="timeoutMs">
                    {(field) => (
                      <TextField
                        field={field}
                        label={t('timeout')}
                        hint={t('timeoutHint')}
                        inputMode="numeric"
                        messageValues={{ max: TIMEOUT_MAX }}
                        disabled={disabled}
                      />
                    )}
                  </form.Field>
                  <form.Field name="retries">
                    {(field) => (
                      <TextField
                        field={field}
                        label={t('retries')}
                        hint={t('retriesHint')}
                        inputMode="numeric"
                        messageValues={{ max: RETRIES_MAX }}
                        disabled={disabled}
                      />
                    )}
                  </form.Field>
                </div>
                <form.Field name="onError">
                  {(field) => (
                    <SelectField
                      field={field}
                      label={t('onError')}
                      options={ON_ERROR.map((value) => ({ value, label: t(`onErrorOption.${value}`) }))}
                      disabled={disabled}
                    />
                  )}
                </form.Field>
                <form.Subscribe selector={(state) => state.values.onError}>
                  {(onError) =>
                    onError === 'fallback' ? (
                      <form.Field name="fallback">
                        {(field) => (
                          <TextField
                            field={field}
                            label={t('fallback')}
                            hint={t('fallbackHint')}
                            multiline
                            rows={3}
                            className="font-mono text-xs"
                            spellCheck={false}
                            disabled={disabled}
                          />
                        )}
                      </form.Field>
                    ) : null
                  }
                </form.Subscribe>
              </FieldSet>

              <FieldSet className="gap-1">
                <FieldLegend>{t('simulation')}</FieldLegend>
                <form.Field name="mock">
                  {(field) => (
                    <TextField
                      field={field}
                      label={t('mock')}
                      hint={t('mockHint')}
                      multiline
                      rows={4}
                      className="font-mono text-xs"
                      spellCheck={false}
                      disabled={disabled}
                    />
                  )}
                </form.Field>
              </FieldSet>
            </FieldGroup>
          </div>
          <SheetFooter className="flex-row justify-end border-t">
            <Button type="button" variant="outline" onClick={onClose}>
              {disabled ? common('close') : common('cancel')}
            </Button>
            {disabled ? null : <Button type="submit">{t('apply')}</Button>}
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}

/** A choice bound to a TanStack Form field, laid out like {@link TextField}. */
function SelectField({
  field,
  label,
  hint,
  options,
  disabled,
  onPick,
}: {
  field: AnyFieldApi;
  label: string;
  hint?: string;
  options: { value: string; label: string }[];
  disabled: boolean;
  onPick?: (value: string) => void;
}) {
  const id = useId();
  return (
    <Field className="gap-2 pb-5">
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select
        value={String(field.state.value)}
        disabled={disabled}
        onValueChange={(value) => {
          field.handleChange(value as ConnectorValues[keyof ConnectorValues]);
          onPick?.(value);
        }}
      >
        <SelectTrigger id={id} className="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {hint ? <FieldDescription>{hint}</FieldDescription> : null}
    </Field>
  );
}
