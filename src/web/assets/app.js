(function () {
  "use strict";

  const params = new URLSearchParams(window.location.search);
  const TOKEN = params.get("token") || "";

  const tokenState = document.getElementById("tokenState");
  if (TOKEN) {
    tokenState.textContent = "token loaded";
  } else {
    tokenState.textContent = "no token in URL — every action will 401";
    tokenState.classList.add("is-bad");
  }

  async function api(path, options) {
    options = options || {};
    const headers = Object.assign(
      { "Content-Type": "application/json", "X-Horcrux-Token": TOKEN },
      options.headers || {}
    );
    const res = await fetch("/api" + path, {
      method: options.method || "GET",
      headers,
      body: options.body ? JSON.stringify(options.body) : undefined,
    });
    let data = null;
    try {
      data = await res.json();
    } catch (e) {
      data = null;
    }
    if (!res.ok) {
      const err = new Error((data && data.error) || res.statusText);
      err.status = res.status;
      err.data = data;
      throw err;
    }
    return data;
  }

  function el(tag, attrs, children) {
    const node = document.createElement(tag);
    Object.entries(attrs || {}).forEach(([k, v]) => {
      if (k === "class") node.className = v;
      else if (k === "html") node.innerHTML = v;
      else node.setAttribute(k, v);
    });
    (children || []).forEach((c) => node.appendChild(typeof c === "string" ? document.createTextNode(c) : c));
    return node;
  }

  function clear(node) {
    while (node.firstChild) node.removeChild(node.firstChild);
  }

  function banner(kind, title, reasons) {
    const b = el("div", { class: "banner banner-" + kind }, [title]);
    if (reasons && reasons.length) {
      const ul = el("ul", {}, reasons.map((r) => el("li", {}, [r])));
      b.appendChild(ul);
    }
    return b;
  }

  function kvTable(rows) {
    const table = el("table", { class: "kv" });
    rows.forEach(([k, v]) => {
      if (v === undefined || v === null) return;
      table.appendChild(el("tr", {}, [el("td", {}, [k]), el("td", {}, [String(v)])]));
    });
    return table;
  }

  function copyable(text) {
    const wrap = el("span", { class: "copyable" }, [text]);
    const btn = el("button", { class: "copy-inline", type: "button" }, ["copy"]);
    btn.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(text);
        btn.textContent = "copied";
        setTimeout(() => (btn.textContent = "copy"), 1200);
      } catch (e) {}
    });
    wrap.appendChild(btn);
    return wrap;
  }

  // ===== QR: show a written shard/share as a scannable code, and import one =====
  // Reuses the same HX3 QR transport the TUI and CLI already use
  // (src/qr.rs) via the /api/shard-qr and /api/shard-qr-import endpoints —
  // no new crypto here, just base64 PNG frames over the existing token-gated API.

  function qrShowButton(path) {
    const wrap = el("div", { style: "margin-top:6px" });
    const btn = el("button", { class: "btn btn-outline", type: "button" }, ["Show as QR"]);
    const frames = el("div", { class: "qr-frames" });
    btn.addEventListener("click", async () => {
      clear(frames);
      btn.disabled = true;
      try {
        const res = await api("/shard-qr", { method: "POST", body: { path } });
        res.frames.forEach((b64, i) => {
          const cell = el("div", { class: "qr-frame" });
          cell.appendChild(el("img", { src: "data:image/png;base64," + b64, alt: "QR frame " + (i + 1) }));
          if (res.frames.length > 1) {
            cell.appendChild(el("p", { class: "qr-frame-label" }, ["frame " + (i + 1) + "/" + res.frames.length]));
          }
          frames.appendChild(cell);
        });
      } catch (e) {
        clear(frames);
        frames.appendChild(banner("error", e.message));
      } finally {
        btn.disabled = false;
      }
    });
    wrap.appendChild(btn);
    wrap.appendChild(frames);
    return wrap;
  }

  function fileToBase64(file) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(",").pop());
      reader.onerror = () => reject(reader.error || new Error("failed to read file"));
      reader.readAsDataURL(file);
    });
  }

  /// Wires an "Import from QR" file input + button onto `textareaName`
  /// (the `shards`/`shares` field) within `form`, appending each imported
  /// temp path as a new line.
  function wireQrImport(form, textareaName, inputId, buttonId) {
    const input = form.querySelector("#" + inputId);
    const button = form.querySelector("#" + buttonId);
    const textarea = form.querySelector('[name="' + textareaName + '"]');
    if (!input || !button || !textarea) return;
    button.addEventListener("click", async () => {
      const files = Array.from(input.files || []);
      if (!files.length) return;
      button.disabled = true;
      try {
        for (const file of files) {
          const png_base64 = await fileToBase64(file);
          const res = await api("/shard-qr-import", { method: "POST", body: { png_base64 } });
          const sep = textarea.value && !textarea.value.endsWith("\n") ? "\n" : "";
          textarea.value += sep + res.path;
        }
        input.value = "";
      } catch (e) {
        alert("QR import failed: " + e.message);
      } finally {
        button.disabled = false;
      }
    });
  }

  // ===== Section switching =====
  // Section and MPC-sub-tab changes push a URL hash (e.g. "#mpc/sign") via
  // history.pushState, and a popstate listener re-derives the active
  // section/tab from location.hash. This makes the browser Back/Forward
  // buttons work as real in-app navigation and makes every section
  // deep-linkable, instead of navigating away from the app entirely (which
  // previously dropped the ?token= query param). location.search — and so
  // ?token= — is untouched by every hash change below.
  const VALID_SECTIONS = ["log", "verify", "init", "sign", "mpc"];
  const railItems = document.querySelectorAll(".rail-item");
  const sections = document.querySelectorAll("[data-section]");

  function activateSection(name, opts) {
    const push = !opts || opts.push !== false;
    railItems.forEach((b) => b.classList.toggle("is-active", b.dataset.section === name));
    sections.forEach((s) => {
      s.hidden = s.id !== "section-" + name;
    });
    if (name === "log") loadLog();
    if (push) history.pushState(null, "", "#" + name);
  }
  railItems.forEach((btn) => {
    btn.addEventListener("click", () => activateSection(btn.dataset.section));
  });

  // ===== Access log =====
  function kindBadge(kind) {
    const map = { decrypt_ok: "ok", decrypt_fail: "fail", blocked: "blocked", signed: "signed" };
    return el("span", { class: "badge badge-" + (map[kind] || "ok") }, [kind]);
  }

  async function loadLog() {
    const result = document.getElementById("logResult");
    clear(result);
    result.appendChild(el("p", {}, ["Loading…"]));
    try {
      const tailInput = document.getElementById("logTail").value;
      const q = tailInput ? "?tail=" + encodeURIComponent(tailInput) : "";
      const data = await api("/log" + q);
      clear(result);
      if (!data.entries.length) {
        result.appendChild(el("p", {}, ["No access log entries yet."]));
        return;
      }
      const table = el("table", { class: "data" });
      table.appendChild(
        el("tr", {}, ["Time (UTC)", "Attempt", "Shard", "Kind"].map((h) => el("th", {}, [h])))
      );
      data.entries
        .slice()
        .reverse()
        .forEach((e) => {
          table.appendChild(
            el("tr", {}, [
              el("td", {}, [e.ts_utc]),
              el("td", {}, [String(e.attempt)]),
              el("td", {}, [e.shard_id ? String(e.shard_id) : "—"]),
              el("td", {}, [kindBadge(e.kind)]),
            ])
          );
        });
      result.appendChild(table);
    } catch (e) {
      clear(result);
      result.appendChild(banner("error", e.message));
    }
  }
  document.getElementById("logReload").addEventListener("click", loadLog);

  // ===== Helpers shared by forms =====
  function lines(value) {
    return value
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean);
  }

  function setBusy(form, busy) {
    form.querySelectorAll("button[type=submit]").forEach((b) => (b.disabled = busy));
  }

  function formData(form) {
    const fd = new FormData(form);
    const out = {};
    for (const [k, v] of fd.entries()) out[k] = v;
    form.querySelectorAll('input[type=checkbox]').forEach((cb) => {
      out[cb.name] = cb.checked;
    });
    return out;
  }

  async function handleBlocked(err, form, result, retry) {
    clear(result);
    if (err.status === 409 && err.data && err.data.reasons) {
      result.appendChild(banner("error", "Audit blocked this attempt:", err.data.reasons));
      const forceField = form.querySelector(".force-field");
      if (forceField) {
        forceField.hidden = false;
        forceField.querySelector("input").checked = true;
      }
    } else {
      result.appendChild(banner("error", err.message));
    }
  }

  // ===== Verify =====
  const verifyForm = document.getElementById("verifyForm");
  verifyForm.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const result = document.getElementById("verifyResult");
    const data = formData(verifyForm);
    setBusy(verifyForm, true);
    clear(result);
    try {
      const res = await api("/verify", {
        method: "POST",
        body: { files: lines(data.files), password: data.password || null },
      });
      clear(result);
      if (res.consistency_error) {
        result.appendChild(banner("error", "Inconsistent set: " + res.consistency_error));
      } else {
        result.appendChild(
          banner(res.all_ok ? "ok" : "error", res.all_ok ? "All files verified." : "Verification failed for one or more files.")
        );
      }
      const table = el("table", { class: "data" });
      table.appendChild(el("tr", {}, ["Path", "Kind", "t / n", "Status"].map((h) => el("th", {}, [h]))));
      res.reports.forEach((r) => {
        table.appendChild(
          el("tr", {}, [
            el("td", {}, [r.path]),
            el("td", {}, [r.kind ? r.kind.toUpperCase() : "invalid"]),
            el("td", {}, [r.threshold != null ? r.threshold + " / " + r.shares : "—"]),
            el("td", {}, [r.ok ? el("span", { class: "badge badge-ok" }, ["ok"]) : el("span", { class: "badge badge-fail" }, ["fail"])]),
          ])
        );
      });
      result.appendChild(table);
    } catch (e) {
      clear(result);
      result.appendChild(banner("error", e.message));
    } finally {
      setBusy(verifyForm, false);
    }
  });

  // ===== Init =====
  const initForm = document.getElementById("initForm");
  initForm.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const result = document.getElementById("initResult");
    const data = formData(initForm);
    setBusy(initForm, true);
    clear(result);
    try {
      const res = await api("/init", {
        method: "POST",
        body: {
          threshold: Number(data.threshold),
          shares: Number(data.shares),
          key_hex: data.key_hex || null,
          generate: !!data.generate,
          out_dir: data.out_dir,
          password: data.password,
        },
      });
      clear(result);
      if (res.generated_key_hex) {
        const b = banner("warn", "Generated test key (shown once — never use for real funds):");
        b.appendChild(el("div", { style: "margin-top:8px" }, [copyable("0x" + res.generated_key_hex)]));
        result.appendChild(b);
      }
      result.appendChild(banner("ok", "Wrote " + res.paths.length + " shard(s) to " + res.out_dir));
      result.appendChild(
        el(
          "ul",
          { class: "file-list" },
          res.paths.map((p) => el("li", {}, [p, qrShowButton(p)]))
        )
      );
    } catch (e) {
      clear(result);
      result.appendChild(banner("error", e.message));
    } finally {
      setBusy(initForm, false);
    }
  });

  // ===== Sign =====
  const signForm = document.getElementById("signForm");
  wireQrImport(signForm, "shards", "qrImportInputSign", "qrImportBtnSign");
  signForm.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const result = document.getElementById("signResult");
    const data = formData(signForm);
    setBusy(signForm, true);
    try {
      const res = await api("/sign", {
        method: "POST",
        body: {
          shards: lines(data.shards),
          password: data.password,
          to: data.to,
          lamports: Number(data.lamports),
          blockhash: data.blockhash || null,
          broadcast: !!data.broadcast,
          rpc_url: data.rpc_url || null,
          force: !!data.force,
        },
      });
      clear(result);
      if (res.warnings && res.warnings.length) result.appendChild(banner("warn", "Audit warnings:", res.warnings));
      result.appendChild(
        kvTable([
          ["From", res.from],
          ["Signature", res.signature],
          ["Raw (base58)", res.raw_base58],
          ["Mined", res.mined_signature],
          ["Broadcast error", res.broadcast_error],
        ])
      );
      result.insertBefore(banner("ok", "Signed."), result.firstChild);
    } catch (e) {
      await handleBlocked(e, signForm, result);
    } finally {
      setBusy(signForm, false);
    }
  });

  // ===== MPC (Split / Sign tabs) =====
  const mpcTabs = document.querySelectorAll("[data-mpc-tab].tab");
  const mpcForms = document.querySelectorAll("form.mpc-tab");

  function activateMpcTab(which, opts) {
    const push = !opts || opts.push !== false;
    mpcTabs.forEach((t) => t.classList.toggle("is-active", t.dataset.mpcTab === which));
    mpcForms.forEach((f) => (f.hidden = f.dataset.mpcTab !== which));
    clear(document.getElementById("mpcResult"));
    if (push) history.pushState(null, "", "#mpc/" + which);
  }
  mpcTabs.forEach((tab) => {
    tab.addEventListener("click", () => activateMpcTab(tab.dataset.mpcTab));
  });

  document.getElementById("mpcSplitForm").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const form = ev.target;
    const result = document.getElementById("mpcResult");
    const data = formData(form);
    setBusy(form, true);
    clear(result);
    try {
      const res = await api("/mpc-split", {
        method: "POST",
        body: {
          threshold: Number(data.threshold),
          shares: Number(data.shares),
          key_hex: data.key_hex || null,
          generate: !!data.generate,
          out_dir: data.out_dir,
          password: data.password,
        },
      });
      clear(result);
      if (res.generated_key_hex) {
        const b = banner("warn", "Generated test key (shown once — never use for real funds):");
        b.appendChild(el("div", { style: "margin-top:8px" }, [copyable("0x" + res.generated_key_hex)]));
        result.appendChild(b);
      }
      result.appendChild(banner("ok", "Wrote " + res.paths.length + " key share(s). Group package: " + res.group_path));
      result.appendChild(
        el(
          "ul",
          { class: "file-list" },
          res.paths.map((p) => el("li", {}, [p, qrShowButton(p)]))
        )
      );
    } catch (e) {
      clear(result);
      result.appendChild(banner("error", e.message));
    } finally {
      setBusy(form, false);
    }
  });

  const mpcSignForm = document.getElementById("mpcSignForm");
  wireQrImport(mpcSignForm, "shares", "qrImportInputMpcSign", "qrImportBtnMpcSign");
  mpcSignForm.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const form = ev.target;
    const result = document.getElementById("mpcResult");
    const data = formData(form);
    setBusy(form, true);
    try {
      const res = await api("/mpc-sign", {
        method: "POST",
        body: {
          shares: lines(data.shares),
          group_dir: data.group_dir,
          password: data.password,
          to: data.to,
          lamports: Number(data.lamports),
          blockhash: data.blockhash || null,
          broadcast: !!data.broadcast,
          rpc_url: data.rpc_url || null,
          force: !!data.force,
        },
      });
      clear(result);
      if (res.warnings && res.warnings.length) result.appendChild(banner("warn", "Audit warnings:", res.warnings));
      result.appendChild(
        kvTable([
          ["From", res.from],
          ["Signature", res.signature],
          ["Raw (base58)", res.raw_base58],
          ["Mined", res.mined_signature],
          ["Broadcast error", res.broadcast_error],
        ])
      );
      result.insertBefore(banner("ok", "Signed with FROST."), result.firstChild);
    } catch (e) {
      await handleBlocked(e, form, result);
    } finally {
      setBusy(form, false);
    }
  });

  // ===== Hash router =====
  function applyHashRoute(push) {
    const [rawSection, subtab] = (location.hash.slice(1) || "log").split("/");
    const section = VALID_SECTIONS.includes(rawSection) ? rawSection : "log";
    activateSection(section, { push });
    if (section === "mpc" && (subtab === "split" || subtab === "sign")) {
      activateMpcTab(subtab, { push });
    }
  }
  window.addEventListener("popstate", () => applyHashRoute(false));
  const backBtn = document.getElementById("backBtn");
  if (backBtn) backBtn.addEventListener("click", () => history.back());

  applyHashRoute(false);
})();
