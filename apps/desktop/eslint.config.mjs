import eslint from '@eslint/js';
import prettier from 'eslint-config-prettier';
import react from 'eslint-plugin-react';
import tseslint from 'typescript-eslint';

const browserGlobals = {
  AudioContext: 'readonly',
  Blob: 'readonly',
  CSS: 'readonly',
  CustomEvent: 'readonly',
  Event: 'readonly',
  File: 'readonly',
  FileReader: 'readonly',
  HTMLElement: 'readonly',
  HTMLInputElement: 'readonly',
  KeyboardEvent: 'readonly',
  MouseEvent: 'readonly',
  URL: 'readonly',
  WebSocket: 'readonly',
  Window: 'readonly',
  console: 'readonly',
  document: 'readonly',
  localStorage: 'readonly',
  navigator: 'readonly',
  window: 'readonly',
};

const nodeGlobals = {
  Buffer: 'readonly',
  process: 'readonly',
  __dirname: 'readonly',
  __filename: 'readonly',
  console: 'readonly',
};

export default tseslint.config(
  {
    ignores: [
      'coverage/**',
      'dist/**',
      'src-tauri/target/**',
      'node_modules/**',
    ],
  },
  { ...eslint.configs.recommended, files: ['**/*.{js,mjs,ts,tsx}'] },
  ...tseslint.configs.recommended.map((config) => ({
    ...config,
    files: ['**/*.{ts,tsx}'],
  })),
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      globals: {
        ...browserGlobals,
      },
    },
  },
  {
    files: [
      'tests/**/*.ts',
      'scripts/**/*.mjs',
      'vite.config.ts',
      'eslint.config.mjs',
    ],
    languageOptions: { globals: nodeGlobals },
  },
  {
    ...react.configs.flat.recommended,
    files: ['**/*.{jsx,tsx}'],
    settings: {
      react: {
        version: 'detect',
      },
    },
  },
  {
    ...react.configs.flat['jsx-runtime'],
    files: ['**/*.{jsx,tsx}'],
  },
  {
    files: ['tests/**/*.ts'],
    rules: {
      '@typescript-eslint/no-explicit-any': 'off',
    },
  },
  prettier,
);
