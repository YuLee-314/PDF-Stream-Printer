# PDF 流式打印器

**大文件 · 流式渲染 · 逐页打印**

一款 Windows 桌面工具，专门解决超大 PDF 文件（> 1 GB）无法打印的问题。采用流式分页读取策略，内存占用与 PDF 大小解耦——1 GB 和 10 GB 的 PDF，内存峰值完全相同。

---

## 为什么需要这个工具

传统 PDF 软件（Adobe Acrobat、浏览器等）采用"全量加载"策略，打开大 PDF 时会将全部页面渲染数据读入内存：

```
1 GB PDF → 全部渲染为位图 → 5000 页 × 8 MB/页 ≈ 40 GB → OOM 崩溃
```

本工具采用不同策略：

```
任意大小 PDF → 只读取元数据（几 KB）→ 逐页渲染到磁盘 → 逐页发送到打印机 → 立即删除
峰值内存: ~55 MB（始终只有 1 页在内存中）
```

---

## 功能

- **任意大小 PDF 打印** — 经过 1 GB+ PDF 测试，理论上无上限
- **快速打开** — 仅读取元数据，≤ 2 秒显示文件信息，与文件大小无关
- **页面预览** — 低 DPI 下实时预览任意页面，不影响打印流程
- **自定义页码范围** — 支持 `1-10`, `1,3,5`, `1-10,15,20-30` 等格式
- **完整打印选项** — 纸张大小（A3/A4/A5/Letter/Legal/B5）、纵向/横向、彩色/灰度、单面/双面（长边翻转/短边翻转）、逐份打印
- **可调 DPI** — 72–600 DPI，平衡速度与质量
- **多份打印** — 1–99 份
- **实时进度** — 进度条 + 当前状态 + 预估剩余时间
- **随时取消** — 已提交的页面正常输出，未处理的立即停止，不浪费纸张
- **打印历史** — 本地 JSON 记录，最近 100 条
- **单页容错** — 单页渲染或打印失败不影响后续页面
- **自动清理** — 打印完成后立即删除临时文件

---

## 技术栈

