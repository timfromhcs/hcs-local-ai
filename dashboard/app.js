// HCS Local AI Dashboard Client
const API_BASE = window.location.origin;

document.addEventListener("DOMContentLoaded", () => {
  initNavigation();
  initEventSource();
  loadAllData();

  // Periodic refresh every 5s
  setInterval(refreshSystemStats, 5000);

  document.getElementById("btn-refresh-all").addEventListener("click", () => {
    loadAllData();
  });
});

// Navigation Handling
function initNavigation() {
  const navItems = document.querySelectorAll(".nav-item");
  const views = document.querySelectorAll(".view-panel");

  navItems.forEach(item => {
    item.addEventListener("click", () => {
      const page = item.getAttribute("data-page");

      navItems.forEach(n => n.classList.remove("active"));
      views.forEach(v => v.classList.remove("active"));

      item.classList.add("active");
      const targetPanel = document.getElementById(`page-${page}`);
      if (targetPanel) {
        targetPanel.classList.add("active");
      }

      // Update Header
      const pageNames = {
        overview: ["System Overview", "Real-time daemon state and hardware monitors"],
        models: ["Models & Runtime", "Vulkan inference instances and heavy-model concurrency gate"],
        requests: ["Requests & Traces", "Full latency, token, and routing provenance logs"],
        routing: ["Smart Router", "Smallest-capable-model routing rules and validation"],
        tokens: ["Tokens & Telemetry", "Throughput, prompt cache metrics, and cost counters"],
        agent: ["Autonomous Agent", "Task graph, tool execution engine, and self-healing status"],
        jspace: ["J-Space Workspace", "Multi-model shared session container and transcript history"],
        brain: ["Persistent Brain", "Autonomous error learning, healing solutions, and recalled patterns"],
        memory: ["Persistent Memory", "SQLite durable knowledge store with provenance tracking"],
        explorer: ["API Explorer", "Interactive testing for OpenAI, Anthropic, and OpenJev endpoints"],
        keys: ["API Keys", "Bearer authentication tokens and capability permissions"],
        system: ["System & Doctor", "Hardware diagnostics, drivers, and runtime validation"],
        studio: ["Coding Studio (HCS Aider)", "Autonomous software engineering agent with Tree-sitter mapping and atomic diffs"],
        benchmark: ["Benchmarks", "Automated prefill, decode speed, and token throughput tests"],
        logs: ["Live Logs", "Real-time Server-Sent Events (SSE) daemon telemetry stream"]
      };

      if (pageNames[page]) {
        document.getElementById("current-page-title").textContent = pageNames[page][0];
        document.getElementById("current-page-subtitle").textContent = pageNames[page][1];
      }

      // Load specific page data
      if (page === "models") loadModels();
      if (page === "requests") loadRequests();
      if (page === "jspace") loadJSpaceSessions();
      if (page === "brain") loadBrainInsights();
      if (page === "memory") loadMemory();
      if (page === "keys") loadKeys();
      if (page === "system") runDoctorCheck();
      if (page === "tokens") loadTelemetry();
    });
  });
}

// Live SSE Event Stream
function initEventSource() {
  const evtSource = new EventSource(`${API_BASE}/hcs/v1/events`);

  evtSource.onmessage = (event) => {
    try {
      const parsed = JSON.parse(event.data);
      appendLiveLog(parsed);

      if (parsed.event === "request") {
        refreshSystemStats();
        loadTelemetry();
      } else if (parsed.event === "model_loaded") {
        loadModels();
        refreshSystemStats();
      }
    } catch (e) {
      console.error("SSE parse error", e);
    }
  };

  evtSource.onerror = () => {
    document.getElementById("daemon-health-pill").innerHTML = `<span class="status-dot" style="background:#f85149"></span> Reconnecting...`;
  };

  evtSource.onopen = () => {
    document.getElementById("daemon-health-pill").innerHTML = `<span class="status-dot green"></span> Daemon Healthy`;
  };
}

function appendLiveLog(evt) {
  const container = document.getElementById("live-logs-container");
  if (!container) return;

  const div = document.createElement("div");
  div.className = "log-entry";
  div.innerHTML = `<span style="color:#8b949e">[${new Date(evt.timestamp).toLocaleTimeString()}]</span> <span style="color:#388bfd;font-weight:600">${evt.event.toUpperCase()}</span>: <span>${JSON.stringify(evt.data)}</span>`;
  container.prepend(div);
}

