'use client'

import { type ComponentPropsWithoutRef } from 'react'

import {
  Card,
  Description,
  Label,
  ListBox,
  Select,
  Separator,
  Switch,
} from '@heroui/react'

import { cn } from '../../utils/class-name'

export type SettingsValueKey = string | number

export interface SettingsSelectOption {
  key: string
  label: string
  description?: string
  isDisabled?: boolean
}

interface SettingsItemBase {
  id: string
  label: string
  description?: string
  isDisabled?: boolean
}

export interface SettingsItemSwitch extends SettingsItemBase {
  type: 'switch'
  value: boolean
  onChange: (value: boolean) => void
}

export interface SettingsItemSelect extends SettingsItemBase {
  type: 'select'
  value: SettingsValueKey | null
  onChange: (value: SettingsValueKey | null) => void
  placeholder?: string
  options: SettingsSelectOption[]
}

export type SettingsItem = SettingsItemSwitch | SettingsItemSelect

export interface SettingsSection {
  id: string
  title: string
  description?: string
  items: SettingsItem[]
}

export interface SettingsProps extends Omit<
  ComponentPropsWithoutRef<'section'>,
  'children'
> {
  title?: string
  description?: string
  sections: SettingsSection[]
}

const normalizeSelectValue = (
  value: SettingsValueKey | SettingsValueKey[] | null
): SettingsValueKey | null => {
  if (Array.isArray(value)) {
    return value[0] ?? null
  }

  return value
}

const renderSettingsItem = (item: SettingsItem) => {
  if (item.type === 'switch') {
    return (
      <div className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface-secondary)] px-3 py-3">
        <Switch
          isDisabled={item.isDisabled}
          isSelected={item.value}
          onChange={item.onChange}
        >
          <Switch.Control>
            <Switch.Thumb />
          </Switch.Control>
          <Switch.Content>
            <Label className="text-sm font-medium">{item.label}</Label>
            {item.description ? (
              <Description className="text-xs text-[var(--muted)]">
                {item.description}
              </Description>
            ) : null}
          </Switch.Content>
        </Switch>
      </div>
    )
  }

  return (
    <div className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface-secondary)] p-3">
      <Select
        className="w-full"
        isDisabled={item.isDisabled}
        onChange={value => item.onChange(normalizeSelectValue(value))}
        placeholder={item.placeholder ?? 'Select an option'}
        value={item.value}
      >
        <Label>{item.label}</Label>
        <Select.Trigger>
          <Select.Value />
          <Select.Indicator />
        </Select.Trigger>
        <Select.Popover>
          <ListBox>
            {item.options.map(option => (
              <ListBox.Item
                key={option.key}
                id={option.key}
                isDisabled={option.isDisabled}
                textValue={option.label}
              >
                <div className="flex flex-col gap-0.5">
                  <Label>{option.label}</Label>
                  {option.description ? (
                    <Description className="text-xs text-[var(--muted)]">
                      {option.description}
                    </Description>
                  ) : null}
                </div>
                <ListBox.ItemIndicator />
              </ListBox.Item>
            ))}
          </ListBox>
        </Select.Popover>
        {item.description ? (
          <Description className="text-xs text-[var(--muted)]">
            {item.description}
          </Description>
        ) : null}
      </Select>
    </div>
  )
}

export const Settings = ({
  title = 'Settings',
  description,
  sections,
  className,
  ...props
}: SettingsProps) => {
  return (
    <section className={cn('w-full', className)} {...props}>
      <Card
        className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--surface)]"
        variant="secondary"
      >
        <Card.Header className="flex flex-col gap-2">
          <Card.Title className="text-[var(--foreground)]">{title}</Card.Title>
          {description ? (
            <Card.Description className="text-sm text-[var(--muted)]">
              {description}
            </Card.Description>
          ) : null}
        </Card.Header>
        <Card.Content className="space-y-5">
          {sections.map((section, index) => (
            <div key={section.id} className="space-y-3">
              <div className="space-y-1">
                <h3 className="text-sm font-semibold tracking-wide text-[var(--foreground)] uppercase">
                  {section.title}
                </h3>
                {section.description ? (
                  <p className="text-sm text-[var(--muted)]">
                    {section.description}
                  </p>
                ) : null}
              </div>
              <div className="space-y-3">
                {section.items.map(item => (
                  <div key={item.id}>{renderSettingsItem(item)}</div>
                ))}
              </div>
              {index < sections.length - 1 ? (
                <Separator className="my-4" variant="secondary" />
              ) : null}
            </div>
          ))}
        </Card.Content>
      </Card>
    </section>
  )
}
