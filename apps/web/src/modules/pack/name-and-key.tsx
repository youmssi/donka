'use client';

import { formOptions } from '@tanstack/react-form';
import { useTranslations } from 'next-intl';

import { withForm } from '@/components/shared/form';
import { FieldGroup } from '@/components/ui/field';
import { KEY_RULES, keyFromName, NAME_MAX, type NewProjectValues } from '@/modules/project';

export const newProjectOptions = formOptions({ defaultValues: { name: '', key: '' } satisfies NewProjectValues });

/**
 * The new project's name and key. The key follows the name until the person edits it
 * (`keyEdited`, owned by the dialog so it resets with the form).
 */
export const NameAndKey = withForm({
  ...newProjectOptions,
  props: { keyEdited: false, onKeyEdited: () => {} },
  render: function Render({ form, keyEdited, onKeyEdited }) {
    const t = useTranslations('projects');
    return (
      <FieldGroup className="gap-2">
        <form.AppField
          name="name"
          listeners={{
            onChange: ({ value }) => {
              if (!keyEdited) form.setFieldValue('key', keyFromName(value), { dontRunListeners: true });
            },
          }}
        >
          {(field) => <field.TextField label={t('name')} required messageValues={{ max: NAME_MAX }} />}
        </form.AppField>
        <form.AppField name="key" listeners={{ onChange: onKeyEdited }}>
          {(field) => (
            <field.TextField
              label={t('key')}
              hint={t('keyHint')}
              required
              spellCheck={false}
              autoCapitalize="off"
              className="font-mono"
              messageValues={{ min: KEY_RULES.min, max: KEY_RULES.max }}
            />
          )}
        </form.AppField>
      </FieldGroup>
    );
  },
});
