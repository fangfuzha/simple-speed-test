const state = {
  token: null,
  config: null,
  running: false,
  logs: [],
  progress: 0,
};

const els = {
  start: document.getElementById('btnStart'),
  reset: document.getElementById('btnReset'),
  log: document.getElementById('log'),
  progress: document.getElementById('progressBar'),
  statusDot: document.getElementById('statusDot'),
  statusText: document.getElementById('statusText'),
  nodeName: document.getElementById('nodeName'),
  modeName: document.getElementById('modeName'),
  sessionState: document.getElementById('sessionState'),
  estimatedTime: document.getElementById('estimatedTime'),
  planNote: document.getElementById('planNote'),
  downloadValue: document.getElementById('downloadValue'),
  uploadValue: document.getElementById('uploadValue'),
  pingValue: document.getElementById('pingValue'),
  jitterValue: document.getElementById('jitterValue'),
  lossValue: document.getElementById('lossValue'),
};

function pushLog(message) {
  state.logs.unshift({ message, time: new Date().toLocaleTimeString() });
  els.log.innerHTML = state.logs.slice(0, 20).map((item) => (
    `<div class="log-item"><strong>${item.time}</strong><br>${escapeHtml(item.message)}</div>`
  )).join('');
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function setStatus(text, running) {
  els.statusText.textContent = text;
  els.statusDot.classList.toggle('running', running);
}

function setProgress(value) {
  state.progress = Math.max(0, Math.min(100, value));
  els.progress.style.width = `${state.progress}%`;
}

function median(values) {
  const filtered = values.filter((value) => Number.isFinite(value)).sort((a, b) => a - b);
  if (!filtered.length) return 0;
  const mid = Math.floor(filtered.length / 2);
  return filtered.length % 2 === 0
    ? (filtered[mid - 1] + filtered[mid]) / 2
    : filtered[mid];
}

function trimmedMedian(values) {
  const filtered = values.filter((value) => Number.isFinite(value)).sort((a, b) => a - b);
  if (filtered.length >= 5) {
    return median(filtered.slice(1, -1));
  }
  return median(filtered);
}

function formatMbps(value) {
  if (!Number.isFinite(value) || value <= 0) return '-';
  return `${value.toFixed(2)} Mbps`;
}

function formatMs(value) {
  if (!Number.isFinite(value) || value <= 0) return '-';
  return `${value.toFixed(1)} ms`;
}

function formatLoss(value) {
  if (!Number.isFinite(value) || value < 0) return '-';
  return `${value.toFixed(1)} %`;
}

async function fetchJson(url, options = {}) {
  const response = await fetch(url, {
    cache: 'no-store',
    headers: {
      'Content-Type': 'application/json',
      ...(options.headers || {}),
    },
    ...options,
  });

  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(payload.error || `请求失败: ${response.status}`);
  }
  return payload;
}

async function loadConfig() {
  state.config = await fetchJson('/api/config');
  els.nodeName.textContent = state.config.node_name;
  els.modeName.textContent = state.config.mode;
  els.planNote.textContent = `快速模式：${state.config.test_plan.latency_probes} 次延迟探测，下载/上传各 ${state.config.test_plan.throughput_samples} 轮，每轮 ${state.config.test_plan.throughput_workers} 个并发 worker，单轮窗口约 ${state.config.test_plan.throughput_window_ms}ms。`;

  const roughSeconds = 1.2 + (state.config.test_plan.throughput_samples * state.config.test_plan.throughput_window_ms * 2 / 1000);
  els.estimatedTime.textContent = `${roughSeconds.toFixed(1)}s`;
  pushLog(`已连接节点 ${state.config.node_name}，测速模式 ${state.config.mode}。`);
}

async function startSession() {
  const session = await fetchJson('/api/session', { method: 'POST' });
  state.token = session.token;
  els.sessionState.textContent = '已创建';
  pushLog(`测速会话已创建，有效期 ${session.expires_in_seconds} 秒。`);
}

function authHeaders() {
  return state.token ? { 'X-Speedtest-Session': state.token } : {};
}

async function runLatencyPhase() {
  const probes = state.config.test_plan.latency_probes;
  const interval = state.config.test_plan.latency_interval_ms;
  const samples = [];

  for (let i = 0; i < probes; i += 1) {
    const started = performance.now();
    try {
      await fetch(`/api/ping?ts=${Date.now()}-${i}`, {
        cache: 'no-store',
        headers: authHeaders(),
      });
      const elapsed = performance.now() - started;
      samples.push(elapsed);
    } catch (error) {
      samples.push(Number.NaN);
      pushLog(`延迟探测失败：${error.message}`);
    }

    setProgress((10 + (i + 1) / probes * 20));
    if (i < probes - 1) {
      await sleep(interval);
    }
  }

  const filtered = samples.filter(Number.isFinite);
  const ping = median(filtered);
  const jitter = filtered.length > 1
    ? median(filtered.slice(1).map((value, index) => Math.abs(value - filtered[index])))
    : 0;
  const loss = ((probes - filtered.length) / probes) * 100;

  els.pingValue.textContent = formatMs(ping);
  els.jitterValue.textContent = formatMs(jitter);
  els.lossValue.textContent = formatLoss(loss);
  pushLog(`延迟完成：RTT ${ping.toFixed(2)} ms，抖动 ${jitter.toFixed(2)} ms，丢包 ${loss.toFixed(1)}%。`);
}

