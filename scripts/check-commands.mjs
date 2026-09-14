#!/usr/bin/env node
/**
 * 校验 #[tauri::command] 定义、lib.rs invoke_handler 注册、前端 invoke 调用三者一致。
 *
 * 检查项：
 * 1. 已定义但未在 invoke_handler 注册的命令（错误）
 * 2. invoke_handler 注册了但源码中找不到 #[tauri::command] 的命令（错误）
 * 3. 前端 invoke("...") 调用了未注册的命令（错误）
 * 4. 已注册但前端零调用的命令（警告，不失败）
 * 5. 前端 listen("...") 但 Rust 无对应 emit 的事件（错误）
 * 6. Rust emit("...") 但前端无对应 listen 的事件（警告，不失败）
 *
 * 用法: node scripts/check-commands.mjs
 * 退出码: 0 通过, 1 发现问题
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(fileURLToPath(new URL(".", import.meta.url)), "..");
const RUST_SRC = join(ROOT, "src-tauri", "src");
const FRONTEND_SRC = join(ROOT, "src");
const LIB_RS = join(RUST_SRC, "lib.rs");

/** 递归收集指定扩展名文件 */
function walk(dir, exts, out = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const name of entries) {
    if (name === "node_modules" || name === "target" || name === "dist" || name === ".git") {
      continue;
    }
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) {
      walk(full, exts, out);
    } else if (exts.some((e) => name.endsWith(e))) {
      out.push(full);
    }
  }
  return out;
}

/**
 * 从 Rust 源码提取 #[tauri::command] 标注的函数名。
 * 匹配属性后的下一行 fn / async fn / pub fn / pub async fn。
 */
function extractRustCommands(rustFiles) {
  const defined = new Map(); // name -> file:line
  for (const file of rustFiles) {
    const text = readFileSync(file, "utf8");
    const lines = text.split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
      if (!/^\s*#\[tauri::command\]/.test(lines[i])) continue;
      // 向后最多找 5 行，跳过可能的注释/属性
      for (let j = i + 1; j < Math.min(i + 6, lines.length); j++) {
        const m = lines[j].match(
          /^(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>]*>)?\s*\(/,
        );
        if (m) {
          defined.set(m[1], `${relative(ROOT, file).replace(/\\/g, "/")}:${j + 1}`);
          break;
        }
      }
    }
  }
  return defined;
}

/**
 * 从 lib.rs 的 generate_handler![...] 提取注册列表。
 * 支持 bare 名与 path::to::command 形式，统一取末段函数名。
 */
function extractRegistered(libText) {
  const start = libText.indexOf("generate_handler!");
  if (start < 0) {
    throw new Error("lib.rs 中未找到 generate_handler!");
  }
  const bracketStart = libText.indexOf("[", start);
  if (bracketStart < 0) {
    throw new Error("generate_handler! 后未找到 [");
  }
  // 找到匹配的 ]
  let depth = 0;
  let end = -1;
  for (let i = bracketStart; i < libText.length; i++) {
    if (libText[i] === "[") depth++;
    else if (libText[i] === "]") {
      depth--;
      if (depth === 0) {
        end = i;
        break;
      }
    }
  }
  if (end < 0) {
    throw new Error("generate_handler![...] 括号未闭合");
  }
  const body = libText.slice(bracketStart + 1, end);
  const registered = new Map(); // shortName -> fullPath
  // 去掉注释
  const cleaned = body.replace(/\/\/[^\n]*/g, "");
  for (const raw of cleaned.split(",")) {
    const token = raw.trim();
    if (!token) continue;
    // token 形如 greet / lcu::client::call_lcu_api
    const parts = token.split("::").map((s) => s.trim()).filter(Boolean);
    if (parts.length === 0) continue;
    const short = parts[parts.length - 1];
    registered.set(short, token);
  }
  return registered;
}

