import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const REQUIRED_TASKS = ['build', 'lint', 'check-types', 'test'] as const

interface PackageJson {
  name?: string
  scripts?: Record<string, string>
}

interface TurboJson {
  tasks?: Record<string, unknown>
}

const repositoryRoot = fileURLToPath(new URL('../', import.meta.url))

async function readJson<T>(path: string): Promise<T> {
  return JSON.parse(await readFile(path, 'utf8')) as T
}

async function findWorkspacePackageFiles(): Promise<string[]> {
  const packageFiles: string[] = []

  for (const group of ['apps', 'packages']) {
    const entries = await readdir(join(repositoryRoot, group), {
      withFileTypes: true,
    })

    for (const entry of entries) {
      if (entry.isDirectory()) {
        packageFiles.push(
          join(repositoryRoot, group, entry.name, 'package.json')
        )
      }
    }
  }

  packageFiles.push(join(repositoryRoot, 'plugins', 'package.json'))

  return packageFiles.sort()
}

function validateWorkspace(
  packageFile: string,
  packageJson: PackageJson
): string[] {
  const errors: string[] = []
  const workspaceName = packageJson.name ?? packageFile
  const scripts = packageJson.scripts ?? {}

  for (const task of REQUIRED_TASKS) {
    if (!scripts[task]) {
      errors.push(`${workspaceName}: missing script "${task}"`)
    }
  }

  if (scripts.lint?.includes('--fix')) {
    errors.push(`${workspaceName}: lint must be read-only; use lint:fix`)
  }

  if (scripts['check:types']) {
    errors.push(`${workspaceName}: use "check-types", not "check:types"`)
  }

  if (/deprecated|console\.log/i.test(scripts.test ?? '')) {
    errors.push(`${workspaceName}: test must invoke a test runner`)
  }

  return errors
}

async function main(): Promise<void> {
  const packageFiles = await findWorkspacePackageFiles()
  const errors: string[] = []

  for (const packageFile of packageFiles) {
    const packageJson = await readJson<PackageJson>(packageFile)
    errors.push(...validateWorkspace(packageFile, packageJson))
  }

  const rootPackage = await readJson<PackageJson>(
    join(repositoryRoot, 'package.json')
  )
  const rootTest = rootPackage.scripts?.test
  if (rootTest !== 'turbo run test') {
    errors.push('root: test must delegate to "turbo run test"')
  }

  const turbo = await readJson<TurboJson>(join(repositoryRoot, 'turbo.json'))
  for (const task of REQUIRED_TASKS) {
    if (!turbo.tasks?.[task]) {
      errors.push(`turbo.json: missing task "${task}"`)
    }
  }

  if (errors.length > 0) {
    for (const error of errors) {
      process.stderr.write(`- ${error}\n`)
    }

    process.exitCode = 1
    return
  }

  process.stdout.write(
    `Verified ${packageFiles.length} workspaces with standard quality tasks.\n`
  )
}

await main()
