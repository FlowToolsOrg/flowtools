import { useState } from 'react'

import {
  ToolDetailPage,
  ToolLayout,
  ToolLayoutMain,
  ToolLayoutSidebar,
  ToolPermissionList,
  ToolRelatedList,
  ToolSummaryCard,
  ToolVersionTimeline,
  type ToolPermissionRecord,
  type ToolRelatedRecord,
  type ToolVersionRecord,
} from '@flow-tool/ui'
import { createFileRoute } from '@tanstack/react-router'

import { Button } from '@heroui/react'

const permissions: ToolPermissionRecord[] = [
  {
    id: 'network',
    name: 'network',
    description: 'Used for remote hash validation APIs.',
    required: true,
  },
  {
    id: 'storage',
    name: 'storage',
    description: 'Cache previous output history locally.',
  },
]

const versions: ToolVersionRecord[] = [
  {
    id: 'v1.0.0',
    version: '1.0.0',
    date: '2026-01-15',
    notes: 'Initial stable release',
  },
  {
    id: 'v1.1.0',
    version: '1.1.0',
    date: '2026-02-10',
    notes: 'Add multi-hash support',
  },
]

const relatedTools: ToolRelatedRecord[] = [
  {
    id: 'jwt-decoder',
    name: 'JWT Decoder',
    description: 'Decode JWT payloads quickly.',
  },
  {
    id: 'base64-converter',
    name: 'Base64 Converter',
    description: 'Encode and decode Base64 text.',
  },
]

export const Route = createFileRoute('/detail')({
  component: DetailPage,
})

function DetailPage() {
  const [selectedPermissionId, setSelectedPermissionId] = useState(
    permissions[0]?.id ?? ''
  )
  const [selectedVersionId, setSelectedVersionId] = useState(
    versions[0]?.id ?? ''
  )
  const [selectedRelatedId, setSelectedRelatedId] = useState(
    relatedTools[0]?.id ?? ''
  )

  return (
    <ToolLayout>
      <ToolLayoutMain>
        <ToolDetailPage>
          <ToolDetailPage.Header>
            <h1 className="text-2xl font-semibold text-slate-900">
              Tool Detail
            </h1>
            <p className="text-sm text-slate-500">
              Detailed view using summary, permission, timeline and related-tool
              components.
            </p>
          </ToolDetailPage.Header>
          <ToolDetailPage.Content>
            <div className="space-y-4">
              <ToolSummaryCard
                category="Security"
                description="Generate SHA-256/MD5 hashes with optional remote validation."
                status="stable"
                title="Hash Generator"
                version="1.1.0"
                actions={
                  <>
                    <Button size="sm">Install</Button>
                    <Button size="sm" variant="outline">
                      Star
                    </Button>
                  </>
                }
              />

              <section className="space-y-2">
                <h2 className="text-sm font-semibold text-slate-900">
                  Permissions
                </h2>
                <ToolPermissionList>
                  {permissions.map(permission => (
                    <ToolPermissionList.Item
                      key={permission.id}
                      description={permission.description}
                      name={permission.name}
                      onPress={() => setSelectedPermissionId(permission.id)}
                      permissionId={permission.id}
                      required={permission.required}
                    />
                  ))}
                </ToolPermissionList>
              </section>

              <section className="space-y-2">
                <h2 className="text-sm font-semibold text-slate-900">
                  Version Timeline
                </h2>
                <ToolVersionTimeline
                  onSelect={setSelectedVersionId}
                  records={versions}
                />
              </section>
            </div>

            <div className="space-y-4">
              <ToolRelatedList
                items={relatedTools}
                onSelect={setSelectedRelatedId}
              />
              <div className="rounded-3xl border border-slate-200/80 bg-white p-4 text-sm text-slate-600">
                <p>
                  Selected permission:{' '}
                  <strong>{selectedPermissionId || '-'}</strong>
                </p>
                <p>
                  Selected version: <strong>{selectedVersionId || '-'}</strong>
                </p>
                <p>
                  Selected related tool:{' '}
                  <strong>{selectedRelatedId || '-'}</strong>
                </p>
              </div>
            </div>
          </ToolDetailPage.Content>
        </ToolDetailPage>
      </ToolLayoutMain>
      <ToolLayoutSidebar>
        <h3 className="text-sm font-semibold text-slate-900">Detail Notes</h3>
        <p className="text-sm text-slate-600">
          Current page acts as a host-level detail shell. Runtime-bound data can
          be injected later from SDK/runtime layer.
        </p>
      </ToolLayoutSidebar>
    </ToolLayout>
  )
}