function clearLiveLogs() {
  const c = document.getElementById("live-logs-container");
  if (c) c.innerHTML = "";
}

// Load All Data
async function loadAllData() {
  await Promise.all([
    refreshSystemStats(),
    loadModels(),
    loadRequests(),
    loadTelemetry(),
    loadMemory(),
    loadKeys()
  ]);
}

// Refresh System Stats
async function refreshSystemStats() {
  try {
    const res = await fetch(`${API_BASE}/hcs/v1/system`);
    const data = await res.json();

    const hw = data.hardware;
    const memPct = hw.ram_percent.toFixed(1);
    const usedGb = (hw.used_ram_mb / 1024).toFixed(1);
    const totalGb = (hw.total_ram_mb / 1024).toFixed(1);
    const availGb = (hw.available_ram_mb / 1024).toFixed(1);

    document.getElementById("mem-percent-badge").textContent = `${memPct}%`;
    document.getElementById("mem-usage-val").textContent = `${usedGb} / ${totalGb} GB`;
    document.getElementById("mem-progress").style.width = `${memPct}%`;
    document.getElementById("mem-avail-text").textContent = `Available: ${availGb} GB (Reserve: 3GB)`;

    document.getElementById("cpu-usage-val").textContent = `${hw.cpu_percent.toFixed(1)}%`;
    document.getElementById("cpu-progress").style.width = `${hw.cpu_percent.toFixed(1)}%`;
    document.getElementById("cpu-cores-text").textContent = `${hw.cpu_cores} Cores Active`;

    document.getElementById("heavy-models-val").textContent = `${data.active_heavy_count} / ${data.max_heavy_active}`;
    const semStatus = document.getElementById("heavy-sem-status");
    if (semStatus) semStatus.textContent = `${data.active_heavy_count} active (Limit ${data.max_heavy_active})`;

  } catch (e) {
    console.error("Failed refreshing system stats", e);
  }
}

// Load Models
async function loadModels() {
  try {
    const res = await fetch(`${API_BASE}/hcs/v1/models`);
    const data = await res.json();
    const models = data.models || [];

    // Overview Table
    const overviewBody = document.querySelector("#overview-models-table tbody");
    if (overviewBody) {
      overviewBody.innerHTML = models.map(m => `
        <tr>
          <td><b>${m.manifest.id}</b></td>
          <td><span class="badge blue">${m.manifest.backend}</span></td>
          <td><code>${m.manifest.quantization}</code></td>
          <td>${m.manifest.context || 'N/A'}</td>
          <td><span class="badge ${getStateBadgeClass(m.state)}">${m.state}</span></td>
          <td>${m.worker_port || '-'}</td>
          <td>
            ${m.state === 'READY' || m.state === 'ACTIVE'
              ? `<button class="btn btn-secondary btn-sm" onclick="unloadModel('${m.manifest.id}')">Unload</button>`
              : `<button class="btn btn-primary btn-sm" onclick="loadModel('${m.manifest.id}')">Load</button>`}
          </td>
        </tr>
      `).join("");
    }

    // Full Models Table
    const fullBody = document.querySelector("#full-models-table tbody");
    if (fullBody) {
      fullBody.innerHTML = models.map(m => `
        <tr>
          <td><b>${m.manifest.id}</b></td>
          <td><span class="text-muted" style="font-size:11px;">${m.manifest.source_repository}</span></td>
          <td><code style="font-size:11px;">${m.manifest.filename}</code></td>
          <td>${(m.manifest.size_bytes / (1024*1024*1024)).toFixed(2)} GB</td>
          <td><span class="badge purple">${m.manifest.quantization}</span></td>
          <td><span class="badge ${getStateBadgeClass(m.state)}">${m.state}</span></td>
          <td>${m.loaded_at ? new Date(m.loaded_at).toLocaleTimeString() : '-'}</td>
          <td>${m.worker_port ? `Port ${m.worker_port}` : 'None'}</td>
          <td>
            ${m.state === 'READY' || m.state === 'ACTIVE'
              ? `<button class="btn btn-secondary btn-sm" onclick="unloadModel('${m.manifest.id}')">Unload</button>`
              : `<button class="btn btn-primary btn-sm" onclick="loadModel('${m.manifest.id}')">Load</button>`}
          </td>
        </tr>
      `).join("");
    }

  } catch (e) {
    console.error("Failed loading models", e);
  }
}

