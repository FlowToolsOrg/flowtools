/// <reference types="bun-types" />

import { describe, expect, test } from 'bun:test'

interface ExportTarget {
  types: string
  import: string
}

interface PackageManifest {
  name: string
  exports: Record<string, ExportTarget>
}

type PublicModule = Record<string, unknown>

const expectedSubpaths = ['.', './icons', './plugin'] as const
type PublicSubpath = (typeof expectedSubpaths)[number]

const rootExports = [
  'CommandPalette',
  'HeroSection',
  'MarketEmptyState',
  'MarketToolbar',
  'RunHistoryPanel',
  'RunInputPanel',
  'RunLogList',
  'RunPanel',
  'RunResultPanel',
  'RunStatusStrip',
  'Settings',
  'SettingsCenterItem',
  'SettingsCenterPage',
  'SettingsGroupCard',
  'SettingsInputField',
  'SettingsSelectField',
  'SettingsSwitchField',
  'ToolCard',
  'ToolDetailPage',
  'ToolGrid',
  'ToolLayout',
  'ToolLayoutMain',
  'ToolLayoutSidebar',
  'ToolList',
  'ToolMarketPage',
  'ToolPermissionList',
  'ToolRelatedList',
  'ToolSummaryCard',
  'ToolVersionTimeline',
  'cn',
] as const

const compoundComponents = {
  MarketToolbar: ['Root', 'Search', 'Filters', 'Actions'],
  RunPanel: ['Root', 'Header', 'Content', 'Footer'],
  SettingsCenterItem: ['Root', 'Label', 'Description', 'Control'],
  SettingsCenterPage: ['Root', 'Nav', 'Content'],
  SettingsGroupCard: ['Root', 'Header', 'Title', 'Description', 'Content'],
  ToolCard: [
    'Root',
    'Header',
    'Title',
    'Description',
    'Meta',
    'Tags',
    'Actions',
  ],
  ToolDetailPage: ['Root', 'Header', 'Content'],
  ToolGrid: ['Root', 'Item'],
  ToolMarketPage: ['Root', 'Header', 'Content'],
  ToolPermissionList: ['Root', 'Item'],
} as const

const pluginExports = [
  'Accordion',
  'Alert',
  'AlertDialog',
  'Autocomplete',
  'Avatar',
  'Breadcrumbs',
  'Button',
  'ButtonGroup',
  'Calendar',
  'CalendarYearPicker',
  'Card',
  'Checkbox',
  'CheckboxGroup',
  'Chip',
  'CloseButton',
  'Collection',
  'ColorArea',
  'ColorField',
  'ColorPicker',
  'ColorSlider',
  'ColorSwatch',
  'ColorSwatchPicker',
  'ComboBox',
  'DateField',
  'DatePicker',
  'DateRangePicker',
  'Description',
  'Disclosure',
  'DisclosureGroup',
  'Dropdown',
  'EmptyState',
  'ErrorMessage',
  'FieldError',
  'Fieldset',
  'Form',
  'Header',
  'Input',
  'InputGroup',
  'InputOTP',
  'Kbd',
  'Label',
  'Link',
  'ListBox',
  'ListBoxItem',
  'ListBoxLoadMoreItem',
  'ListBoxSection',
  'Menu',
  'MenuItem',
  'MenuSection',
  'Modal',
  'NumberField',
  'Popover',
  'Radio',
  'RadioGroup',
  'RangeCalendar',
  'ScrollShadow',
  'SearchField',
  'Select',
  'Separator',
  'Skeleton',
  'Slider',
  'Spinner',
  'Surface',
  'Switch',
  'SwitchGroup',
  'Tabs',
  'Tag',
  'TagGroup',
  'Text',
  'TextArea',
  'TextField',
  'TimeField',
  'Toast',
  'ToastActionButton',
  'ToastCloseButton',
  'ToastContent',
  'ToastDescription',
  'ToastIndicator',
  'ToastQueue',
  'ToastTitle',
  'Tooltip',
  'cn',
  'parseColor',
  'toast',
  'toastQueue',
  'useIsHydrated',
  'useIsMounted',
  'useListData',
  'useMediaQuery',
  'useSafeLayoutEffect',
] as const

