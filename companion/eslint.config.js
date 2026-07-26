import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import cssModules from 'eslint-plugin-css-modules'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

const wrapCssModulesRule = (rule) => {
  if (!rule?.create) {
    return rule
  }

  return {
    ...rule,
    create(context) {
      const compatContext = Object.create(context)

      compatContext.getFilename = () => {
        if (typeof context.getFilename === 'function') {
          return context.getFilename()
        }

        if (typeof context.filename === 'string') {
          return context.filename
        }

        if (typeof context.getPhysicalFilename === 'function') {
          return context.getPhysicalFilename()
        }

        return ''
      }

      return rule.create(compatContext)
    },
  }
}

const cssModulesCompat = {
  ...cssModules,
  rules: Object.fromEntries(
    Object.entries(cssModules.rules).map(([name, rule]) => [name, wrapCssModulesRule(rule)]),
  ),
}

export default defineConfig([
  globalIgnores(['dist', 'src-tauri', '**/*.d.css.ts']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      globals: globals.browser,
    },
    rules: {
      quotes: ['error', 'single', { avoidEscape: true, allowTemplateLiterals: true }],
      'css-modules/no-unused-class': 'error',
      'css-modules/no-undef-class': 'error',
    },
    plugins: {
      'css-modules': cssModulesCompat,
    },
  },
])
