// Live results for the header search box, from /api/v1/search. Without this file the
// form still submits to /search, which renders the same results on the server.
(() => {
  "use strict";
  const form = document.querySelector("form.search");
  if (!form) return;
  const input = form.querySelector('input[name="q"]');
  const list = form.querySelector(".live");
  if (!input || !list) return;

  const LIMIT = 8;
  let timer = 0;
  let controller = null;
  let active = -1;
  let lastQuery = "";

  const links = () => Array.from(list.querySelectorAll("a[data-hit]"));

  function hide() {
    list.hidden = true;
    active = -1;
    input.setAttribute("aria-expanded", "false");
  }

  function show() {
    if (!list.childElementCount) return;
    list.hidden = false;
    input.setAttribute("aria-expanded", "true");
  }

  function highlight(i) {
    const all = links();
    all.forEach((a, j) => a.classList.toggle("on", j === i));
    active = i;
    if (all[i]) all[i].scrollIntoView({ block: "nearest" });
  }

  function hrefFor(hit) {
    return "/d/" + hit.doc + (hit.anchor ? "#" + hit.anchor : "");
  }

  function line(className, text) {
    const li = document.createElement("li");
    li.className = className;
    li.textContent = text;
    return li;
  }

  function render(query, hits) {
    list.replaceChildren();
    const docs = hits.filter((h) => h.kind === "doc");
    if (!docs.length) {
      list.append(line("empty", "No results."));
    }
    for (const hit of docs) {
      const li = document.createElement("li");
      li.setAttribute("role", "option");
      const a = document.createElement("a");
      a.href = hrefFor(hit);
      a.dataset.hit = "";
      const t = document.createElement("span");
      t.className = "t";
      t.textContent = hit.title;
      const s = document.createElement("span");
      s.className = "s";
      s.textContent = hit.snippet;
      a.append(t, s);
      li.append(a);
      list.append(li);
    }
    if (docs.length) {
      const more = document.createElement("li");
      more.className = "all";
      const a = document.createElement("a");
      a.href = "/search?q=" + encodeURIComponent(query);
      a.textContent = "All results";
      more.append(a);
      list.append(more);
    }
    active = -1;
    show();
  }

  async function run(query) {
    if (controller) controller.abort();
    controller = new AbortController();
    try {
      const url = "/api/v1/search?limit=" + LIMIT + "&q=" + encodeURIComponent(query);
      const resp = await fetch(url, { signal: controller.signal, headers: { accept: "application/json" } });
      if (!resp.ok) return;
      const hits = await resp.json();
      if (query === input.value.trim()) render(query, hits);
    } catch (_) {
      // Aborted or offline: the form still works.
    }
  }

  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");

  input.addEventListener("input", () => {
    const query = input.value.trim();
    clearTimeout(timer);
    if (query === lastQuery) return;
    lastQuery = query;
    if (!query) {
      list.replaceChildren();
      hide();
      return;
    }
    timer = setTimeout(() => run(query), 120);
  });

  input.addEventListener("keydown", (e) => {
    const all = links();
    if (e.key === "ArrowDown" && all.length) {
      e.preventDefault();
      show();
      highlight((active + 1) % all.length);
    } else if (e.key === "ArrowUp" && all.length) {
      e.preventDefault();
      highlight(active <= 0 ? all.length - 1 : active - 1);
    } else if (e.key === "Enter" && active >= 0 && all[active]) {
      e.preventDefault();
      window.location.href = all[active].href;
    } else if (e.key === "Escape") {
      hide();
    }
  });

  input.addEventListener("focus", show);
  document.addEventListener("click", (e) => {
    if (!form.contains(e.target)) hide();
  });

  // "/" focuses search, as on most docs sites.
  document.addEventListener("keydown", (e) => {
    if (e.key !== "/" || e.metaKey || e.ctrlKey || e.altKey) return;
    const tag = document.activeElement && document.activeElement.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    e.preventDefault();
    input.focus();
  });
})();