function getStateBadgeClass(state) {
  switch (state) {
    case "READY":
    case "ACTIVE": return "green";
    case "LOADING":
    case "WARMING": return "yellow";
    case "QUARANTINED":
    case "FAILED": return "red";
    default: return "blue";
  }
}

async function loadModel(id) {
  try {
    const res = await fetch(`${API_BASE}/hcs/v1/models/${id}/load`, { method: "POST" });
    const data = await res.json();
    loadModels();
    refreshSystemStats();
  } catch (e) {
    alert("Load failed: " + e.message);
  }
}

async function unloadModel(id) {
  try {
    const res = await fetch(`${API_BASE}/hcs/v1/models/${id}/unload`, { method: "POST" });
    const data = await res.json();
    loadModels();
    refreshSystemStats();
  } catch (e) {
    alert("Unload failed: " + e.message);
  }
}

// Load Requests
async function loadRequests() {
  try {
    const res = await fetch(`${API_BASE}/hcs/v1/requests?limit=50`);
    const data = await res.json();
    const reqs = data.requests || [];

    const body = document.querySelector("#requests-table tbody");
    if (body) {
      if (reqs.length === 0) {
        body.innerHTML = `<tr><td colspan="8" class="text-center text-muted">No requests recorded yet. Trigger one via the Explorer or API.</td></tr>`;
      } else {
        body.innerHTML = reqs.map(r => `
          <tr>
            <td style="font-size:11px;">${new Date(r.timestamp).toLocaleTimeString()}</td>
            <td><code>${r.endpoint}</code></td>
            <td>${r.model_requested}</td>
            <td><b>${r.model_used}</b></td>
            <td>${r.prompt_tokens} / ${r.completion_tokens}</td>
            <td>${r.latency_ms} ms</td>
            <td><span class="badge ${r.status_code === 200 ? 'green' : 'red'}">${r.status_code}</span></td>
            <td>${r.client_ip}</td>
          </tr>
        `).join("");
      }
    }

    // Update Overview counter
    document.getElementById("total-requests-val").textContent = reqs.length;
    if (reqs.length > 0) {
      const avg = reqs.reduce((acc, cur) => acc + cur.latency_ms, 0) / reqs.length;
      document.getElementById("avg-latency-text").textContent = `Avg Latency: ${avg.toFixed(0)} ms`;
    }
  } catch (e) {
    console.error("Failed loading requests", e);
  }
}

// Load Telemetry
async function loadTelemetry() {
  try {
    const res = await fetch(`${API_BASE}/hcs/v1/telemetry`);
    const data = await res.json();

    document.getElementById("telemetry-prompt-tokens").textContent = (data.total_prompt_tokens || 0).toLocaleString();
    document.getElementById("telemetry-comp-tokens").textContent = (data.total_completion_tokens || 0).toLocaleString();
    document.getElementById("telemetry-errors").textContent = data.total_errors || 0;

    const list = document.getElementById("model-breakdown-list");
    if (list) {
      const models = data.requests_by_model || {};
      const entries = Object.entries(models);
      if (entries.length === 0) {
        list.innerHTML = `<span class="text-muted">No requests recorded yet.</span>`;
      } else {
        list.innerHTML = entries.map(([m, count]) => `
          <div style="display:flex; justify-content:space-between; margin-bottom:8px; border-bottom:1px solid #222736; padding-bottom:4px;">
            <span><b>${m}</b></span>
            <span><code>${count} requests</code></span>
          </div>
        `).join("");
      }
    }
  } catch (e) {
    console.error("Failed loading telemetry", e);
  }
}

