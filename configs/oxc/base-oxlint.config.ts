import { defineConfig } from 'oxlint'

export default defineConfig({
  jsPlugins: ['oxlint-tailwindcss'],
  categories: {
    correctness: 'warn',
  },
  rules: {
    'no-console': 'warn',
    'no-unused-vars': [
      'error',
      {
        fix: {
          imports: 'safe-fix',
          variables: 'off',
        },
      },
    ],
    'typescript/no-floating-promises': 'error',
    'typescript/no-unsafe-assignment': 'warn',

    '@typescript-eslint/no-unused-vars': [
      'warn',
      {
        args: 'after-used',
        ignoreRestSiblings: false,
        argsIgnorePattern: '^_.*?$',
      },
    ],
    'react/self-closing-comp': 'warn',

    'no-floating-promises': 'off',
    // Correctness
    // Exact static stylesheet classes, shared by root and workspace lint.
    // Unknown utilities still fail; no wildcard prefix exemption.
    'tailwindcss/no-unknown-classes': [
      'error',
      {
        allowlist: [
          'flowtools-appearance',
          'appearance-lab',
          'appearance-lab__header',
          'appearance-lab__eyebrow',
          'appearance-lab__layout',
          'appearance-lab__controls',
          'appearance-lab__check',
          'appearance-lab__apply',
          'appearance-lab__saved',
          'appearance-lab__recovery',
          'appearance-lab__stage',
          'appearance-lab__preview-label',
          'appearance-lab__canvas',
          'appearance-lab__workspace',
          'appearance-lab__mark',
          'appearance-lab__count',
          'appearance-lab__feature',
          'appearance-lab__cards',
          'appearance-lab__motion',
        ],
      },
    ],
    'tailwindcss/no-duplicate-classes': 'error',
    'tailwindcss/no-conflicting-classes': 'error',
    'tailwindcss/no-deprecated-classes': 'error',
    'tailwindcss/no-unnecessary-whitespace': 'error',
    'tailwindcss/no-dark-without-light': 'warn',
    'tailwindcss/no-contradicting-variants': 'warn',
    // Style
    'tailwindcss/enforce-canonical': 'warn',
    'tailwindcss/enforce-sort-order': 'warn',
    'tailwindcss/enforce-shorthand': 'warn',
    'tailwindcss/enforce-logical': 'off',
    'tailwindcss/enforce-physical': 'off',
    'tailwindcss/enforce-consistent-important-position': 'warn',
    'tailwindcss/enforce-negative-arbitrary-values': 'warn',
    'tailwindcss/enforce-consistent-variable-syntax': 'warn',
    'tailwindcss/consistent-variant-order': 'warn',
    // Complexity
    'tailwindcss/max-class-count': 'off',
    'tailwindcss/enforce-consistent-line-wrapping': 'off',
    // Restrictions
    'tailwindcss/no-restricted-classes': 'off',
    'tailwindcss/no-arbitrary-value': 'off',
    'tailwindcss/no-hardcoded-colors': 'warn',
    'tailwindcss/no-unnecessary-arbitrary-value': 'warn',
    'tailwindcss/prefer-theme-tokens': 'error',
  },
  plugins: ['import', 'oxc', 'react', 'promise', 'typescript'],
  ignorePatterns: [
    'node_modules/**',
    'dist/**',
    'src-tauri/**',
    'src/utils/bindings.ts',
  ],
})
