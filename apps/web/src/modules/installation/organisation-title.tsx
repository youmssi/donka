'use client';

import { useEffect } from 'react';

import { useOrganisation } from './useInstallation';

/**
 * Adds the organisation to the browser tab's title, `Projects · Donka · Microfinance SA`, so
 * people with several Studios open tell them apart. The pages set their title as they load; this
 * adds the name each time the title changes.
 */
export function OrganisationTitle() {
  const organisation = useOrganisation();

  useEffect(() => {
    if (!organisation) return;
    const suffix = ` · ${organisation}`;
    const sign = () => {
      if (!document.title.endsWith(suffix)) document.title = `${document.title}${suffix}`;
    };
    sign();
    const observer = new MutationObserver(sign);
    observer.observe(document.head, { subtree: true, childList: true, characterData: true });
    return () => {
      observer.disconnect();
      if (document.title.endsWith(suffix)) document.title = document.title.slice(0, -suffix.length);
    };
  }, [organisation]);

  return null;
}
