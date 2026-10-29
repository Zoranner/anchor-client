import tailwindcss from "@tailwindcss/vite";

export default defineNuxtConfig({
  srcDir: "app/",
  ssr: false,
  modules: ["@nuxt/icon", "@stellar/ui/nuxt"],
  icon: {
    provider: "none",
    componentName: "NuxtIcon",
    clientBundle: {
      scan: {
        globInclude: ["**/*.{vue,jsx,tsx,ts,md,mdc,mdx,yml,yaml}"],
      },
    },
  },
  compatibilityDate: "2026-08-19",
  devServer: {
    port: 18082,
  },
  css: ["~/assets/css/main.css"],
  vite: {
    plugins: [tailwindcss()],
    server: {
      strictPort: true,
    },
  },
  typescript: {
    strict: true,
  },
  nitro: {
    externals: {
      inline: ["@vue/shared"],
    },
  },
  devtools: {
    enabled: true,
  },
});