async function runThroughputPhase(kind) {
  const samples = state.config.test_plan.throughput_samples;
  const workers = state.config.test_plan.throughput_workers;
  const windowMs = state.config.test_plan.throughput_window_ms;
  const results = [];

  for (let sampleIndex = 0; sampleIndex < samples; sampleIndex += 1) {
    const started = performance.now();
    const bytes = await Promise.all(
      Array.from({ length: workers }, () => workerLoop(kind, windowMs))
    ).then((parts) => parts.reduce((sum, value) => sum + value, 0));

    const duration = Math.max((performance.now() - started) / 1000, windowMs / 1000);
    const mbps = (bytes * 8) / duration / (1024 * 1024);
    results.push(mbps);

    const phaseBase = kind === 'download' ? 30 : 62;
    setProgress(phaseBase + ((sampleIndex + 1) / samples) * 24);
    pushLog(`${kind === 'download' ? '下载' : '上传'}样本 ${sampleIndex + 1}/${samples}：${mbps.toFixed(2)} Mbps。`);
  }

  return trimmedMedian(results);
}

async function workerLoop(kind, windowMs) {
  const deadline = performance.now() + windowMs;
  let bytes = 0;
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), windowMs + 100);

  try {
    while (performance.now() < deadline) {
      if (kind === 'download') {
        const response = await fetch(`/api/download?size=${state.config.test_plan.download_request_size}&ts=${Date.now()}`, {
          cache: 'no-store',
          headers: authHeaders(),
          signal: controller.signal,
        });
        if (!response.ok) break;

        const reader = response.body.getReader();
        while (performance.now() < deadline) {
          const { done, value } = await reader.read();
          if (done) break;
          bytes += value.byteLength;
        }
      } else {
        const payload = new Uint8Array(state.config.test_plan.upload_payload_size);
        for (let i = 0; i < payload.length; i += 4096) {
          payload[i] = 97;
        }

        const response = await fetch('/api/upload', {
          method: 'POST',
          cache: 'no-store',
          headers: {
            ...authHeaders(),
            'Content-Type': 'application/octet-stream',
          },
          body: payload,
          signal: controller.signal,
        });
        if (!response.ok) break;
        const data = await response.json();
        bytes += data.received || payload.length;
      }

      if (performance.now() < deadline) {
        await sleep(20);
      }
    }
  } catch (error) {
    if (error.name !== 'AbortError') {
      pushLog(`${kind === 'download' ? '下载' : '上传'} worker 终止：${error.message}`);
    }
  } finally {
    clearTimeout(timeoutId);
  }

  return bytes;
}

async function runSpeedTest() {
  if (state.running) return;
  state.running = true;
  els.start.disabled = true;
  state.token = null;
  els.sessionState.textContent = '创建中';
  setStatus('正在创建测速会话', true);
  setProgress(5);

  try {
    await startSession();
    setStatus('正在测量延迟和丢包', true);
    await runLatencyPhase();

    setStatus('正在测量下载', true);
    const download = await runThroughputPhase('download');
    els.downloadValue.textContent = formatMbps(download);

    setStatus('正在测量上传', true);
    const upload = await runThroughputPhase('upload');
    els.uploadValue.textContent = formatMbps(upload);

    setStatus('测速完成', false);
    setProgress(100);
    pushLog('完整测速流程结束，结果仅保留在当前页面内存中。');
  } catch (error) {
    setStatus('测速失败', false);
    pushLog(`测速失败：${error.message}`);
  } finally {
    state.running = false;
    els.start.disabled = false;
  }
}

function resetResults() {
  state.token = null;
  state.logs = [];
  state.progress = 0;
  els.log.innerHTML = '';
  els.downloadValue.textContent = '-';
  els.uploadValue.textContent = '-';
  els.pingValue.textContent = '-';
  els.jitterValue.textContent = '-';
  els.lossValue.textContent = '-';
  els.sessionState.textContent = '未启动';
  setProgress(0);
  setStatus('等待开始', false);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

els.start.addEventListener('click', runSpeedTest);
els.reset.addEventListener('click', resetResults);

loadConfig().catch((error) => {
  pushLog(`读取配置失败：${error.message}`);
  setStatus('配置加载失败', false);
});