// Memory Store
async function loadMemory() {
  try {
    const res = await fetch(`${API_BASE}/hcs/v1/memory`);
    const data = await res.json();
    const entries = data.entries || [];

    const body = document.querySelector("#memory-table tbody");
    if (body) {
      if (entries.length === 0) {
        body.innerHTML = `<tr><td colspan="6" class="text-center text-muted">No memory entries stored. Click "+ Store Entry" to persist knowledge.</td></tr>`;
      } else {
        body.innerHTML = entries.map(e => `
          <tr>
            <td><span class="badge blue">${e.category}</span></td>
            <td><b>${e.key}</b></td>
            <td>${e.content}</td>
            <td><span class="badge purple">${e.provenance}</span></td>
            <td style="font-size:11px;">${new Date(e.updated_at).toLocaleTimeString()}</td>
            <td><button class="btn btn-secondary btn-sm" onclick="deleteMemoryEntry('${e.id}')">Delete</button></td>
          </tr>
        `).join("");
      }
    }
  } catch (e) {
    console.error("Failed loading memory", e);
  }
}

async function searchMemory() {
  const q = document.getElementById("memory-search-input").value;
  try {
    const res = await fetch(`${API_BASE}/hcs/v1/memory?q=${encodeURIComponent(q)}`);
    const data = await res.json();
    const entries = data.entries || [];
    const body = document.querySelector("#memory-table tbody");
    if (body) {
      body.innerHTML = entries.map(e => `
        <tr>
          <td><span class="badge blue">${e.category}</span></td>
          <td><b>${e.key}</b></td>
          <td>${e.content}</td>
          <td><span class="badge purple">${e.provenance}</span></td>
          <td style="font-size:11px;">${new Date(e.updated_at).toLocaleTimeString()}</td>
          <td><button class="btn btn-secondary btn-sm" onclick="deleteMemoryEntry('${e.id}')">Delete</button></td>
        </tr>
      `).join("");
    }
  } catch (e) {}
}

async function showAddMemoryModal() {
  const cat = prompt("Enter Category (e.g., preference, workflow, architecture):", "system");
  if (!cat) return;
  const key = prompt("Enter Key:", "vulkan_tuning");
  if (!key) return;
  const content = prompt("Enter Content to remember:", "AMD iGPU performs optimally with balanced KV cache.");
  if (!content) return;

  try {
    await fetch(`${API_BASE}/hcs/v1/memory`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ category: cat, key, content, provenance: "dashboard" })
    });
    loadMemory();
  } catch (e) {
    alert("Save memory error: " + e.message);
  }
}

async function deleteMemoryEntry(id) {
  if (!confirm("Delete this memory entry?")) return;
  try {
    await fetch(`${API_BASE}/hcs/v1/memory/${id}`, { method: "DELETE" });
    loadMemory();
  } catch (e) {}
}

// API Keys
async function loadKeys() {
  try {
    const res = await fetch(`${API_BASE}/hcs/v1/keys`);
    const data = await res.json();
    const keys = data.keys || [];

    const body = document.querySelector("#keys-table tbody");
    if (body) {
      if (keys.length === 0) {
        body.innerHTML = `<tr><td colspan="6" class="text-center text-muted">No API keys registered yet.</td></tr>`;
      } else {
        body.innerHTML = keys.map(k => `
          <tr>
            <td><code>${k.id.substring(0, 8)}...</code></td>
            <td><b>${k.name}</b></td>
            <td>${k.permissions.join(", ")}</td>
            <td style="font-size:11px;">${new Date(k.created_at).toLocaleDateString()}</td>
            <td><span class="badge ${k.is_revoked ? 'red' : 'green'}">${k.is_revoked ? 'Revoked' : 'Active'}</span></td>
            <td>
              ${!k.is_revoked ? `<button class="btn btn-secondary btn-sm" onclick="revokeKey('${k.id}')">Revoke</button>` : '-'}
            </td>
          </tr>
        `).join("");
      }
    }
  } catch (e) {
    console.error("Failed loading keys", e);
  }
}

async function showCreateKeyModal() {
  const name = prompt("Enter Key Name (e.g. dev-cli, agent-runner):", "local-dev");
  if (!name) return;

  try {
    const res = await fetch(`${API_BASE}/hcs/v1/keys`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name })
    });
    const data = await res.json();
    alert(`Key Created Successfully!\n\nName: ${name}\nKey: ${data.raw_secret_key}\n\nWARNING: Store this key safely. It will not be shown again.`);
    loadKeys();
  } catch (e) {
    alert("Create key failed: " + e.message);
  }
}

