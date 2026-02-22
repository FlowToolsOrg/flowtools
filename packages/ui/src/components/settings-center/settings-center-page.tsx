'use client'

import { type ComponentPropsWithoutRef } from 'react'

import { Chip, Surface } from '@heroui/react'

import { cn } from '../../utils/class-name'

import { type SettingsNavSection } from './types'

export interface SettingsCenterPageRootProps extends ComponentPropsWithoutRef<'section'> {}

export interface SettingsCenterPageNavProps extends Omit<
  ComponentPropsWithoutRef<'nav'>,
  'onChange'
> {
  sections: SettingsNavSection[]
  activeSectionId?: string
  onSectionChange?: (sectionId: string) => void
}

export interface SettingsCenterPageContentProps extends ComponentPropsWithoutRef<'div'> {}

const SettingsCenterPageRoot = ({
  children,
  className,
  ...props
}: SettingsCenterPageRootProps) => {
  return (
    <section
      className={cn('grid gap-4 lg:grid-cols-[16rem_minmax(0,1fr)]', className)}
      {...props}
    >
      {children}
    </section>
  )
}

const SettingsCenterPageNav = ({
  sections,
  activeSectionId,
  onSectionChange,
  className,
  ...props
}: SettingsCenterPageNavProps) => {
  return (
    <nav className={cn('min-w-0', className)} {...props}>
      <Surface
        className="rounded-3xl border border-slate-200/80 p-2"
        variant="secondary"
      >
        <ul className="flex flex-col gap-1" role="list">
          {sections.map(section => {
            const isActive = section.id === activeSectionId

            return (
              <li key={section.id}>
                <button
                  aria-current={isActive ? 'page' : undefined}
                  className={cn(
                    'flex w-full items-start justify-between gap-3 rounded-2xl px-3 py-2 text-left transition',
                    isActive ? 'bg-white shadow-sm' : 'hover:bg-white/70'
                  )}
                  data-active={isActive}
                  onClick={() => onSectionChange?.(section.id)}
                  type="button"
                >
                  <span className="flex min-w-0 flex-col">
                    <span className="truncate text-sm font-medium text-slate-900">
                      {section.label}
                    </span>
                    {section.description ? (
                      <span className="truncate text-xs text-slate-500">
                        {section.description}
                      </span>
                    ) : null}
                  </span>
                  {section.badge ? (
                    <Chip size="sm" variant="tertiary">
                      {section.badge}
                    </Chip>
                  ) : null}
                </button>
              </li>
            )
          })}
        </ul>
      </Surface>
    </nav>
  )
}

const SettingsCenterPageContent = ({
  children,
  className,
  ...props
}: SettingsCenterPageContentProps) => {
  return (
    <div className={cn('min-w-0 space-y-4', className)} {...props}>
      {children}
    </div>
  )
}

export const SettingsCenterPage = Object.assign(SettingsCenterPageRoot, {
  Root: SettingsCenterPageRoot,
  Nav: SettingsCenterPageNav,
  Content: SettingsCenterPageContent,
})
