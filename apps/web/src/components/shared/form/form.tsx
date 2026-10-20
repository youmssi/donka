'use client';

import { useEffect, useRef, useSyncExternalStore, type ComponentProps } from 'react';

import { useFormContext } from './context';

const subscribe = () => () => {};
const clientSnapshot = () => true;
const serverSnapshot = () => false;

/**
 * The `<form>` element of a `useAppForm` form. It submits through the form (the browser's own
 * validation is off: the form's schema decides) and, when the submission is refused, moves focus
 * to the first field showing an error so a keyboard or screen reader user lands on it.
 */
export function Form({ children, ...props }: Omit<ComponentProps<'form'>, 'onSubmit' | 'noValidate'>) {
  const form = useFormContext();
  const element = useRef<HTMLFormElement>(null);
  const hydrated = useSyncExternalStore(subscribe, clientSnapshot, serverSnapshot);
  const restoreAutofocus = useRef(!hydrated);

  useEffect(() => {
    if (!hydrated || !restoreAutofocus.current) return;
    restoreAutofocus.current = false;
    // Disabled static fields cannot receive their initial autofocus.
    if (document.activeElement === document.body) {
      element.current?.querySelector<HTMLElement>('[autofocus]')?.focus();
    }
  }, [hydrated]);

  return (
    <form
      ref={element}
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit().then(() => {
          if (form.state.isValid) return;
          // The fields mark themselves invalid on the render that follows the submission.
          setTimeout(() => {
            element.current?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
          }, 0);
        });
      }}
      {...props}
    >
      {/* Static fields must not accept input that hydration would discard. */}
      <fieldset disabled={!hydrated} className="contents">
        {children}
      </fieldset>
    </form>
  );
}
