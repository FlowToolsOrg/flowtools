import { curry } from 'es-toolkit'

import _runHello from '@plugins/plugin-example-run-hello'

import { runWebToolPlugin } from '@/runtime'

export const runHello = curry(runWebToolPlugin)(_runHello)
