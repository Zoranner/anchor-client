import js from "@eslint/js";
import pluginVue from "eslint-plugin-vue";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      ".nuxt/**",
      ".output/**",
      "node_modules/**",
      "src-tauri/target/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/essential"],
  {
    files: ["**/*.{ts,vue}"],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
      },
    },
    rules: {
      // Nuxt 自动导入由 `nuxt typecheck` 提供完整的名称解析。
      "no-undef": "off",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { ignoreRestSiblings: true },
      ],
    },
  },
  {
    files: ["app/pages/**/*.vue"],
    rules: {
      // Nuxt 以文件名定义路由，单词文件名是既定路由契约。
      "vue/multi-word-component-names": "off",
    },
  },
);
