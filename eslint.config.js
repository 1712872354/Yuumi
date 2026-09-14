import js from "@eslint/js";
import tseslint from "typescript-eslint";
import pluginVue from "eslint-plugin-vue";
import vueParser from "vue-eslint-parser";
import globals from "globals";

export default tseslint.config(
  {
    ignores: [
      "node_modules/**",
      "src-tauri/**",
      "dist/**",
      "src/components.d.ts",
      "src/vite-env.d.ts",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/recommended"],
  {
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.es2021,
      },
    },
  },
  {
    files: ["**/*.vue"],
    languageOptions: {
      parser: vueParser,
      parserOptions: {
        parser: tseslint.parser,
        extraFileExtensions: [".vue"],
        sourceType: "module",
      },
    },
  },
  {
    rules: {
      // 项目规范：禁止 any
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      // 浏览器全局已由 globals 提供，关闭误报
      "no-undef": "off",
      // Vue 模板：关闭与现有代码风格冲突、且 Prettier 已覆盖的规则
      "vue/multi-word-component-names": "off",
      "vue/no-v-html": "off",
      "vue/max-attributes-per-line": "off",
      "vue/singleline-html-element-content-newline": "off",
      "vue/multiline-html-element-content-newline": "off",
      "vue/attributes-order": "off",
      "vue/html-self-closing": "off",
      "vue/attribute-hyphenation": "off",
      "vue/html-indent": "off",
      "vue/html-closing-bracket-newline": "off",
      "vue/first-attribute-linebreak": "off",
      "vue/html-closing-bracket-spacing": "off",
      "vue/no-multiple-template-root": "off",
      // 空 catch 已在代码中常见（localStorage 等）
      "no-empty": ["warn", { allowEmptyCatch: true }],
      // TS 项目由 tsc 负责类型检查
      "preserve-caught-error": "off",
    },
  },
);
