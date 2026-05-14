(() => {
  const boot = window.PI_ARTIFACT_BOOTSTRAP;
  if (!boot?.artifactId || !boot.apiBase || !boot.token) {
    document.body.innerHTML = '<div class="fatal">Missing interactive artifact bootstrap data.</div>';
    return;
  }

  const state = {
    data: null,
    selectedNodeId: "hero",
    stream: null,
    submitting: false,
    streamStatus: "Connecting…",
  };

  const DOMPURIFY_SCRIPT_SRC = "https://cdn.jsdelivr.net/npm/dompurify@3.2.6/dist/purify.min.js";
  const MERMAID_SCRIPT_SRC =
    "https://cdn.jsdelivr.net/npm/mermaid@" + ["10", "9", "3"].join(".") + "/dist/mermaid.min.js";
  let domPurifyLoadPromise = null;
  let mermaidLoadPromise = null;

  const app = document.getElementById("app");
  if (!app) return;

  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function formatTime(ts) {
    if (!ts) return "—";
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(ts));
  }

  function titleForNode(nodeId) {
    const document = state.data?.document;
    if (!document) return nodeId;
    if (nodeId === "hero") return document.title || "Artifact header";
    if (nodeId === "summary") return "Summary";
    if (nodeId.startsWith("section:")) {
      const sectionId = nodeId.slice("section:".length);
      const section = (document.sections || []).find((item) => item.id === sectionId);
      return section ? section.title : nodeId;
    }
    if (nodeId.startsWith("glossary:")) {
      const glossaryId = nodeId.slice("glossary:".length);
      const term = (document.glossary || []).find((item) => item.id === glossaryId);
      return term ? term.term : nodeId;
    }
    if (nodeId === "next-actions") return "Next actions";
    return nodeId;
  }

  function renderPlainTextBlocks(text) {
    const safe = escapeHtml(text || "");
    const lines = safe
      .split(/\n+/)
      .map((line) => line.trim())
      .filter(Boolean);
    if (lines.length === 0) return "";
    const chunks = [];
    let listItems = [];

    const flushList = () => {
      if (listItems.length === 0) return;
      chunks.push(`<ul>${listItems.map((item) => `<li>${item}</li>`).join("")}</ul>`);
      listItems = [];
    };

    for (const line of lines) {
      if (line.startsWith("- ") || line.startsWith("* ")) {
        listItems.push(line.slice(2).trim());
        continue;
      }
      flushList();
      chunks.push(`<p>${line}</p>`);
    }
    flushList();
    return chunks.join("");
  }

  function splitMarkdownFences(text) {
    const source = String(text || "");
    const blocks = [];
    const fencePattern = /```([^\n`]*)\n([\s\S]*?)```/g;
    let lastIndex = 0;
    let match;

    while ((match = fencePattern.exec(source)) !== null) {
      if (match.index > lastIndex) {
        blocks.push({ type: "text", value: source.slice(lastIndex, match.index) });
      }
      const info = (match[1] || "").trim().toLowerCase();
      const lang = info.split(/\s+/)[0] || "";
      blocks.push({ type: "code", lang, value: (match[2] || "").trim() });
      lastIndex = fencePattern.lastIndex;
    }

    if (lastIndex < source.length) {
      blocks.push({ type: "text", value: source.slice(lastIndex) });
    }

    return blocks;
  }

  function renderCodeBlock(block) {
    if (block.lang === "mermaid") {
      return `
        <div class="mermaid-shell">
          <div class="mermaid" data-mermaid-source="true">${escapeHtml(block.value)}</div>
        </div>
      `;
    }

    const languageClass = block.lang ? ` language-${escapeHtml(block.lang)}` : "";
    const languageLabel = block.lang ? `<div class="code-label">${escapeHtml(block.lang)}</div>` : "";
    return `
      <div class="code-shell">
        ${languageLabel}
        <pre class="code-block"><code class="${languageClass}">${escapeHtml(block.value)}</code></pre>
      </div>
    `;
  }

  function renderTextBlocks(text) {
    const blocks = splitMarkdownFences(text);
    if (blocks.length === 0) return '<p class="empty-copy">No content yet.</p>';

    const html = blocks
      .map((block) => {
        if (block.type === "code") return renderCodeBlock(block);
        return renderPlainTextBlocks(block.value);
      })
      .filter(Boolean)
      .join("");

    return html || '<p class="empty-copy">No content yet.</p>';
  }

  function loadScriptOnce(src, globalCheck, label) {
    if (globalCheck()) return Promise.resolve();

    return new Promise((resolve, reject) => {
      const existing = Array.from(document.scripts).find((script) => script.src === src);
      if (existing) {
        existing.addEventListener("load", () => resolve(), { once: true });
        existing.addEventListener("error", () => reject(new Error(`Could not load ${label}.`)), { once: true });
        return;
      }

      const script = document.createElement("script");
      script.src = src;
      script.async = true;
      script.onload = () => resolve();
      script.onerror = () => reject(new Error(`Could not load ${label}.`));
      document.head.appendChild(script);
    });
  }

  function ensureDomPurifyLoaded() {
    const hasUsableDomPurify = () =>
      !!window.DOMPurify &&
      typeof window.DOMPurify.sanitize === "function" &&
      typeof window.DOMPurify.addHook === "function";

    if (hasUsableDomPurify()) return Promise.resolve(window.DOMPurify);
    if (domPurifyLoadPromise) return domPurifyLoadPromise;

    // Mermaid's browser bundle expects a full DOMPurify global. If another script
    // provides a partial/stub DOMPurify, Mermaid warns with
    // "DOMPurify.addHook is not a function". Load a pinned DOMPurify first.
    domPurifyLoadPromise = loadScriptOnce(DOMPURIFY_SCRIPT_SRC, hasUsableDomPurify, "DOMPurify").then(() => {
      if (!hasUsableDomPurify()) {
        throw new Error("DOMPurify loaded but sanitize/addHook are unavailable.");
      }
      return window.DOMPurify;
    });

    return domPurifyLoadPromise;
  }

  function ensureMermaidLoaded() {
    if (window.mermaid) return Promise.resolve(window.mermaid);
    if (mermaidLoadPromise) return mermaidLoadPromise;

    mermaidLoadPromise = ensureDomPurifyLoaded()
      .then(() => loadScriptOnce(MERMAID_SCRIPT_SRC, () => !!window.mermaid, "Mermaid renderer"))
      .then(() => {
        if (!window.mermaid) {
          throw new Error("Mermaid script loaded but window.mermaid is missing.");
        }
        window.mermaid.initialize({
          startOnLoad: false,
          theme: "dark",
          securityLevel: "strict",
          flowchart: { htmlLabels: false, curve: "basis" },
          sequence: { mirrorActors: false },
        });
        return window.mermaid;
      });

    return mermaidLoadPromise;
  }

  async function renderMermaidDiagrams() {
    const nodes = Array.from(app.querySelectorAll(".mermaid"));
    if (nodes.length === 0) return;

    try {
      const mermaid = await ensureMermaidLoaded();
      await mermaid.run({ nodes });
    } catch (error) {
      console.error("Mermaid render failed", error);
      for (const node of nodes) {
        if (node.dataset.renderFailed === "true") continue;
        node.dataset.renderFailed = "true";
        const source = node.textContent || "";
        node.innerHTML = `
          <div class="mermaid-error">Mermaid render failed. Showing source.</div>
          <pre class="code-block"><code>${escapeHtml(source)}</code></pre>
        `;
      }
    }
  }

  function setSelectedNode(nodeId) {
    state.selectedNodeId = nodeId || "hero";
    render();
    const textarea = document.getElementById("feedback-textarea");
    if (textarea) textarea.focus();
  }

  async function loadState() {
    const response = await fetch(`${boot.apiBase}/artifacts/${encodeURIComponent(boot.artifactId)}`, {
      cache: "no-store",
    });
    if (!response.ok) {
      throw new Error(`Server returned ${response.status}`);
    }
    state.data = await response.json();
    if (!state.selectedNodeId) state.selectedNodeId = "hero";
    render();
  }

  function connectStream() {
    if (state.stream) {
      state.stream.close();
    }

    state.streamStatus = "Live updates connected";
    const stream = new EventSource(
      `${boot.apiBase}/artifacts/${encodeURIComponent(boot.artifactId)}/stream?token=${encodeURIComponent(boot.token)}`,
    );
    state.stream = stream;

    stream.onmessage = (event) => {
      try {
        state.data = JSON.parse(event.data);
        state.streamStatus = "Live updates connected";
        render();
      } catch (error) {
        console.error("Failed to parse SSE payload", error);
      }
    };

    stream.onerror = () => {
      state.streamStatus = "Disconnected. Waiting for reconnection…";
      render();
    };
  }

  async function submitFeedback(event) {
    event.preventDefault();
    if (state.submitting) return;
    const textarea = document.getElementById("feedback-textarea");
    if (!textarea) return;
    const comment = textarea.value.trim();
    if (!comment) return;

    state.submitting = true;
    render();

    try {
      const response = await fetch(`${boot.apiBase}/artifacts/${encodeURIComponent(boot.artifactId)}/comments`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Pi-Artifact-Token": boot.token,
        },
        body: JSON.stringify({
          nodeId: state.selectedNodeId,
          comment,
        }),
      });

      if (!response.ok) {
        const payload = await response.json().catch(() => ({}));
        throw new Error(payload.error || `Server returned ${response.status}`);
      }

      textarea.value = "";
    } catch (error) {
      window.alert(error instanceof Error ? error.message : "Could not send comment to pi.");
    } finally {
      state.submitting = false;
      render();
    }
  }

  function onAppClick(event) {
    const pinTarget = event.target.closest("[data-pin-node]");
    if (pinTarget) {
      setSelectedNode(pinTarget.getAttribute("data-pin-node"));
      return;
    }

    const nodeTarget = event.target.closest("[data-node-id]");
    if (nodeTarget) {
      setSelectedNode(nodeTarget.getAttribute("data-node-id"));
    }
  }

  function renderHero(document, busy, lastEvent) {
    return `
      <section class="hero node ${state.selectedNodeId === "hero" ? "selected" : ""}" data-node-id="hero">
        <div class="hero-copy">
          <div class="hero-kicker">Pi interactive artifact</div>
          <h1>${escapeHtml(document.title || "Interactive artifact")}</h1>
          ${document.subtitle ? `<p class="subtitle">${escapeHtml(document.subtitle)}</p>` : ""}
          ${document.summary ? `<div class="summary-card node ${state.selectedNodeId === "summary" ? "selected" : ""}" data-node-id="summary">${renderTextBlocks(document.summary)}</div>` : ""}
        </div>
        <div class="hero-meta">
          <div class="meta-card">
            <div class="meta-label">Revision</div>
            <div class="meta-value">${document.revision}</div>
          </div>
          <div class="meta-card">
            <div class="meta-label">Updated</div>
            <div class="meta-value">${escapeHtml(formatTime(document.updatedAt))}</div>
          </div>
          <div class="meta-card ${busy ? "busy" : ""}">
            <div class="meta-label">Status</div>
            <div class="meta-value">${busy ? "Pi is updating…" : "Idle"}</div>
          </div>
          ${lastEvent ? `<p class="last-event">${escapeHtml(lastEvent)}</p>` : ""}
        </div>
      </section>
    `;
  }

  function renderSections(document) {
    const sections = document.sections || [];
    if (sections.length === 0) {
      return `
        <section class="empty-state">
          <h2>Waiting for the first revision…</h2>
          <p>Pi will publish the concept explainer here. Keep this tab open.</p>
        </section>
      `;
    }

    return sections
      .map((section) => {
        const nodeId = `section:${section.id}`;
        return `
          <article class="section-card node ${state.selectedNodeId === nodeId ? "selected" : ""}" data-node-id="${escapeHtml(nodeId)}">
            <div class="section-header">
              <div>
                <div class="section-label">Section</div>
                <h2>${escapeHtml(section.title)}</h2>
              </div>
              <button class="pin-btn" type="button" data-pin-node="${escapeHtml(nodeId)}">Pin feedback</button>
            </div>
            <div class="copy">${renderTextBlocks(section.body)}</div>
            ${section.bullets && section.bullets.length > 0 ? `<div class="detail-block"><h3>Key points</h3><ul>${section.bullets.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}</ul></div>` : ""}
            ${section.examples && section.examples.length > 0 ? `<div class="detail-block"><h3>Examples</h3><ul>${section.examples.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}</ul></div>` : ""}
            ${section.questions && section.questions.length > 0 ? `<div class="detail-block"><h3>Review questions</h3><ol>${section.questions.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}</ol></div>` : ""}
          </article>
        `;
      })
      .join("");
  }

  function renderGlossary(document) {
    const glossary = document.glossary || [];
    if (glossary.length === 0) return "";
    return `
      <section class="glossary-shell">
        <div class="block-header">
          <div>
            <div class="section-label">Reference</div>
            <h2>Glossary</h2>
          </div>
        </div>
        <div class="glossary-grid">
          ${glossary
            .map((term) => {
              const nodeId = `glossary:${term.id}`;
              return `
                <article class="glossary-card node ${state.selectedNodeId === nodeId ? "selected" : ""}" data-node-id="${escapeHtml(nodeId)}">
                  <div class="glossary-head">
                    <h3>${escapeHtml(term.term)}</h3>
                    <button class="pin-inline" type="button" data-pin-node="${escapeHtml(nodeId)}">Pin</button>
                  </div>
                  <p>${escapeHtml(term.definition)}</p>
                </article>
              `;
            })
            .join("")}
        </div>
      </section>
    `;
  }

  function renderNextActions(document) {
    const nextActions = document.nextActions || [];
    if (nextActions.length === 0) return "";
    const nodeId = "next-actions";
    return `
      <section class="next-actions node ${state.selectedNodeId === nodeId ? "selected" : ""}" data-node-id="${nodeId}">
        <div class="block-header">
          <div>
            <div class="section-label">Wrap-up</div>
            <h2>Next actions</h2>
          </div>
          <button class="pin-btn" type="button" data-pin-node="${nodeId}">Pin feedback</button>
        </div>
        <ul>${nextActions.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}</ul>
      </section>
    `;
  }

  function renderOutline(document) {
    const items = [
      { nodeId: "hero", label: "Overview" },
      ...(document.sections || []).map((section) => ({ nodeId: `section:${section.id}`, label: section.title })),
      ...(document.glossary || []).map((term) => ({ nodeId: `glossary:${term.id}`, label: term.term })),
      ...((document.nextActions || []).length > 0 ? [{ nodeId: "next-actions", label: "Next actions" }] : []),
    ];

    return `
      <nav class="outline-card">
        <div class="sidebar-label">Outline</div>
        <ul>
          ${items
            .map(
              (item) =>
                `<li><button type="button" class="outline-btn ${state.selectedNodeId === item.nodeId ? "active" : ""}" data-pin-node="${escapeHtml(item.nodeId)}">${escapeHtml(item.label)}</button></li>`,
            )
            .join("")}
        </ul>
      </nav>
    `;
  }

  function renderComments(comments) {
    if (!comments || comments.length === 0) {
      return '<p class="empty-comments">No pinned feedback yet.</p>';
    }

    return comments
      .slice()
      .reverse()
      .map((comment) => {
        const nodeTitle = titleForNode(comment.nodeId);
        return `
          <article class="comment-card ${comment.status}">
            <div class="comment-meta">
              <button type="button" class="comment-node" data-pin-node="${escapeHtml(comment.nodeId)}">${escapeHtml(nodeTitle)}</button>
              <span>${escapeHtml(formatTime(comment.createdAt))}</span>
            </div>
            <p>${escapeHtml(comment.comment)}</p>
            <div class="comment-status ${comment.status}">${comment.status === "queued" ? "Queued for Pi" : "Captured"}</div>
          </article>
        `;
      })
      .join("");
  }

  function render() {
    const artifact = state.data?.document;
    const comments = state.data?.comments || [];
    const busy = !!state.data?.busy;
    const lastEvent = state.data?.lastEvent || "";

    if (!artifact) {
      app.innerHTML = `
        <div class="shell loading">
          <div class="loading-card">
            <h1>Connecting to interactive artifact…</h1>
            <p>${escapeHtml(state.streamStatus)}</p>
          </div>
        </div>
      `;
      return;
    }

    app.innerHTML = `
      <div class="shell">
        <aside class="left-column">
          <div class="brand-card">
            <div class="sidebar-label">Artifact</div>
            <h2>${escapeHtml(artifact.title || "Interactive artifact")}</h2>
            <p>${escapeHtml(state.streamStatus)}</p>
          </div>
          ${renderOutline(artifact)}
        </aside>

        <main class="main-column">
          ${renderHero(artifact, busy, lastEvent)}
          <section class="content-stack">
            ${renderSections(artifact)}
            ${renderGlossary(artifact)}
            ${renderNextActions(artifact)}
          </section>
        </main>

        <aside class="right-column">
          <section class="feedback-card">
            <div class="sidebar-label">Pinned feedback</div>
            <h2>${escapeHtml(titleForNode(state.selectedNodeId))}</h2>
            <p class="feedback-help">Click any section, glossary card, or summary block, then leave feedback here. Pi receives it through the local artifact bridge.</p>
            <form id="feedback-form">
              <textarea id="feedback-textarea" placeholder="Example: clarify this section with a concrete example, simplify the wording, or add a comparison."></textarea>
              <button type="submit" class="submit-btn" ${state.submitting ? "disabled" : ""}>${state.submitting ? "Sending…" : "Send to Pi"}</button>
            </form>
          </section>

          <section class="comment-list-card">
            <div class="sidebar-label">Comment stream</div>
            <div class="comment-list">${renderComments(comments)}</div>
          </section>
        </aside>
      </div>
    `;

    const form = document.getElementById("feedback-form");
    if (form) {
      form.addEventListener("submit", submitFeedback);
    }

    void renderMermaidDiagrams();
  }

  app.addEventListener("click", onAppClick);
  loadState().catch((error) => {
    app.innerHTML = `
      <div class="shell loading">
        <div class="loading-card">
          <h1>Could not load the interactive artifact</h1>
          <p>${escapeHtml(error instanceof Error ? error.message : "Unknown error")}</p>
        </div>
      </div>
    `;
  });
  connectStream();
})();