| 层 | 技术 |
|---|---|
| **桌面框架** | [Tauri 2](https://v2.tauri.app/) |
| **后端语言** | Rust |
| **PDF 渲染** | [PDFium](https://pdfium.googlesource.com/pdfium/)（Chrome / Edge 同源引擎） |
| **打印 API** | Windows GDI（`CreateDC` / `StretchDIBits`） |
| **前端** | 原生 HTML / CSS / JS，零框架依赖 |

---

## 架构

```
┌──────────────────────────────────────────────────┐
│                     前端                          │
│              HTML / CSS / JS                     │
│         (深色主题 · 左右分屏布局)                   │
└──────────────┬───────────────────────────────────┘
               │  invoke()  /  listen("progress")
               │  Tauri IPC
┌──────────────▼───────────────────────────────────┐
│                  Rust 后端                        │
│                                                  │
│  main.rs      ← 命令路由 + 打印主循环              │
│  pdf_ops.rs   ← PDF 打开 / 逐页渲染 / 预览        │
│  printer.rs   ← Windows GDI 打印 + DEVMODE 配置   │
│  logger.rs    ← 打印历史 + 临时文件清理            │
│                                                  │
│  依赖:  pdfium  ·  windows  ·  image  ·  serde   │
└──────────────┬───────────────────────────────────┘
               │
┌──────────────▼───────────────────────────────────┐
│          pdfium.dll  (Chrome 同源 PDF 引擎)        │
│          Windows Print Spooler API                 │
└──────────────────────────────────────────────────┘
```

**数据流：**

```
PDF 文件 → pdf_ops::render_page()
              → 渲染到临时 PNG
                → 释放位图内存
                  → printer::print_image()
                      → Windows 打印队列
                        → 删除临时 PNG
                          → 循环下一页
```

---

## 从零开始打包（零基础手把手版）

以下操作全部在 **Windows 10/11 64 位** 上完成。每步都标注了在哪个程序中执行、输入什么、看到什么算成功。

> 预计总耗时：首次约 30–60 分钟（大部分是下载和编译等待时间）。

---

### 准备：安装终端工具

你需要用到的两个终端：

| 终端 | 如何打开 | 用途 |
|---|---|---|
| **PowerShell**（推荐） | 按 `Win + R`，输入 `powershell`，回车 | 执行脚本、下载文件 |
| **命令提示符 (CMD)** | 按 `Win + R`，输入 `cmd`，回车 | 安装 winget 包、编译 |

以下步骤中会标注 **"在 CMD 中执行"** 还是 **"在 PowerShell 中执行"**，其余未标注的两种均可。

---

### 第 1 步：安装 Rust 编程语言（10 分钟）

**在 CMD 中执行：**

```cmd
winget install Rustlang.Rustup
```

如果 `winget` 不可用，改用浏览器安装：
1. 打开 https://rustup.rs
2. 下载 `rustup-init.exe`
3. 双击运行，一路按回车（全部用默认选项）

**验证安装成功** — 关闭当前 CMD 窗口，重新打开一个新的 CMD，输入：

```cmd
rustc --version
```

**预期输出类似：** `rustc 1.85.0 (4d91de4c4 2026-02-17)`

> 版本号 ≥ 1.75 即可，具体数字无所谓。

---

### 第 2 步：安装 C++ 编译工具（15 分钟）

Rust 编译需要 Windows 的 C++ 链接器。

**在 CMD 中执行：**

```cmd
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

如果 winget 不可用，手动安装：
1. 打开 https://visualstudio.microsoft.com/zh-hans/downloads/
2. 找到"Visual Studio 2022 生成工具"，点击下载
3. 运行安装程序，勾选 **"使用 C++ 的桌面开发"** 工作负载
4. 点击安装，等待完成（约 3–5 GB 下载量）

**验证安装成功** — 打开新的 CMD，输入：

```cmd
where link.exe
```

**预期输出类似：** `C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\...\bin\Hostx64\x64\link.exe`

> 只要显示了 `link.exe` 的路径就算成功。

---

### 第 3 步：安装 Tauri CLI（3 分钟）

Tauri CLI 是将代码打包成 exe 的命令行工具。

**在 CMD 中执行：**

```cmd
cargo install tauri-cli --version "^2"
```

这一步会下载并编译 Tauri CLI，约 2–5 分钟，中间会有大量编译日志输出，正常现象。

**验证安装成功：**

```cmd
cargo tauri --version
```

**预期输出类似：** `cargo-tauri 2.x.x`

---

### 第 4 步：获取 PDFium 渲染引擎（2 分钟）

PDFium 是 Chrome 浏览器使用的 PDF 渲染引擎，以 `pdfium.dll` 文件的形式提供。这个文件必须放到 `src-tauri/` 目录下。

#### 方式 A — 自动下载脚本（推荐）

项目自带下载脚本。**在 PowerShell 中执行：**

```powershell
cd "D:\学习资料\考研\数学\大内存打印器\protect"
.\get-pdfium.ps1
```

> 如果提示"无法加载文件，因为在此系统上禁止运行脚本"，先执行：
> ```powershell
> Set-ExecutionPolicy -Scope CurrentUser -ExecutionPolicy RemoteSigned
> ```
> 输入 `Y` 确认，再重新运行 `.\get-pdfium.ps1`。

**预期输出：**
```
Downloading: https://ghproxy.net/...
OK: pdfium.dll copied to src-tauri\
```

**验证：** 确认 `src-tauri\pdfium.dll` 文件存在，大小约 20 MB。

#### 方式 B — 手动下载（脚本失败时用）

1. 浏览器打开： https://github.com/bblanchon/pdfium-binaries/releases/latest
2. 找到 Assets 列表，下载文件名包含 **`pdfium-win-x64`** 的压缩包（`.zip` 或 `.tgz`）
   - 注意：不要选带 `v8` 字样的
   - 如果 GitHub 打不开，用镜像：https://ghproxy.net/https://github.com/bblanchon/pdfium-binaries/releases/latest
3. 解压压缩包，找到里面的 `pdfium.dll`
4. 把 `pdfium.dll` 复制到 `protect\src-tauri\` 目录下

---

### 第 5 步：编译打包（首次 10–20 分钟）

**在 CMD 中执行：**

```cmd
cd /d "D:\学习资料\考研\数学\大内存打印器\protect"
cargo tauri build
```

首次编译会从网络下载所有 Rust 依赖包（约 300–500 个 crate），需要 10–20 分钟，取决于网络速度。

中途你会看到的正常输出：
- `Downloading ...` — 正在下载依赖
- `Compiling ...` — 正在编译（会刷几百行，正常）
- `Finished release [optimized] target(s)` — Rust 部分编译完成
- `Bundling ...` — 正在打包为 NSIS 安装包

**编译完成的标志：** 最后一行显示成功信息，没有 `error` 字样。

**验证编译成功 — 找到你的 exe 文件：**

在文件资源管理器中打开：

```
protect\src-tauri\target\release\
```

你会看到：

| 文件 | 说明 |
|---|---|
| `pdf-printer.exe` | 独立可执行文件，可直接双击运行 |
| `bundle/msi/` | MSI 安装包目录（发给别人用这个安装） |
| `bundle/nsis/` | NSIS 安装包目录 |

**验证 exe 可运行：** 双击 `pdf-printer.exe`，应弹出一个独立的程序窗口。

> 注意：`pdf-printer.exe` 必须与 `pdfium.dll` 在同一目录才能运行。Tauri 打包时会自动把 dll 包含进去（`tauri.conf.json` 中已配置 `"resources": ["pdfium.dll"]`）。

---

### 第 6 步（可选）：生成图标

如果需要自定义程序图标，替换 `src-tauri/icons/` 目录下的图标文件，然后重新执行第 5 步编译。

---

### 开发调试模式

如果你需要修改代码后实时预览效果（不用每次都重新编译）：

```cmd
:: 终端 1：启动前端开发服务器
cd /d "D:\学习资料\考研\数学\大内存打印器\protect\src"
python -m http.server 1420

:: 终端 2：启动 Tauri 开发模式（支持热重载）
cd /d "D:\学习资料\考研\数学\大内存打印器\protect"
cargo tauri dev
```

> 前端代码（HTML/CSS/JS）修改后刷新窗口即可生效；Rust 代码修改后会自动重新编译。

---

### 常见报错速查表

| # | 错误信息关键字 | 原因 | 修复方法 |
|---|---|---|---|
| 1 | `link.exe not found` | 未安装 C++ 编译工具 | 回到**第 2 步**，安装 VS Build Tools 并确保勾选"C++ 桌面开发" |
| 2 | `pdfium.dll not found` 或 `load pdfium: ...` | PDFium 动态库缺失 | 回到**第 4 步**，确保 `pdfium.dll` 已放入 `src-tauri/` 目录 |
| 3 | `cargo : 无法将"cargo"项识别为...` | Rust 未安装或环境变量未生效 | 关闭终端重新打开，再试。若仍不行，回到**第 1 步**重新安装 |
| 4 | `error: failed to run custom build command for 'tauri-build'` | WebView2 缺失（极少见，Win10 早期版本可能缺少） | 下载安装 [WebView2 运行时](https://developer.microsoft.com/microsoft-edge/webview2/) |
| 5 | `memory allocation of ... failed` 或 `rustc exited with ...` | 内存不足（编译大型依赖时需要 4 GB+ 空闲内存） | 关闭其他大型程序（浏览器、IDE），释放内存后重试 |
| 6 | `Get-WmiObject` 或 PowerShell 脚本报错 | PowerShell 执行策略限制 | 以管理员身份打开 PowerShell，执行 `Set-ExecutionPolicy RemoteSigned` |
| 7 | `error: could not compile ... (signal: 9)` | 被杀毒软件拦截 | 临时关闭杀毒软件（Windows Defender 或第三方），编译完再开启 |
| 8 | `无法加载文件 ... 禁止运行脚本` | PowerShell 安全策略阻止脚本 | 在 PowerShell 中执行：`Set-ExecutionPolicy -Scope CurrentUser -ExecutionPolicy RemoteSigned` |

---

## 使用

1. 启动程序，点击 **选择 PDF 文件**，自动显示文件大小和页数
2. 在右侧预览区可翻页查看内容
3. 选择打印机（已自动选中系统默认打印机）
4. 输入打印范围（默认全部），设置 DPI、份数、纸张等选项
5. 点击 **开始打印**，观察进度条
6. 可随时点击 **取消** 中断打印

---

## 内存模型对比

| | 传统 PDF 软件 | 本工具 |
|---|---|---|
| 打开 1 GB PDF | ~5–10 GB 内存 | ~5 MB |
| 打印 5000 页 (300 DPI) | ~40 GB 临时位图 | ~55 MB 峰值 |
| 能否打开 5 GB PDF | 大概率崩溃 | 正常 |
| UI 是否卡顿 | 冻结 | 始终流畅 |
| 能否取消打印 | 难以中断 | 即时生效 |

---

## 打包体积

| 组件 | 大小 |
|---|---|
| Tauri shell | ~3 MB |
| Rust 代码 | ~5 MB |
| PDFium 引擎 | ~20 MB |
| Web 前端 | ~20 KB |
| **合计** | **~28–35 MB** |

---

## 限制

- 仅支持 Windows 10 / 11 64 位
- 不提供 PDF 阅读/编辑功能（这是有意为之——预览功能正是导致 OOM 的根源之一）
- 无页面缩略图预览（仅单页预览）

---

## License

MIT
