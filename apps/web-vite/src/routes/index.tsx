import { HeroSection, ToolLayout, ToolLayoutMain } from '@flow-tool/ui'
import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { Button } from '@heroui/react'

export const Route = createFileRoute('/')({
  component: Index,
})

function Index() {
  const navigate = useNavigate()

  return (
    <ToolLayout>
      <ToolLayoutMain>
        <HeroSection
          badges={[
            'tool-market',
            'tool-detail',
            'run-panel',
            'settings-center',
          ]}
          description="web-vite 已接入 @flow-tool/ui 新组件分组，按页面组织展示市场、详情、运行与设置。"
          eyebrow="Flow Tool"
          stats={[
            { id: 'routes', label: 'Demo Routes', value: 4 },
            { id: 'ui', label: 'UI Package', value: '@flow-tool/ui' },
            { id: 'theme', label: 'Theme', value: 'HeroUI default' },
            { id: 'runtime', label: 'Runtime', value: 'web-vite' },
          ]}
          title="Flow Tool Web Host"
          actions={
            <>
              <Button onPress={() => navigate({ to: '/market' })}>
                Open Market
              </Button>
              <Button
                onPress={() => navigate({ to: '/run' })}
                variant="outline"
              >
                Open Run Panel
              </Button>
            </>
          }
        />
      </ToolLayoutMain>
    </ToolLayout>
  )
}