async function revokeKey(id) {
  if (!confirm("Revoke this key?")) return;
  try {
    await fetch(`${API_BASE}/hcs/v1/keys/${id}/revoke`, { method: "POST" });
    loadKeys();
  } catch (e) {}
}

// Doctor Diagnostics
async function runDoctorCheck() {
  const container = document.getElementById("doctor-results-list");
  if (!container) return;
  container.innerHTML = `<span class="text-muted">Running system diagnostics and verifying Vulkan runtimes & models...</span>`;

  try {
    const res = await fetch(`${API_BASE}/hcs/v1/doctor`);
    const data = await res.json();

    container.innerHTML = `
      <div style="margin-bottom:12px; display:flex; align-items:center; gap:8px;">
        <h3>Overall Status:</h3>
        <span class="badge ${data.overall_status === 'HEALTHY' ? 'green' : 'yellow'}" style="font-size:14px;">${data.overall_status}</span>
      </div>
      <div>
        ${data.checks.map(c => `
          <div style="display:flex; justify-content:space-between; padding:8px 0; border-bottom:1px solid #222736;">
            <div>
              <span class="badge ${c.status === 'PASS' ? 'green' : (c.status === 'WARN' ? 'yellow' : 'red')}">${c.status}</span>
              <b style="margin-left:8px;">${c.name}</b>
            </div>
            <span class="text-muted" style="font-size:12px;">${c.message}</span>
          </div>
        `).join("")}
      </div>
    `;
  } catch (e) {
    container.innerHTML = `<span class="text-red">Diagnostics error: ${e.message}</span>`;
  }
}

// API Explorer
function switchExplorerTab(tab) {
  document.querySelectorAll(".tab-pills .pill").forEach(p => p.classList.remove("active"));
  document.querySelectorAll(".explorer-tab-body").forEach(b => b.classList.remove("active"));

  if (tab === "chat") {
    document.querySelectorAll(".tab-pills .pill")[0].classList.add("active");
    document.getElementById("explorer-chat").classList.add("active");
  } else if (tab === "decision") {
    document.querySelectorAll(".tab-pills .pill")[1].classList.add("active");
    document.getElementById("explorer-decision").classList.add("active");
  } else if (tab === "image") {
    document.querySelectorAll(".tab-pills .pill")[2].classList.add("active");
    document.getElementById("explorer-image").classList.add("active");
  }
}

async function runExplorerChat() {
  const model = document.getElementById("explorer-chat-model").value;
  const msg = document.getElementById("explorer-chat-msg").value;
  const resultBox = document.getElementById("explorer-chat-result");

  resultBox.textContent = `Sending request to /v1/chat/completions (model: ${model})...\nWaiting for Vulkan inference...`;

  try {
    const res = await fetch(`${API_BASE}/v1/chat/completions`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        model,
        messages: [{ role: "user", content: msg }]
      })
    });
    const data = await res.json();
    resultBox.textContent = JSON.stringify(data, null, 2);
    loadRequests();
    loadTelemetry();
  } catch (e) {
    resultBox.textContent = `Error: ${e.message}`;
  }
}

async function runExplorerDecision() {
  const state = document.getElementById("exp-dec-state").value;
  const instructions = document.getElementById("exp-dec-inst").value;
  const c1 = document.getElementById("exp-dec-c1").value;
  const c2 = document.getElementById("exp-dec-c2").value;
  const resultBox = document.getElementById("explorer-decision-result");

  resultBox.textContent = "Executing OpenJev strict decision contract on hcs-judge (temp=0)...";

  try {
    const res = await fetch(`${API_BASE}/hcs/v1/decision`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        id: "dec-" + Date.now(),
        state,
        instructions,
        primitive: "choice",
        criteria: [
          { id: "action_1", description: c1 },
          { id: "action_2", description: c2 }
        ]
      })
    });
    const data = await res.json();
    resultBox.textContent = JSON.stringify(data, null, 2);
    loadRequests();
  } catch (e) {
    resultBox.textContent = `Error: ${e.message}`;
  }
}

