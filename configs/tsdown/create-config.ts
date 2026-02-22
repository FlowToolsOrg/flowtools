/// <reference types="node" />

import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { defineConfig } from 'tsdown'

type ManifestDeps = Record<string, string> | undefined
type EntryOption = string | string[] | Record<string, string | string[]>

interface PackageManifest {
  dependencies?: ManifestDeps
  peerDependencies?: ManifestDeps
  optionalDependencies?: ManifestDeps
}

interface CreatePackageConfigOptions {
  packageDir: string
  entry: EntryOption
  external?: string[]
  platform?: 'node' | 'neutral' | 'browser'
}

const readPackageManifest = (packageDir: string): PackageManifest => {
  const manifestPath = resolve(packageDir, 'package.json')
  const content = readFileSync(manifestPath, 'utf8')
  return JSON.parse(content) as PackageManifest
}

const inferExternalDeps = (manifest: PackageManifest): string[] => {
  return [
    ...Object.keys(manifest.dependencies ?? {}),
    ...Object.keys(manifest.peerDependencies ?? {}),
    ...Object.keys(manifest.optionalDependencies ?? {}),
  ]
}

export const createPackageTsdownConfig = ({
  packageDir,
  entry,
  external = [],
  platform = 'neutral',
}: CreatePackageConfigOptions) => {
  const manifest = readPackageManifest(packageDir)
  const inferredExternal = inferExternalDeps(manifest)

  return defineConfig({
    cwd: packageDir,
    entry,
    outDir: 'dist',
    format: ['esm'],
    dts: true,
    sourcemap: true,
    clean: true,
    skipNodeModulesBundle: true,
    platform,
    external: [...new Set([...inferredExternal, ...external])],
  })
}