const hostOwnedPluginExports = [
  'I18nProvider',
  'RouterProvider',
  'ToastProvider',
  'useCssVariable',
  'useTheme',
] as const

const iconExports = [
  'ArrowLeftIcon',
  'BadgeAlertIcon',
  'BanIcon',
  'BlocksIcon',
  'CircleCheckIcon',
  'CircleHelpIcon',
  'ClockIcon',
  'CopyIcon',
  'EarthIcon',
  'EyeIcon',
  'FlaskIcon',
  'GalleryThumbnailsIcon',
  'HomeIcon',
  'LayersIcon',
  'LoaderPinwheelIcon',
  'LockIcon',
  'PlayIcon',
  'RefreshCWIcon',
  'ScanTextIcon',
  'SearchIcon',
  'SettingsIcon',
  'ShieldCheckIcon',
  'SlidersHorizontalIcon',
  'SparklesIcon',
  'TerminalIcon',
  'WrenchIcon',
  'XIcon',
  'ZapIcon',
] as const

const runtimeExportsBySubpath = {
  '.': rootExports,
  './icons': iconExports,
  './plugin': pluginExports,
} satisfies Record<PublicSubpath, readonly string[]>

const packageRoot = new URL('../', import.meta.url)

async function readManifest(): Promise<PackageManifest> {
  return Bun.file(new URL('package.json', packageRoot)).json()
}

function getSpecifier(manifest: PackageManifest, subpath: string): string {
  return subpath === '.'
    ? manifest.name
    : `${manifest.name}/${subpath.slice(2)}`
}

async function loadSubpath(
  manifest: PackageManifest,
  subpath: string
): Promise<PublicModule> {
  return (await import(getSpecifier(manifest, subpath))) as PublicModule
}

function expectExports(
  loaded: PublicModule,
  exportNames: readonly string[]
): void {
  for (const exportName of exportNames) {
    expect(loaded).toHaveProperty(exportName)
    expect(loaded[exportName]).toBeDefined()
  }
}

function expectExactExports(
  loaded: PublicModule,
  exportNames: readonly string[]
): void {
  expect(Object.keys(loaded).sort()).toEqual([...exportNames].sort())
  expectExports(loaded, exportNames)
}

describe('package exports', () => {
  test('points every public subpath at importable build output', async () => {
    const manifest = await readManifest()

    expect(Object.keys(manifest.exports).sort()).toEqual(
      [...expectedSubpaths].sort()
    )

    for (const subpath of expectedSubpaths) {
      const target = manifest.exports[subpath]
      expect(target.import.startsWith('./dist/')).toBe(true)
      expect(target.types.startsWith('./dist/')).toBe(true)

      for (const filePath of [target.import, target.types]) {
        const output = Bun.file(
          new URL(filePath.replace(/^\.\//, ''), packageRoot)
        )
        expect(await output.exists()).toBe(true)
        expect(output.size).toBeGreaterThan(0)
      }

      const loaded = await loadSubpath(manifest, subpath)
      expectExactExports(loaded, runtimeExportsBySubpath[subpath])
    }
  })
})

describe('public component contracts', () => {
  test('preserves compound component subcomponents', async () => {
    const manifest = await readManifest()
    const loaded = await loadSubpath(manifest, '.')

    for (const [componentName, subcomponents] of Object.entries(
      compoundComponents
    )) {
      const component = loaded[componentName] as PublicModule | undefined
      expect(component).toBeDefined()
      expectExports(component ?? {}, subcomponents)
    }
  })
})

describe('plugin and icon contracts', () => {
  test('keeps host-owned providers out of the plugin surface', async () => {
    const manifest = await readManifest()
    const loaded = await loadSubpath(manifest, './plugin')

    for (const exportName of hostOwnedPluginExports) {
      expect(loaded).not.toHaveProperty(exportName)
    }
  })
})
