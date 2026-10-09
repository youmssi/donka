import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import { NextIntlClientProvider } from 'next-intl';
import type { ReactElement } from 'react';

import { TourProvider } from '@/components/shared/tour';
import { TooltipProvider } from '@/components/ui/tooltip';
import messages from '@/messages/en.json';

/** Renders with the providers every Studio component expects, in English. */
export function renderWithProviders(ui: ReactElement) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <NextIntlClientProvider locale="en" messages={messages}>
      <QueryClientProvider client={queryClient}>
        {/* Tours seen unknown: no tour starts on its own in a test. */}
        <TourProvider seen={undefined} onSeen={() => {}}>
          <TooltipProvider>{ui}</TooltipProvider>
        </TourProvider>
      </QueryClientProvider>
    </NextIntlClientProvider>,
  );
}
