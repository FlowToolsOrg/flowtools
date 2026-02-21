'use client'

import {
  Card,
  Chip,
  Description,
  Label,
  ListBox,
  type Selection,
} from '@heroui/react'
import { type ComponentPropsWithoutRef, type ReactNode } from 'react'

import { cn } from '../../utils/class-name'

export type ToolStatus = 'stable' | 'beta' | 'experimental' | 'deprecated'
export type ToolListKey = string | number

export interface ToolListItem {
  id: ToolListKey
  name: string
  description?: string
  status?: ToolStatus
  version?: string
  tags?: string[]
  isInstalled?: boolean
  isPinned?: boolean
  isDisabled?: boolean
  endContent?: ReactNode
}

export interface ToolListProps extends Omit<
  ComponentPropsWithoutRef<'section'>,
  'children'
> {
  title?: string
  description?: string
  items: ToolListItem[]
  ariaLabel?: string
  selectionMode?: 'none' | 'single' | 'multiple'
  selectedKeys?: Selection
  defaultSelectedKeys?: Selection
  onSelectionChange?: (keys: Selection) => void
  onAction?: (key: ToolListKey) => void
  emptyContent?: ReactNode
}

const statusLabelMap: Record<ToolStatus, string> = {
  stable: 'Stable',
  beta: 'Beta',
  experimental: 'Experimental',
  deprecated: 'Deprecated',
}

const statusColorMap: Record<
  ToolStatus,
  'success' | 'accent' | 'warning' | 'danger'
> = {
  stable: 'success',
  beta: 'accent',
  experimental: 'warning',
  deprecated: 'danger',
}

export const ToolList = ({
  title = 'Tool List',
  description,
  items,
  ariaLabel = 'Tool list',
  selectionMode = 'single',
  selectedKeys,
  defaultSelectedKeys,
  onSelectionChange,
  onAction,
  emptyContent,
  className,
  ...props
}: ToolListProps) => {
  return (
    <section className={cn('w-full', className)} {...props}>
      <Card
        className="rounded-3xl border border-slate-200/80 bg-white/90 shadow-sm"
        variant="secondary"
      >
        <Card.Header className="flex flex-col gap-2">
          <Card.Title>{title}</Card.Title>
          {description ? (
            <Card.Description className="text-sm text-slate-600">
              {description}
            </Card.Description>
          ) : null}
        </Card.Header>
        <Card.Content>
          {items.length === 0 ? (
            <div className="rounded-2xl border border-dashed border-slate-300 bg-white px-4 py-6 text-sm text-slate-500">
              {emptyContent ?? 'No tools found yet.'}
            </div>
          ) : (
            <ListBox
              aria-label={ariaLabel}
              className="w-full rounded-2xl border border-slate-200/80 bg-white p-2"
              defaultSelectedKeys={defaultSelectedKeys}
              onAction={onAction}
              onSelectionChange={onSelectionChange}
              selectedKeys={selectedKeys}
              selectionMode={selectionMode}
            >
              {items.map(item => (
                <ListBox.Item
                  key={String(item.id)}
                  className="rounded-xl px-3 py-3 transition data-[hovered=true]:bg-amber-50/70 data-[selected=true]:bg-amber-100/70"
                  id={item.id}
                  isDisabled={item.isDisabled}
                  textValue={item.name}
                >
                  <div className="flex min-w-0 flex-1 flex-col gap-2">
                    <div className="flex flex-wrap items-center gap-2">
                      <Label className="text-sm font-semibold text-slate-900">
                        {item.name}
                      </Label>
                      {item.status ? (
                        <Chip
                          color={statusColorMap[item.status]}
                          size="sm"
                          variant="soft"
                        >
                          {statusLabelMap[item.status]}
                        </Chip>
                      ) : null}
                      {item.isPinned ? (
                        <Chip size="sm" variant="tertiary">
                          Pinned
                        </Chip>
                      ) : null}
                      {item.isInstalled ? (
                        <Chip color="success" size="sm" variant="secondary">
                          Installed
                        </Chip>
                      ) : null}
                    </div>
                    {item.description ? (
                      <Description className="text-sm text-slate-600">
                        {item.description}
                      </Description>
                    ) : null}
                    <div className="flex flex-wrap items-center gap-2">
                      {item.version ? (
                        <Chip size="sm" variant="secondary">
                          v{item.version}
                        </Chip>
                      ) : null}
                      {item.tags?.map(tag => (
                        <Chip key={tag} size="sm" variant="tertiary">
                          {tag}
                        </Chip>
                      ))}
                    </div>
                  </div>
                  {item.endContent}
                  {selectionMode !== 'none' ? <ListBox.ItemIndicator /> : null}
                </ListBox.Item>
              ))}
            </ListBox>
          )}
        </Card.Content>
      </Card>
    </section>
  )
}
