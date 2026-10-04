import { expect, test } from 'bun:test'
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
  existsSync,
} from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'

import { parsePluginManifest } from '@flowtools/sdk/manifest'
import ts from 'typescript'

import { pluginSmokeFixtures } from './smoke-fixtures'

test('all real command artifacts run in a consumer with no React, GUI or source checkout', async () => {
  const root = mkdtempSync(join(tmpdir(), 'flowtools-headless-'))
  const pluginsRoot = resolve(import.meta.dir, '..')
  try {
    const copied = new Set<string>()
    const copyGraph = (source: string, destination: string) => {
      const canonical = realpathSync(source)
      if (copied.has(canonical)) return
      copied.add(canonical)
      const content = readFileSync(canonical, 'utf8')
      const ast = ts.createSourceFile(
        source,
        content,
        ts.ScriptTarget.Latest,
        true,
        ts.ScriptKind.JS
      )
      const imports = ast.statements.flatMap(statement =>
        (ts.isImportDeclaration(statement) ||
          ts.isExportDeclaration(statement)) &&
        statement.moduleSpecifier &&
        ts.isStringLiteral(statement.moduleSpecifier)
          ? [statement.moduleSpecifier.text]
          : []
      )
      for (const specifier of imports) {
        expect(specifier).not.toMatch(
          /^(?:react(?:-dom)?(?:\/|$)|@flowtools\/ui|@tauri-apps\/|@flowtools\/sdk\/(?:runtime|hooks|development))/
        )
        expect(specifier).not.toBe('@flowtools/sdk')
        if (specifier.startsWith('.'))
          copyGraph(
            resolve(dirname(source), specifier),
            resolve(dirname(destination), specifier)
          )
        else if (specifier.startsWith('@flowtools/sdk/')) {
          const path = specifier.slice('@flowtools/sdk/'.length) + '.js'
          copyGraph(
            resolve(pluginsRoot, '../packages/sdk/dist', path),
            join(root, 'node_modules/@flowtools/sdk/dist', path)
          )
        } else
          expect([
            'zod',
            'semver',
            'node:fs',
            'node:path',
            'node:crypto',
          ]).toContain(specifier)
      }
      mkdirSync(dirname(destination), { recursive: true })
      writeFileSync(destination, content)
    }
    for (const fixture of pluginSmokeFixtures)
      copyGraph(
        join(pluginsRoot, 'dist', `${fixture.id}.commands.js`),
        join(root, `${fixture.id}.commands.js`)
      )
    for (const subpath of ['manifest', 'execution'])
      copyGraph(
        resolve(pluginsRoot, '../packages/sdk/dist', `${subpath}.js`),
        join(root, 'node_modules/@flowtools/sdk/dist', `${subpath}.js`)
      )
    const require = createRequire(
      resolve(pluginsRoot, '../packages/sdk/package.json')
    )
    for (const dependency of ['zod', 'semver']) {
      const packagePath = require.resolve(`${dependency}/package.json`)
      cpSync(dirname(packagePath), join(root, 'node_modules', dependency), {
        recursive: true,
      })
    }
    writeFileSync(
      join(root, 'node_modules/@flowtools/sdk/package.json'),
      JSON.stringify({
        type: 'module',
        exports: {
          './manifest': './dist/manifest.js',
          './execution': './dist/execution.js',
          './result': './dist/result.js',
        },
      })
    )
    const catalog = JSON.parse(
      readFileSync(
        join(pluginsRoot, '.generated/builtin-manifests.json'),
        'utf8'
      )
    ) as { plugins: unknown[] }
    const manifests = catalog.plugins.map(value => parsePluginManifest(value))
    writeFileSync(
      join(root, 'fixtures.json'),
      JSON.stringify(pluginSmokeFixtures)
    )
    writeFileSync(join(root, 'manifests.json'), JSON.stringify(manifests))
    expect(existsSync(join(root, 'node_modules/react'))).toBe(false)
    expect(existsSync(join(root, 'src'))).toBe(false)
    writeFileSync(
      join(root, 'probe.mjs'),
      `
      import assert from 'node:assert/strict';
      import {readFileSync} from 'node:fs';
      import {createRequire} from 'node:module';
      import {executeManifestCommand} from '@flowtools/sdk/manifest';
      const require=createRequire(import.meta.url);
      assert.throws(()=>require.resolve('react'));
      assert.throws(()=>require.resolve('@flowtools/ui'));
      const fixtures=JSON.parse(readFileSync(new URL('./fixtures.json',import.meta.url),'utf8'));
      const manifests=JSON.parse(readFileSync(new URL('./manifests.json',import.meta.url),'utf8'));
      for(const fixture of fixtures) {
        const {default:plugin}=await import('./'+fixture.id+'.commands.js');
        assert.equal('setup' in plugin,false);
        const values=new Map(); const writes=[]; const requests=[];
        const ctx={env:{pluginId:fixture.id,pluginType:'app',platform:'unknown',mode:'test'},
          ui:{toast(){},openPanel(){},closePanel(){}},signal:new AbortController().signal,log(){},utils:{now:Date.now},
          storage:{get:key=>values.get(key),set:(key,value)=>{writes.push(key);values.set(key,value)}},
          request:async input=>{assert.equal(input,'https://latency.fixture.invalid/');requests.push(input);return new Response('fixture')}};
        const manifest=manifests.find(item=>item.id===fixture.id);
        const target={hostVersion:'0.1.0',sdkVersion:'0.0.0',platform:'windows',arch:'x64'};
        const invalid=await executeManifestCommand(manifest,'run',plugin,{[Object.keys(manifest.commands[0].inputSchema.properties)[0]]:null},ctx,target);
        assert.equal(invalid.success,false); assert.equal(invalid.error.code,'INPUT_INVALID');
        assert.equal(writes.length+requests.length,0);
        const result=await executeManifestCommand(manifest,'run',plugin,fixture.input,ctx,target);
        assert.equal(result.success,true,fixture.id+': '+JSON.stringify(result));
        assert.equal(plugin.outputSchema.safeParse(result.data).success,true);
        const abort=new AbortController();abort.abort();
        const cancelled=await executeManifestCommand(manifest,'run',plugin,fixture.input,ctx,target,{signal:abort.signal});
        assert.equal(cancelled.error.code,'ABORTED');
      }
      console.log('12 real command artifacts: no React/UI/source; actual outputs, schema rejection and abort passed');
    `
    )
    const child = Bun.spawn(['node', join(root, 'probe.mjs')], {
      cwd: root,
      stdout: 'pipe',
      stderr: 'pipe',
    })
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ])
    expect(stderr).toBe('')
    expect(code).toBe(0)
    expect(stdout).toContain('12 real command artifacts')
  } finally {
    const canonical = realpathSync(root)
    expect(dirname(canonical)).toBe(realpathSync(tmpdir()))
    expect(basename(canonical).startsWith('flowtools-headless-')).toBe(true)
    rmSync(canonical, { recursive: true })
  }
}, 30_000)
