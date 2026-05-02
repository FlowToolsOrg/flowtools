import { defineConfig } from 'oxlint'

export default defineConfig({
  jsPlugins: ['oxlint-tailwindcss'],
  categories: {
    correctness: 'warn',
  },
  rules: {
    'no-console': 'warn',
    'eslint/no-unused-vars': 'error',
    'eslint/unused-imports': 'error',
    'react/prop-types': 'off',
    'react/jsx-uses-react': 'off',
    'react/react-in-jsx-scope': 'off',
    'react-hooks/exhaustive-deps': 'off',
    'jsx-a11y/click-events-have-key-events': 'warn',
    'jsx-a11y/interactive-supports-focus': 'warn',

    '@typescript-eslint/no-unused-vars': [
      'warn',
      {
        args: 'after-used',
        ignoreRestSiblings: false,
        argsIgnorePattern: '^_.*?$',
      },
    ],
    'react/self-closing-comp': 'warn',

    'react/jsx-sort-props': [
      'warn',
      {
        callbacksLast: true,
        shorthandFirst: true,
        noSortAlphabetically: false,
        reservedFirst: true,
      },
    ],

    'padding-line-between-statements': [
      'warn',
      {
        blankLine: 'always',
        prev: '*',
        next: 'return',
      },
      {
        blankLine: 'always',
        prev: ['const', 'let', 'var'],
        next: '*',
      },
      {
        blankLine: 'any',
        prev: ['const', 'let', 'var'],
        next: ['const', 'let', 'var'],
      },
    ],
    'no-floating-promises': 'off',
    // Correctness
    'tailwindcss/no-unknown-classes': 'error',
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
  },
  plugins: ['import', 'oxc', 'react', 'promise', 'typescript'],
  ignorePatterns: [
    'node_modules/**',
    'dist/**',
    'src-tauri/**',
    'src/utils/bindings.ts',
  ],
})
