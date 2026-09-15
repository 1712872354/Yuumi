import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import i18n from "./i18n";

// 本地加载 Outfit 字体（避免依赖外部 CDN）；仅 woff2，见 src/fonts.ts
import "./fonts";

// 全局主题变量与基础样式
import "./styles/theme-tokens.css";
import "./styles/base.css";
import "./styles/naive-overrides.css";

// 禁用右键菜单
document.addEventListener("contextmenu", (e) => e.preventDefault());

const app = createApp(App);
app.use(createPinia());
app.use(i18n);
app.mount("#app");
