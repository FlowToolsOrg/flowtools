/** Ephemeral Host references. Copying a string never grants authority. */
export type ResourceRef = string
export type ResourceRight = 'read' | 'stat' | 'list'
export interface ResourceInfo {
  ref: ResourceRef
  kind: 'file' | 'directory'
  name: string
  size: number | null
  revision: string
  rights: ResourceRight[]
}
export interface ResourceOptions {
  signal?: AbortSignal
}
export interface ResourceCapability {
  resolve(slot: string, options?: ResourceOptions): Promise<ResourceInfo>
  stat(ref: ResourceRef, options?: ResourceOptions): Promise<ResourceInfo>
  read(
    ref: ResourceRef,
    request: {
      revision: string
      offset: number
      maxBytes: number
      signal?: AbortSignal
    }
  ): Promise<{ bytes: Uint8Array; nextOffset: number; eof: boolean }>
  list(
    ref: ResourceRef,
    request: {
      revision: string
      cursor?: string
      limit: number
      signal?: AbortSignal
    }
  ): Promise<{ entries: ResourceInfo[]; nextCursor: string | null }>
  release(ref: ResourceRef, options?: ResourceOptions): Promise<void>
}