/** 从前端 ts/vue 提取 invoke("commandName")（支持嵌套泛型与跨行） */
function extractFrontendInvokes(frontFiles) {
  const calls = new Map(); // name -> Set("file:line")
  const callRe = /\binvoke\s*(?:<[^<>]*(?:<[^<>]*>[^<>]*)*>)?\s*\(/g;
  for (const file of frontFiles) {
    const text = readFileSync(file, "utf8");
    const lines = text.split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
      callRe.lastIndex = 0;
      if (!callRe.test(lines[i])) continue;
      // 从本行 invoke( 之后向下最多扫 3 行，取第一个字符串
      for (let j = i; j < Math.min(i + 4, lines.length); j++) {
        const slice = j === i ? lines[i].slice(callRe.lastIndex) : lines[j];
        const strM = slice.match(/["'`]([A-Za-z_][A-Za-z0-9_]*)["'`]/);
        if (strM) {
          const name = strM[1];
          if (!calls.has(name)) calls.set(name, new Set());
          calls.get(name).add(`${relative(ROOT, file).replace(/\\/g, "/")}:${j + 1}`);
          break;
        }
      }
    }
  }
  return calls;
}

/** 从 Rust 提取字面量 emit / emit_to / emit_filter 事件名（支持跨行调用） */
function extractRustEmits(rustFiles) {
  const emits = new Map(); // event -> Set("file:line")
  const callRe = /\.(emit|emit_to|emit_filter)\s*(?:<[^>]*>)?\s*\(/g;
  for (const file of rustFiles) {
    const text = readFileSync(file, "utf8");
    const lines = text.split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
      callRe.lastIndex = 0;
      const m = callRe.exec(lines[i]);
      if (!m) continue;
      const kind = m[1];
      // 从本行 emit( 之后开始，向下最多扫 8 行收集字符串，直到括号深度归零
      const strings = [];
      let depth = 1;
      let started = false;
      for (let j = i; j < Math.min(i + 10, lines.length); j++) {
        const slice = j === i ? lines[i].slice(callRe.lastIndex) : lines[j];
        for (let k = 0; k < slice.length; k++) {
          const ch = slice[k];
          if (ch === "(") {
            depth++;
            started = true;
          } else if (ch === ")") {
            depth--;
            if (depth === 0 && started) break;
          } else if (ch === '"' || ch === "'" || ch === "`") {
            const end = slice.indexOf(ch, k + 1);
            if (end > k) {
              strings.push({ value: slice.slice(k + 1, end), afterLabel: /label\s*:\s*$/.test(slice.slice(0, k)) });
              k = end;
            }
          }
        }
        if (depth === 0 && started) break;
      }
      if (strings.length === 0) continue;
      // emit / emit_filter：第一个字符串即事件名
      // emit_to(WebviewWindow { label: "win" }, "event", ...)：跳过 label 后的字符串
      let event = null;
      if (kind === "emit_to") {
        event = strings.find((s) => !s.afterLabel)?.value;
      } else {
        event = strings[0]?.value;
      }
      if (!event || event.length < 3) continue;
      if (!emits.has(event)) emits.set(event, new Set());
      emits.get(event).add(`${relative(ROOT, file).replace(/\\/g, "/")}:${i + 1}`);
    }
  }
  return emits;
}

/** 从前端提取 listen("event") / listen<...>("event")（支持跨行） */
function extractFrontendListens(frontFiles) {
  const listens = new Map(); // event -> Set("file:line")
  const callRe = /\blisten\s*(?:<[^>]*>)?\s*\(/g;
  for (const file of frontFiles) {
    const text = readFileSync(file, "utf8");
    const lines = text.split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
      callRe.lastIndex = 0;
      if (!callRe.test(lines[i])) continue;
      // 从本行 listen( 之后向下最多扫 4 行，取第一个字符串
      for (let j = i; j < Math.min(i + 5, lines.length); j++) {
        const slice = j === i ? lines[i].slice(callRe.lastIndex) : lines[j];
        const strM = slice.match(/["'`]([A-Za-z0-9_:/-]+)["'`]/);
        if (strM) {
          const name = strM[1];
          if (!listens.has(name)) listens.set(name, new Set());
          listens.get(name).add(`${relative(ROOT, file).replace(/\\/g, "/")}:${j + 1}`);
          break;
        }
      }
    }
  }
  return listens;
}

/** 从前端提取 WebviewWindow / getCurrentWindow 等的 .emit("event") */
function extractFrontendEmits(frontFiles) {
  const emits = new Map();
  const re = /\.emit\s*(?:<[^>]*>)?\s*\(\s*["'`]([A-Za-z0-9_:-]+)["'`]/g;
  for (const file of frontFiles) {
    const text = readFileSync(file, "utf8");
    const lines = text.split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
      re.lastIndex = 0;
      let m;
      while ((m = re.exec(lines[i])) !== null) {
        const name = m[1];
        if (!emits.has(name)) emits.set(name, new Set());
        emits.get(name).add(`${relative(ROOT, file).replace(/\\/g, "/")}:${i + 1}`);
      }
    }
  }
  return emits;
}

function main() {
  const rustFiles = walk(RUST_SRC, [".rs"]);
  const frontFiles = walk(FRONTEND_SRC, [".ts", ".vue"]);
  const libText = readFileSync(LIB_RS, "utf8");

  const defined = extractRustCommands(rustFiles);
  const registered = extractRegistered(libText);
  const frontInvokes = extractFrontendInvokes(frontFiles);
  const rustEmits = extractRustEmits(rustFiles);
  const frontListens = extractFrontendListens(frontFiles);
  const frontEmits = extractFrontendEmits(frontFiles);
  // listen 合法来源：Rust emit 或前端窗口间 emit
  const allEmits = new Set([...rustEmits.keys(), ...frontEmits.keys()]);

  const problems = [];
  const warnings = [];

  // 1. 已定义未注册
  for (const [name, loc] of defined) {
    if (!registered.has(name)) {
      problems.push(`[未注册] #[tauri::command] ${name} 定义于 ${loc}，但未出现在 invoke_handler`);
    }
  }

  // 2. 已注册未定义
  for (const [short, full] of registered) {
    if (!defined.has(short)) {
      problems.push(
        `[悬空注册] invoke_handler 中的 ${full}（${short}）在源码中找不到对应的 #[tauri::command]`,
      );
    }
  }

  // 3. 前端调用未注册命令
  for (const [name, locs] of frontInvokes) {
    if (!registered.has(name)) {
      for (const loc of locs) {
        problems.push(`[前端悬空] invoke("${name}") 于 ${loc}，该命令未在 invoke_handler 注册`);
      }
    }
  }

  // 4. 已注册但前端零调用（警告）
  for (const [short, full] of registered) {
    if (!frontInvokes.has(short)) {
      warnings.push(`[未使用命令] ${full} 已注册但前端无 invoke("${short}") 调用`);
    }
  }

  // 5. 前端 listen 但无任何 emit（错误）
  for (const [event, locs] of frontListens) {
    if (!allEmits.has(event)) {
      for (const loc of locs) {
        problems.push(`[事件悬空] listen("${event}") 于 ${loc}，Rust/前端源码中无对应 emit`);
      }
    }
  }

  // 6. Rust emit 但前端无 listen（警告）
  for (const [event, locs] of rustEmits) {
    if (!frontListens.has(event)) {
      for (const loc of locs) {
        warnings.push(`[未监听事件] emit("${event}") 于 ${loc}，前端无 listen`);
      }
    }
  }

  const definedCount = defined.size;
  const registeredCount = registered.size;
  const invokeCount = frontInvokes.size;

  console.log(
    `check-commands: 定义 ${definedCount} / 注册 ${registeredCount} / 前端 invoke ${invokeCount} / emit ${rustEmits.size} / listen ${frontListens.size}`,
  );

  if (warnings.length > 0) {
    console.warn(`\n警告 ${warnings.length} 条（不阻断）:\n`);
    for (const w of warnings) console.warn("  " + w);
  }

  if (problems.length === 0) {
    console.log("OK: 命令定义、注册与前端调用一致");
    process.exit(0);
  }

  console.error(`\n发现 ${problems.length} 处问题:\n`);
  for (const p of problems) {
    console.error("  " + p);
  }
  process.exit(1);
}

main();