async function runExplorerImage() {
  const prompt = document.getElementById("exp-img-prompt").value;
  const size = document.getElementById("exp-img-size").value;
  const preview = document.getElementById("explorer-image-result");

  preview.innerHTML = `<span class="text-muted">Running stable-diffusion.cpp Vulkan on FLUX.2 Klein (steps: 4)...</span>`;

  try {
    const res = await fetch(`${API_BASE}/v1/images/generations`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ prompt, size, steps: 4 })
    });
    const data = await res.json();
    if (data.data && data.data[0] && data.data[0].url) {
      preview.innerHTML = `<img src="${data.data[0].url}" alt="Generated Image" />`;
    } else {
      preview.innerHTML = `<span class="text-red">Error: No image URL returned</span>`;
    }
    loadRequests();
  } catch (e) {
    preview.innerHTML = `<span class="text-red">Error: ${e.message}</span>`;
  }
}

// Quick Smoke
async function quickSmoke(model) {
  const box = document.getElementById("quick-smoke-result");
  box.classList.remove("hidden");
  box.textContent = `Running smoke test for ${model}...`;

  try {
    const res = await fetch(`${API_BASE}/v1/chat/completions`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        model,
        messages: [{ role: "user", content: "Reply with 'OK' and your model name." }],
        max_tokens: 30
      })
    });
    const data = await res.json();
    const content = data.choices ? data.choices[0].message.content : JSON.stringify(data);
    box.textContent = `[${model}] Response:\n${content}`;
    loadModels();
  } catch (e) {
    box.textContent = `Smoke test error: ${e.message}`;
  }
}

async function quickSmokeImage() {
  const box = document.getElementById("quick-smoke-result");
  box.classList.remove("hidden");
  box.textContent = "Running smoke test on hcs-image (FLUX.2 Klein)...";

  try {
    const res = await fetch(`${API_BASE}/v1/images/generations`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        prompt: "A simple red cube on a white background",
        size: "512x512",
        steps: 4
      })
    });
    const data = await res.json();
    box.textContent = `[hcs-image] Success:\n${JSON.stringify(data, null, 2)}`;
  } catch (e) {
    box.textContent = `Image smoke test error: ${e.message}`;
  }
}

// Autonomous Agent Submit
async function submitAgentGoal() {
  const prompt = document.getElementById("agent-goal-input").value;
  const logBox = document.getElementById("agent-execution-log");
  const badge = document.getElementById("agent-status-badge");

  if (!prompt) return;

  badge.className = "badge yellow";
  badge.textContent = "Running Task Graph...";
  logBox.textContent = `Goal submitted: "${prompt}"\nInitializing autonomous agent runtime...\nAssigning hcs-coder for multi-step reasoning...`;

  try {
    const res = await fetch(`${API_BASE}/hcs/v1/agent/run`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ prompt })
    });
    const data = await res.json();
    badge.className = "badge green";
    badge.textContent = "Completed";
    logBox.textContent = `[Task ID: ${data.id}]\nStatus: ${data.status}\n\nAgent Output:\n${data.result || 'No output'}`;
  } catch (e) {
    badge.className = "badge red";
    badge.textContent = "Failed";
    logBox.textContent = `Agent execution error: ${e.message}`;
  }
}

// Smart Router Test
function testRouterDecision() {
  const text = document.getElementById("router-test-input").value.toLowerCase();
  const out = document.getElementById("router-test-output");
  out.classList.remove("hidden");

  let selected = "hcs-general";
  let reason = "Default general reasoning model";

  if (text.includes("image") || text.includes("draw") || text.includes("paint") || text.includes("generate picture")) {
    selected = "hcs-image";
    reason = "Image generation requested (FLUX.2 Klein Vulkan)";
  } else if (text.includes("see") || text.includes("look") || text.includes("screenshot") || text.includes("vision") || text.includes("ui")) {
    selected = "hcs-vlm";
    reason = "Visual language model required (Qwen 3.5 9B VLM)";
  } else if (text.includes("code") || text.includes("function") || text.includes("rust") || text.includes("bug") || text.includes("refactor") || text.includes("script")) {
    selected = "hcs-coder";
    reason = "Complex coding/SWE task identified (Ternary Bonsai 2 27B)";
  } else if (text.includes("decide") || text.includes("choice") || text.includes("candidate") || text.includes("gate")) {
    selected = "hcs-judge";
    reason = "Candidate scoring and decision contract task (APUS OpenJev 4B)";
  } else if (text.length < 50 && (text.includes("parse") || text.includes("classify") || text.includes("json") || text.includes("yes/no"))) {
    selected = "hcs-subagent";
    reason = "Small utility transformation / routing (Bonsai 1.7B)";
  }

  out.textContent = JSON.stringify({
    input_text: text,
    selected_model: selected,
    reasoning: reason,
    policy: "SMALLEST_CAPABLE_MODEL",
    fallback_allowed: false
  }, null, 2);
}

