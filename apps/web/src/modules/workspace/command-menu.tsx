'use client';

import { FolderOpen, Languages, Monitor, Moon, Search, Sun } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useTheme } from 'next-themes';
import { useEffect, useState } from 'react';

import { LOCALE_NAMES, useLocaleSwitch } from '@/components/shared/language-switch';
import { Button } from '@/components/ui/button';
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from '@/components/ui/command';
import { Kbd, KbdGroup } from '@/components/ui/kbd';
import { usePathname, useRouter } from '@/i18n/navigation';
import { routing } from '@/i18n/routing';
import { useCurrentUser } from '@/modules/identity';
import { projectHref, useOpenProject, useProjectList } from '@/modules/project';

import { PEOPLE_ICON, placeOf, PROJECTS_ICON, projectSections } from './sections';

/** Jump anywhere with the keyboard: Ctrl K (⌘ K on a Mac). */
export function CommandMenu() {
  const t = useTranslations('command');
  const nav = useTranslations('nav');
  const common = useTranslations('common');
  const sections = useTranslations('project');
  const user = useCurrentUser();
  const router = useRouter();
  const place = placeOf(usePathname());
  const { setTheme } = useTheme();
  const { change } = useLocaleSwitch();
  const [open, setOpen] = useState(false);
  // Loaded when the menu opens, not on every page.
  const projects = useProjectList(false, 0, open).data;
  const current = useOpenProject().query.data;
  const project = place.kind === 'project' && current?.ok ? current.data : null;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'k' && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        setOpen((value) => !value);
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);

  const run = (action: () => void) => {
    setOpen(false);
    action();
  };

  return (
    <>
      <Button
        variant="outline"
        size="sm"
        className="w-9 justify-center px-0 text-muted-foreground sm:w-56 sm:justify-between sm:px-3"
        onClick={() => setOpen(true)}
        aria-label={t('open')}
      >
        <span className="flex items-center gap-2">
          <Search aria-hidden />
          <span className="hidden sm:inline">{t('placeholder')}</span>
        </span>
        <KbdGroup className="hidden sm:inline-flex">
          <Kbd>Ctrl</Kbd>
          <Kbd>K</Kbd>
        </KbdGroup>
      </Button>
      <CommandDialog open={open} onOpenChange={setOpen} title={t('title')} description={t('description')}>
        <CommandInput placeholder={t('placeholder')} />
        <CommandList>
          <CommandEmpty>{t('empty')}</CommandEmpty>
          {project ? (
            <CommandGroup heading={project.name}>
              {projectSections(project).map(({ section, icon: Icon }) => (
                <CommandItem
                  key={section}
                  value={`${project.name} ${sections(section)}`}
                  onSelect={() => run(() => router.push(projectHref(section, project.key)))}
                >
                  <Icon aria-hidden />
                  {sections(section)}
                </CommandItem>
              ))}
            </CommandGroup>
          ) : null}
          <CommandGroup heading={t('goTo')}>
            <CommandItem onSelect={() => run(() => router.push('/'))}>
              <PROJECTS_ICON aria-hidden />
              {nav('projects')}
            </CommandItem>
            {user.isAdmin ? (
              <CommandItem onSelect={() => run(() => router.push('/people'))}>
                <PEOPLE_ICON aria-hidden />
                {nav('people')}
              </CommandItem>
            ) : null}
          </CommandGroup>
          {projects?.ok && projects.data.items.length > 0 ? (
            <CommandGroup heading={nav('projects')}>
              {projects.data.items.map((item) => (
                <CommandItem
                  key={item.id}
                  value={`${item.name} ${item.key}`}
                  onSelect={() => run(() => router.push(projectHref('members', item.key)))}
                >
                  <FolderOpen aria-hidden />
                  <span className="truncate">{item.name}</span>
                  <span className="ml-auto font-mono text-xs text-muted-foreground">{item.key}</span>
                </CommandItem>
              ))}
            </CommandGroup>
          ) : null}
          <CommandSeparator />
          <CommandGroup heading={t('preferences')}>
            <CommandItem onSelect={() => run(() => setTheme('light'))}>
              <Sun aria-hidden />
              {t('theme', { theme: common('themeLight') })}
            </CommandItem>
            <CommandItem onSelect={() => run(() => setTheme('dark'))}>
              <Moon aria-hidden />
              {t('theme', { theme: common('themeDark') })}
            </CommandItem>
            <CommandItem onSelect={() => run(() => setTheme('system'))}>
              <Monitor aria-hidden />
              {t('theme', { theme: common('themeSystem') })}
            </CommandItem>
            {routing.locales.map((code) => (
              <CommandItem key={code} onSelect={() => run(() => change(code))}>
                <Languages aria-hidden />
                <span lang={code}>{common(LOCALE_NAMES[code])}</span>
              </CommandItem>
            ))}
          </CommandGroup>
        </CommandList>
      </CommandDialog>
    </>
  );
}
