const $ = (id) => document.getElementById(id);

const PREVIEW_DPI = 120;

function showError(msg) {
  const el = $("file-empty");
  if (el) {
    el.textContent = msg;
    el.style.display = "";
    el.style.color = "#c06050";
  }
}

// Check Tauri API availability
if (!window.__TAURI__) {
  showError("Tauri API 未加载。请确认应用完整安装。");
  throw new Error("window.__TAURI__ is not available");
}

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const state = {
  filePath: null,
  totalPages: 0,
  running: false,
  previewPage: 0,
};

const el = {
  fileEmpty: $("file-empty"),
  fileInfo: $("file-info"),
  fileName: $("file-name"),
  fileMeta: $("file-meta"),
  filePages: $("file-pages"),
  printer: $("printer-select"),
  rangeInput: $("range-input"),
  rangeHint: $("range-hint"),
  copies: $("copies-input"),
  dpi: $("dpi-input"),
  paperSize: $("paper-size"),
  orientation: $("orientation"),
  colorMode: $("color-mode"),
  duplex: $("duplex"),
  collate: $("collate"),
  progressArea: $("progress-area"),
  progressBar: $("progress-bar"),
  progressText: $("progress-text"),
  btnPrint: $("btn-print"),
  btnCancel: $("btn-cancel"),
  btnBrowse: $("btn-browse"),
  btnRefreshPrinters: $("btn-refresh-printers"),
  btnClearHistory: $("btn-clear-history"),
  historyList: $("history-list"),
  historyEmpty: $("history-empty"),
  previewCard: $("preview-card"),
  previewImg: $("preview-img"),
  previewInfo: $("preview-page-info"),
  previewJump: $("preview-jump"),
  btnPrev: $("btn-prev"),
  btnNext: $("btn-next"),
  btnJump: $("btn-jump"),
  statusDot: $("status-dot"),
  overlay: $("drag-overlay"),
};

async function init() {
  await loadPrinters();
  await applySettings();
  await loadHistory();

  el.btnBrowse.addEventListener("click", doBrowse);
  el.btnPrint.addEventListener("click", doPrint);
  el.btnCancel.addEventListener("click", doCancel);
  el.btnRefreshPrinters.addEventListener("click", loadPrinters);
  el.btnClearHistory.addEventListener("click", doClearHistory);
  el.btnPrev.addEventListener("click", () => changePage(-1));
  el.btnNext.addEventListener("click", () => changePage(1));
  el.btnJump.addEventListener("click", doJump);
  el.previewJump.addEventListener("keydown", (e) => { if (e.key === "Enter") doJump(); });
  el.rangeInput.addEventListener("input", updateRangeHint);

  // persist settings on change
  for (const el of [
    document.getElementById("printer-select"),
    document.getElementById("dpi-input"),
    document.getElementById("copies-input"),
    document.getElementById("paper-size"),
    document.getElementById("orientation"),
    document.getElementById("color-mode"),
    document.getElementById("duplex"),
    document.getElementById("collate"),
  ]) {
    if (el) el.addEventListener("change", saveCurrentSettings);
  }

  await listen("progress", onProgress);

  await listen("tauri://drag-enter", () => { el.overlay.classList.add("active"); });
  await listen("tauri://drag-leave", () => { el.overlay.classList.remove("active"); });
  await listen("tauri://drag-drop", onDragDrop);
}

let _rangeTimer = 0;

async function applySettings() {
  try {
    const s = await invoke("load_settings");
    if (s.printer && el.printer.querySelector(`option[value="${s.printer}"]`)) {
      el.printer.value = s.printer;
    }
    el.dpi.value = s.dpi || 200;
    el.copies.value = s.copies || 1;
    if (s.paper_size) el.paperSize.value = s.paper_size;
    if (s.orientation) el.orientation.value = s.orientation;
    if (s.color_mode) el.colorMode.value = s.color_mode;
    if (s.duplex) el.duplex.value = s.duplex;
    el.collate.checked = s.collate !== false;
  } catch (_) {}
}

async function saveCurrentSettings() {
  try {
    await invoke("save_settings", {
      s: {
        printer: el.printer.value || null,
        dpi: parseInt(el.dpi.value) || 200,
        copies: parseInt(el.copies.value) || 1,
        paper_size: parseInt(el.paperSize.value) || 9,
        orientation: parseInt(el.orientation.value) || 1,
        color_mode: parseInt(el.colorMode.value) || 2,
        duplex: parseInt(el.duplex.value) || 1,
        collate: el.collate.checked,
      },
    });
  } catch (_) {}
}
function updateRangeHint() {
  clearTimeout(_rangeTimer);
  _rangeTimer = setTimeout(async () => {
    if (!state.totalPages) return;
    const text = el.rangeInput.value.trim();
    if (!text) {
      el.rangeHint.textContent = `共 ${state.totalPages} 页`;
      el.rangeHint.style.color = "";
      return;
    }
    try {
      const pages = await invoke("parse_range", { text, maxPages: state.totalPages });
      el.rangeHint.textContent = `已选 ${pages.length} 页`;
      el.rangeHint.style.color = "";
    } catch (_) {
      el.rangeHint.textContent = "范围格式错误";
      el.rangeHint.style.color = "#c06050";
    }
  }, 300);
}

