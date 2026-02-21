import { SettingsCenterPage, type SettingsNavSection } from '@flow-tool/ui'
import { describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

const sections: SettingsNavSection[] = [
  { id: 'general', label: 'General', description: 'Base settings' },
  { id: 'runtime', label: 'Runtime', description: 'Plugin runtime' },
]

describe('SettingsCenterPage', () => {
  it('renders navigation and content slots', async () => {
    const { getByRole, getByText } = await render(
      <SettingsCenterPage>
        <SettingsCenterPage.Nav activeSectionId="general" sections={sections} />
        <SettingsCenterPage.Content>
          <p>Section Content</p>
        </SettingsCenterPage.Content>
      </SettingsCenterPage>
    )

    await expect
      .element(getByRole('button', { name: 'General' }))
      .toBeInTheDocument()
    await expect.element(getByText('Section Content')).toBeInTheDocument()
  })

  it('calls onSectionChange when nav item clicked', async () => {
    const user = userEvent.setup()
    const onSectionChange = vi.fn()
    const { getByRole } = await render(
      <SettingsCenterPage>
        <SettingsCenterPage.Nav
          onSectionChange={onSectionChange}
          sections={sections}
        />
        <SettingsCenterPage.Content />
      </SettingsCenterPage>
    )

    await user.click(getByRole('button', { name: 'Runtime' }))

    expect(onSectionChange).toHaveBeenCalledWith('runtime')
  })
})
