// ESLint 9 flat config. It replaces .eslintrc.cjs, which ESLint 9 no longer
// reads (nor its `--ext` / `--ignore-path` flags), with the same rule sets:
// Vue 3 essential, eslint:recommended and typescript-eslint recommended.
//
// Findings that predate this config are recorded in eslint-suppressions.json
// (ESLint's bulk suppressions): `npm run lint` fails on any new finding, and on
// a recorded one that no longer occurs until `npm run lint:prune` removes it.
import js from '@eslint/js';
import globals from 'globals';
import tsPlugin from '@typescript-eslint/eslint-plugin';
import tsParser from '@typescript-eslint/parser';
import pluginVue from 'eslint-plugin-vue';

export default [
  // What .gitignore excludes: dependencies and build output.
  { ignores: ['node_modules/', 'dist/', 'dist-electron/', 'release/', 'out/', 'build/'] },

  js.configs.recommended,
  ...tsPlugin.configs['flat/recommended'],
  // Sets vue-eslint-parser for .vue files, so it comes after typescript-eslint.
  ...pluginVue.configs['flat/essential'],

  {
    // <script lang="ts"> blocks go through the TypeScript parser.
    files: ['**/*.vue'],
    languageOptions: {
      parserOptions: {
        parser: tsParser,
        ecmaVersion: 'latest',
        sourceType: 'module',
      },
    },
  },
  {
    // The type checker already reports undefined names, with the real DOM,
    // Node and Electron types; `no-undef` only knows a fixed globals list.
    // typescript-eslint turns it off for .ts files; .vue scripts are TS too.
    files: ['**/*.vue'],
    rules: { 'no-undef': 'off' },
  },
  {
    // Plain JS config files run under Node.
    files: ['**/*.{js,cjs,mjs}'],
    languageOptions: { globals: globals.node },
  },
  {
    rules: {
      'vue/multi-word-component-names': 'off',
    },
  },
];
