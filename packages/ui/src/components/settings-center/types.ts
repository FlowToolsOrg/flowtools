// TODO(review): waiting code review

export type SettingsValueKey = string | number

export interface SettingsNavSection {
  id: string
  label: string
  description?: string
  badge?: string
}

export interface SettingsOption {
  key: string
  label: string
  description?: string
  isDisabled?: boolean
}

export interface SettingsSwitchValue {
  id: string
  type: 'switch'
  label: string
  description?: string
  value: boolean
}

export interface SettingsSelectValue {
  id: string
  type: 'select'
  label: string
  description?: string
  value: SettingsValueKey | null
  options: SettingsOption[]
}

export interface SettingsInputValue {
  id: string
  type: 'input'
  label: string
  description?: string
  value: string
  placeholder?: string
}

export type SettingsFieldValue =
  | SettingsSwitchValue
  | SettingsSelectValue
  | SettingsInputValue

export interface SettingsGroup {
  id: string
  title: string
  description?: string
  fields: SettingsFieldValue[]
}
