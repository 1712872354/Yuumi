// 字体加载优化（替代原先 4 行 @fontsource/outfit/*.css 的 import）
// 仅加载 woff2：Tauri 使用系统 WebView2（Chromium），原生支持 woff2，woff 回退永不触发，可省约一半字体体积。
// 保留 latin + latin-ext 两个子集并带上官方 unicode-range，避免字形回退。
import latin300 from "@fontsource/outfit/files/outfit-latin-300-normal.woff2";
import latin400 from "@fontsource/outfit/files/outfit-latin-400-normal.woff2";
import latin600 from "@fontsource/outfit/files/outfit-latin-600-normal.woff2";
import latin800 from "@fontsource/outfit/files/outfit-latin-800-normal.woff2";
import latinExt300 from "@fontsource/outfit/files/outfit-latin-ext-300-normal.woff2";
import latinExt400 from "@fontsource/outfit/files/outfit-latin-ext-400-normal.woff2";
import latinExt600 from "@fontsource/outfit/files/outfit-latin-ext-600-normal.woff2";
import latinExt800 from "@fontsource/outfit/files/outfit-latin-ext-800-normal.woff2";

const LATIN_RANGE =
  "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD";
const LATIN_EXT_RANGE =
  "U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF";

type Face = { weight: number; url: string; range: string };

const faces: Face[] = [
  { weight: 300, url: latin300, range: LATIN_RANGE },
  { weight: 400, url: latin400, range: LATIN_RANGE },
  { weight: 600, url: latin600, range: LATIN_RANGE },
  { weight: 800, url: latin800, range: LATIN_RANGE },
  { weight: 300, url: latinExt300, range: LATIN_EXT_RANGE },
  { weight: 400, url: latinExt400, range: LATIN_EXT_RANGE },
  { weight: 600, url: latinExt600, range: LATIN_EXT_RANGE },
  { weight: 800, url: latinExt800, range: LATIN_EXT_RANGE },
];

const css = faces
  .map(
    (f) =>
      `@font-face{font-family:'Outfit';font-style:normal;font-weight:${f.weight};font-display:swap;src:url(${f.url}) format('woff2');unicode-range:${f.range};}`,
  )
  .join("");

const style = document.createElement("style");
style.setAttribute("data-outfit-fonts", "");
style.textContent = css;
document.head.appendChild(style);
