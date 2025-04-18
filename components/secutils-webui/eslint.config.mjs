// @ts-check

import eslint from '@eslint/js';
import tsEsLint from 'typescript-eslint';
import react from 'eslint-plugin-react';
import globals from 'globals';
import eslintPluginPrettierRecommended from 'eslint-plugin-prettier/recommended';

export default tsEsLint.config(
  {
    ignores: ['dist'],
  },
  eslint.configs.recommended,
  ...tsEsLint.configs.recommended,
  {
    ...react.configs.flat.recommended,
    rules: {
      ...react.configs.flat.recommended.rules,
      'react/react-in-jsx-scope': 'off',
    },
    settings: { react: { version: 'detect' } },
  },
  {
    files: ['**/*.ts', '**/*.tsx'],
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

      // Can be re-enabled once the following issues are resolved: https://github.com/import-js/eslint-plugin-import/issues/2948
      // "import/order": ["error", {
      //   groups: ["builtin", "external", "internal", ["parent", "sibling", "index"]],
      //
      //   alphabetize: {
      //     order: "asc",
      //     caseInsensitive: true
      //   },
      //
      //   "newlines-between": "always"
      // }],
      //
      // "import/no-duplicates": ["error"],
      //
      // "sort-imports": ["error", {
      //   ignoreCase: true,
      //   ignoreDeclarationSort: true
      // }]
    },
  },
  eslintPluginPrettierRecommended,
);
