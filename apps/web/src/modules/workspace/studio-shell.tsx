'use client';

import type { ReactNode } from 'react';

import { Separator } from '@/components/ui/separator';
import { SidebarInset, SidebarProvider, SidebarTrigger } from '@/components/ui/sidebar';
import { TourState } from '@/modules/onboarding';

import { AppSidebar } from './app-sidebar';
import { Breadcrumbs } from './breadcrumbs';
import { CommandMenu } from './command-menu';
import { HelpMenu } from './help-menu';

/**
 * The signed-in workspace: sidebar, a header with where you are, the command menu and Help, the
 * page. Pages register their guided tour; what each person has seen comes from the server.
 */
export function StudioShell({ children }: { children: ReactNode }) {
  return (
    <TourState>
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset>
          <header className="sticky top-0 z-10 flex h-14 shrink-0 items-center gap-2 border-b bg-background/95 px-4 backdrop-blur supports-[backdrop-filter]:bg-background/80">
            <SidebarTrigger className="-ml-1" />
            <Separator orientation="vertical" className="mr-2 data-[orientation=vertical]:h-4" />
            <Breadcrumbs />
            <div className="ml-auto flex items-center gap-1">
              <CommandMenu />
              <HelpMenu />
            </div>
          </header>
          {/* SidebarInset is the page's <main>; this is where the skip link lands. */}
          <div id="main" className="flex-1 px-4 py-6 lg:px-6">
            {children}
          </div>
        </SidebarInset>
      </SidebarProvider>
    </TourState>
  );
}