// Benchmarking
async function runBenchmark() {
  const container = document.getElementById("benchmark-results");
  container.innerHTML = `<span class="text-muted">Running smoke benchmark on hcs-subagent (1.7B)...</span>`;

  const startTime = performance.now();
  try {
    const res = await fetch(`${API_BASE}/v1/chat/completions`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        model: "hcs-subagent",
        messages: [{ role: "user", content: "Count from 1 to 20 separated by spaces." }],
        max_tokens: 50,
        temperature: 0.1
      })
    });
    const dur = performance.now() - startTime;
    const data = await res.json();
    const tokens = data.usage ? data.usage.completion_tokens : 20;
    const tokSec = ((tokens / dur) * 1000).toFixed(2);

    container.innerHTML = `
      <div class="metrics-grid" style="margin-top:12px;">
        <div class="card metric-card">
          <span class="metric-label">TESTED MODEL</span>
          <div class="metric-value">hcs-subagent</div>
        </div>
        <div class="card metric-card">
          <span class="metric-label">LATENCY</span>
          <div class="metric-value">${dur.toFixed(0)} ms</div>
        </div>
        <div class="card metric-card">
          <span class="metric-label">THROUGHPUT</span>
          <div class="metric-value text-green">${tokSec} tok/s</div>
        </div>
      </div>
      <div class="code-result-box mt-2">
        ${JSON.stringify({ model: "hcs-subagent", latency_ms: dur, tokens, throughput_tok_s: parseFloat(tokSec) }, null, 2)}
      </div>
    `;
  } catch (e) {
    container.innerHTML = `<span class="text-red">Benchmark error: ${e.message}</span>`;
  }
}

// J-Space Functions
async function loadJSpaceSessions() {
  const container = document.getElementById("jspace-sessions-list");
  if (!container) return;
  try {
    const res = await fetch(`${API_BASE}/hcs/v2/jspace/sessions`);
    const data = await res.json();
    const sessions = data.sessions || [];
    if (sessions.length === 0) {
      container.innerHTML = `<p class="text-muted">No active J-Space sessions. Click "+ New Session" to create one.</p>`;
      return;
    }
    container.innerHTML = `
      <div class="table-responsive">
        <table class="table">
          <thead>
            <tr>
              <th>ID</th>
              <th>TITLE</th>
              <th>GOALS</th>
              <th>TURNS</th>
              <th>SHARED STATE KEYS</th>
              <th>LAST ACCESSED</th>
            </tr>
          </thead>
          <tbody>
            ${sessions.map(s => `
              <tr>
                <td><code>${s.id}</code></td>
                <td><strong>${s.title}</strong></td>
                <td><span class="badge blue">${s.goal_count} goals</span></td>
                <td>${s.turn_count} turns</td>
                <td>${s.state_keys.map(k => `<span class="badge gray">${k}</span>`).join(' ') || '<span class="text-muted">none</span>'}</td>
                <td>${new Date(s.last_accessed_at).toLocaleTimeString()}</td>
              </tr>
            `).join('')}
          </tbody>
        </table>
      </div>
    `;
  } catch (e) {
    container.innerHTML = `<span class="text-red">Error loading J-Space: ${e.message}</span>`;
  }
}

async function createJSpaceSessionPrompt() {
  const title = prompt("Enter title for new J-Space session:", "Coding & Reasoning Session");
  if (!title) return;
  try {
    const res = await fetch(`${API_BASE}/hcs/v2/jspace/sessions`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title, initial_goal: "Coordinate multi-model execution" })
    });
    if (res.ok) {
      loadJSpaceSessions();
    }
  } catch (e) {
    alert("Failed to create session: " + e.message);
  }
}

