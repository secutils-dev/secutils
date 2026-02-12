// @ts-check

import eslint from '@eslint/js';
import tsEsLint from 'typescript-eslint';
import react from 'eslint-plugin-react';
import globals from 'globals';
import eslintPluginPrettierRecommended from 'eslint-plugin-prettier/recommended';
import eslintPluginReactHooks from 'eslint-plugin-react-hooks';
import importPlugin from 'eslint-plugin-import';

export default tsEsLint.config(
  {
    ignores: ['dist'],
  },
  eslint.configs.recommended,
  ...tsEsLint.configs.recommended,
  {
    ...react.configs.flat.recommended,
    plugins: {
      ...react.configs.flat.recommended.plugins,
      'react-hooks': eslintPluginReactHooks,
    },
    rules: {
      ...react.configs.flat.recommended.rules,
      ...eslintPluginReactHooks.configs['recommended'].rules,
      'react/react-in-jsx-scope': 'off',
      'react/no-unknown-property': ['error', { ignore: ['css'] }],
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
      // Disable overly strict rules that produce false positives for legitimate patterns
      'react-hooks/set-state-in-effect': 'off',
      'react-hooks/immutability': 'off',
    },
    settings: { react: { version: 'detect' } },
  },
  {
    files: ['**/*.ts', '**/*.tsx'],
    extends: [importPlugin.flatConfigs.recommended, importPlugin.flatConfigs.typescript],
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

      'import/order': [
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

      'import/no-duplicates': ['error'],

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
