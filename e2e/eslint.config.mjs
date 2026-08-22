// @ts-check

import eslint from '@eslint/js';
import tsEsLint from 'typescript-eslint';
import globals from 'globals';
import eslintPluginPrettierRecommended from 'eslint-plugin-prettier/recommended';
import importPlugin from 'eslint-plugin-import-x';

export default tsEsLint.config(
  {
    ignores: ['dist', 'playwright-report', 'test-results'],
  },
  eslint.configs.recommended,
  ...tsEsLint.configs.recommended,
  {
    files: ['**/*.ts'],
    extends: [importPlugin.flatConfigs.recommended],
    // `flatConfigs.typescript` points at eslint-import-resolver-typescript, which is not
    // installed and predates import-x's resolver interface. import-x bundles its own resolver,
    // so the TS extensions are all that need declaring.
    settings: {
      'import-x/resolver-next': [importPlugin.createNodeResolver({ extensions: ['.ts', '.js', '.mjs', '.json'] })],
    },
    languageOptions: {
      ecmaVersion: 2018,
      sourceType: 'module',
      parserOptions: { project: ['./tsconfig.json'] },
      globals: { ...globals.browser },
    },
    rules: {
      '@typescript-eslint/explicit-function-return-type': 'off',
      '@typescript-eslint/explicit-module-boundary-types': 'off',
      '@typescript-eslint/no-non-null-assertion': 'off',
      '@typescript-eslint/consistent-type-exports': 'error',
      '@typescript-eslint/consistent-type-imports': 'error',
      '@typescript-eslint/no-unused-vars': 'error',
      '@typescript-eslint/no-unused-expressions': 'error',

      '@typescript-eslint/no-empty-object-type': ['error', { allowInterfaces: 'with-single-extends' }],

      'object-curly-spacing': ['error', 'always'],
      'max-len': ['error', { code: 120, ignoreStrings: true, ignoreTemplateLiterals: true }],
      'eol-last': ['error', 'always'],
      'no-multiple-empty-lines': ['error', { max: 1, maxEOF: 0 }],

      'import-x/order': [
        'error',
        {
          groups: ['builtin', 'external', 'internal', ['parent', 'sibling', 'index']],

          alphabetize: {
            order: 'asc',
            caseInsensitive: true,
          },

          'newlines-between': 'always',
        },
      ],

      'import-x/no-duplicates': ['error'],

      'sort-imports': [
        'error',
        {
          ignoreCase: true,
          ignoreDeclarationSort: true,
        },
      ],
    },
  },
  eslintPluginPrettierRecommended,
);