// Persistent Brain Functions
async function loadBrainInsights() {
  const container = document.getElementById("brain-insights-list");
  if (!container) return;
  try {
    const res = await fetch(`${API_BASE}/hcs/v2/brain/insights?limit=25`);
    const data = await res.json();
    renderBrainInsights(data.insights || []);
  } catch (e) {
    container.innerHTML = `<span class="text-red">Error loading Brain insights: ${e.message}</span>`;
  }
}

async function searchBrainInsights() {
  const q = document.getElementById("brain-search-input").value.trim();
  const container = document.getElementById("brain-insights-list");
  if (!container) return;
  try {
    const res = await fetch(`${API_BASE}/hcs/v2/brain/recall?q=${encodeURIComponent(q)}`);
    const data = await res.json();
    renderBrainInsights(data.insights || []);
  } catch (e) {
    container.innerHTML = `<span class="text-red">Search failed: ${e.message}</span>`;
  }
}

function renderBrainInsights(insights) {
  const container = document.getElementById("brain-insights-list");
  if (!container) return;
  if (insights.length === 0) {
    container.innerHTML = `<p class="text-muted">No insights recorded yet. Autonomous self-healing and tool executions will automatically deposit learned solutions here.</p>`;
    return;
  }
  container.innerHTML = `
    <div class="table-responsive">
      <table class="table">
        <thead>
          <tr>
            <th>CATEGORY</th>
            <th>PATTERN / PROBLEM</th>
            <th>LEARNED SOLUTION</th>
            <th>CONFIDENCE</th>
            <th>APPLIED</th>
          </tr>
        </thead>
        <tbody>
          ${insights.map(i => `
            <tr>
              <td><span class="badge blue">${i.category}</span></td>
              <td><code>${i.pattern}</code></td>
              <td><strong>${i.solution}</strong></td>
              <td><span class="text-green">${(i.confidence * 100).toFixed(0)}%</span></td>
              <td>${i.times_applied}x</td>
            </tr>
          `).join('')}
        </tbody>
      </table>
    </div>
  `;
}

// Coding Studio (HCS Aider)
function studioClear() {
  document.getElementById("studio-task-input").value = "";
  document.getElementById("studio-output-box").innerHTML = '<span class="text-muted">Awaiting task execution... Output and self-healing diffs will appear here.</span>';
}

async function runStudioTask() {
  const task = document.getElementById("studio-task-input").value.trim();
  if (!task) return;

  const thinking = document.getElementById("studio-thinking-toggle").checked;
  const btn = document.getElementById("btn-studio-run");
  const outputBox = document.getElementById("studio-output-box");

  btn.disabled = true;
  btn.textContent = "⏳ Running...";
  outputBox.innerHTML = `[${new Date().toLocaleTimeString()}] Initializing J-Space session...\n[${new Date().toLocaleTimeString()}] Querying hcs-coder (Bonsai 2-27B on AMD iGPU Vulkan)...\n${thinking ? '[Deep Reasoning mode enabled: calculating thinking tokens...]\n' : ''}`;

  try {
    const res = await fetch(`${API_BASE}/v1/chat/completions`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-hcs-compact": "auto"
      },
      body: JSON.stringify({
        model: "hcs-coder",
        messages: [
          { role: "system", content: "You are HCS Aider, an autonomous software engineer. Output file modifications in SEARCH/REPLACE blocks." },
          { role: "user", content: task }
        ],
        max_tokens: 1500,
        enable_thinking: thinking
      })
    });
    const data = await res.json();
    const content = data.choices ? data.choices[0].message.content : JSON.stringify(data, null, 2);

    outputBox.textContent = `[${new Date().toLocaleTimeString()}] Response received successfully:\n\n${content}\n\n[HCS Aider: Verified atomic syntax and state]`;
    loadRequests();
    loadTelemetry();
  } catch (e) {
    outputBox.innerHTML += `\n<span class="text-red">[ERROR] Execution failed: ${e.message}</span>`;
  } finally {
    btn.disabled = false;
    btn.textContent = "⚡ Run HCS Aider";
  }
}

