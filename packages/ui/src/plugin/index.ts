/**
 * HeroUI component re-exports for plugin use.
 *
 * Excludes global providers (ToastProvider, RouterProvider, I18nProvider)
 * and theming internals (useTheme, useCssVariable) that are managed by
 * the host application.
 */

// ── Components ──

// Buttons
export { Button, ButtonGroup, CloseButton } from '@heroui/react'

// Forms
export {
  Checkbox,
  CheckboxGroup,
  Description,
  ErrorMessage,
  FieldError,
  Fieldset,
  Form,
  Input,
  InputGroup,
  InputOTP,
  Label,
  NumberField,
  Radio,
  RadioGroup,
  SearchField,
  Select,
  Switch,
  SwitchGroup,
  TextArea,
  TextField,
} from '@heroui/react'

// Data Display
export {
  Avatar,
  Card,
  Chip,
  EmptyState,
  Header,
  Kbd,
  ListBox,
  ListBoxItem,
  ListBoxSection,
  ScrollShadow,
  Separator,
  Surface,
  Tag,
  TagGroup,
  Text,
} from '@heroui/react'

// Feedback
export { Alert, Skeleton, Spinner } from '@heroui/react'

// Toast — expose the toast API and sub-components, but NOT ToastProvider
export {
  Toast,
  ToastActionButton,
  ToastCloseButton,
  ToastContent,
  ToastDescription,
  ToastIndicator,
  ToastQueue,
  ToastTitle,
  toast,
  toastQueue,
} from '@heroui/react'

// Navigation
export {
  Accordion,
  Breadcrumbs,
  Disclosure,
  DisclosureGroup,
  Link,
  Tabs,
} from '@heroui/react'

// Overlays
export {
  AlertDialog,
  Dropdown,
  Menu,
  MenuItem,
  MenuSection,
  Modal,
  Popover,
  Tooltip,
} from '@heroui/react'

// Pickers
export { Autocomplete, ComboBox } from '@heroui/react'

// Date & Time
export {
  Calendar,
  CalendarYearPicker,
  DateField,
  DatePicker,
  DateRangePicker,
  RangeCalendar,
  TimeField,
} from '@heroui/react'

// Colors
export {
  ColorArea,
  ColorField,
  ColorPicker,
  ColorSlider,
  ColorSwatch,
  ColorSwatchPicker,
  parseColor,
} from '@heroui/react'

// Layout & Structure
export { Slider } from '@heroui/react'

// ── Hooks (safe for plugin use) ──
export {
  useIsHydrated,
  useIsMounted,
  useListData,
  useMediaQuery,
  useSafeLayoutEffect,
} from '@heroui/react'

// ── Utilities ──
export { cn } from '@heroui/react'

// ── RAC re-exports (safe for plugin use) ──
export { Collection, ListBoxLoadMoreItem } from '@heroui/react'

// ── Types ──
export type { Key, Selection, ValidationResult } from '@heroui/react'