async function loadPrinters() {
  try {
    const printers = await invoke("get_printers");
    const def = await invoke("get_default_printer");
    el.printer.innerHTML = printers
      .map((p) => `<option value="${p}" ${p === def ? "selected" : ""}>${p}</option>`)
      .join("");
    if (!printers.length) {
      el.printer.innerHTML = '<option>(未检测到打印机)</option>';
    }
  } catch (_) {
    el.printer.innerHTML = '<option>(未检测到打印机)</option>';
  }
}

async function doBrowse() {
  const path = await invoke("browse_pdf");
  if (!path) return;
  openFile(path);
}

function onDragDrop(event) {
  el.overlay.classList.remove("active");
  const paths = event.payload?.paths || [];
  const pdf = paths.find((p) => p.toLowerCase().endsWith(".pdf"));
  if (pdf) openFile(pdf);
}

async function openFile(path) {
  try {
    const info = await invoke("open_pdf", { path });
    state.filePath = path;
    state.totalPages = info.pages;

    el.fileEmpty.style.display = "none";
    el.fileInfo.style.display = "";
    el.fileName.textContent = path.split("\\").pop() || path;
    el.fileMeta.textContent = info.size;
    el.filePages.textContent = `${info.pages} 页`;
    el.rangeInput.value = `1-${info.pages}`;
    el.rangeHint.textContent = `共 ${info.pages} 页`;
    el.btnPrint.disabled = false;

    el.previewCard.style.display = "";
    state.previewPage = 0;
    el.previewJump.max = state.totalPages;
    showPage(0);
  } catch (e) {
    el.fileEmpty.textContent = `错误: ${e}`;
    el.fileEmpty.style.display = "";
    el.fileInfo.style.display = "none";
  }
}

async function doPrint() {
  if (state.running) return;
  if (!state.filePath) return;

  const printer = el.printer.value;
  if (!printer || printer.startsWith("(")) return;

  const dpi = parseInt(el.dpi.value) || 200;
  const copies = parseInt(el.copies.value) || 1;

  let pages;
  try {
    pages = await invoke("parse_range", {
      text: el.rangeInput.value.trim(),
      maxPages: state.totalPages,
    });
  } catch (e) {
    el.progressText.textContent = `页码范围错误: ${e}`;
    return;
  }

  if (!pages.length) {
    el.progressText.textContent = "页码范围为空";
    return;
  }

  state.running = true;
  el.btnPrint.disabled = true;
  el.btnCancel.disabled = false;
  el.btnBrowse.disabled = true;
  el.progressArea.style.display = "";
  el.progressBar.style.width = "0%";
  el.progressText.textContent = "准备中...";

  await invoke("start_print", {
    path: state.filePath,
    printerName: printer,
    pages,
    dpi,
    copies,
    paperSize: parseInt(el.paperSize.value) || 9,
    orientation: parseInt(el.orientation.value) || 1,
    colorMode: parseInt(el.colorMode.value) || 2,
    duplex: parseInt(el.duplex.value) || 1,
    collate: el.collate.checked,
  });
}

async function doCancel() {
  await invoke("cancel_print");
  el.btnCancel.disabled = true;
  el.progressText.textContent = "正在取消...";
}

function onProgress(event) {
  const d = event.payload;
  if (!d) return;

  const pct = d.total > 0 ? Math.round((d.current / d.total) * 100) : 0;
  el.progressBar.style.width = `${pct}%`;
  el.progressText.textContent = d.status || "";

  if (d.type === "done" || d.type === "error") {
    state.running = false;
    el.btnPrint.disabled = false;
    el.btnCancel.disabled = true;
    el.btnBrowse.disabled = false;
    el.progressArea.style.display = "none";
    if (d.type === "done") {
      loadHistory();
    }
  }
}

async function showPage(idx) {
  state.previewPage = idx;
  const pn = idx + 1;
  el.previewInfo.textContent = `${pn} / ${state.totalPages}`;
  el.previewJump.value = pn;
  el.btnPrev.disabled = idx === 0;
  el.btnNext.disabled = idx >= state.totalPages - 1;
  el.previewImg.src = "";
  try {
    const uri = await invoke("preview_page", {
      path: state.filePath,
      pageNum: idx,
      previewDpi: PREVIEW_DPI,
    });
    el.previewImg.src = uri;
  } catch (e) {
    el.previewImg.alt = `error: ${e}`;
  }
}

function changePage(delta) {
  const next = state.previewPage + delta;
  if (next >= 0 && next < state.totalPages) {
    showPage(next);
  }
}

function doJump() {
  const p = parseInt(el.previewJump.value);
  if (p >= 1 && p <= state.totalPages) {
    showPage(p - 1);
  }
}

async function loadHistory() {
  try {
    const records = await invoke("get_history");
    if (!records.length) {
      el.historyList.style.display = "none";
      el.historyEmpty.style.display = "";
      return;
    }
    el.historyEmpty.style.display = "none";
    el.historyList.style.display = "";
    el.historyList.innerHTML = records
      .map((r) => {
        let statusClass = "h-status-done";
        if (r.status === "已取消") statusClass = "h-status-cancel";
        else if (r.status === "部分失败") statusClass = "h-status-fail";
        return `<div class="history-item">
          <div>
            <div class="h-file">${r.file}</div>
            <div class="h-time">${r.time}</div>
          </div>
          <div class="h-meta">
            <div class="h-pages">${r.pages} 页</div>
            <div class="h-status ${statusClass}">${r.status} · ${r.duration}</div>
          </div>
        </div>`;
      })
      .join("");
  } catch (_) {}
}

async function doClearHistory() {
  await invoke("clear_history");
  await loadHistory();
}

init().catch(console.error);
