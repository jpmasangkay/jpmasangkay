const encoder = new TextEncoder();
const decoder = new TextDecoder();

const RETRYABLE_STATUSES = new Set([429, 500, 502, 503, 504]);
const MAX_GRAPHQL_ATTEMPTS = 3;
const REQUEST_TIMEOUT_MS = 20_000;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function fnv1aHex(str) {
  let hash = 0x811c9dc5;
  for (let i = 0; i < str.length; i += 1) {
    hash ^= str.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function parseIntOr(raw, fallback) {
  const parsed = Number.parseInt(String(raw ?? ""), 10);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function readConfig(env) {
  return {
    username: (env.GITHUB_USERNAME || "jpmasangkay").trim(),
    tzOffsetHours: Math.max(-12, Math.min(14, parseIntOr(env.TZ_OFFSET_HOURS, 8))),
    ttlSeconds: Math.max(30, parseIntOr(env.CACHE_TTL_SECONDS, 60)),
  };
}

function kvKeys(username, tzOffsetHours) {
  const base = `v1:${username}:${tzOffsetHours}`;
  return {
    dark: `${base}:dark`,
    light: `${base}:light`,
    hash: `${base}:hash`,
  };
}

function resolveTheme(url) {
  const path = url.pathname.replace(/\/+$/, "") || "/";
  if (path === "/metrics-dark.svg" || path === "/dark.svg") {
    return "dark";
  }
  if (path === "/metrics-light.svg" || path === "/light.svg") {
    return "light";
  }
  if (path === "/") {
    const q = (url.searchParams.get("theme") || "").toLowerCase();
    if (q === "light") return "light";
    if (q === "dark" || q === "") return "dark";
  }
  return null;
}

async function postGraphQlWithRetry(fetchFn, endpoint, token, userAgent, query, variables) {
  const payload = JSON.stringify({ query, variables });

  for (let attempt = 1; attempt <= MAX_GRAPHQL_ATTEMPTS; attempt += 1) {
    try {
      const init = {
        method: "POST",
        headers: {
          Authorization: `Bearer ${token}`,
          "User-Agent": userAgent,
          Accept: "application/vnd.github+json",
          "Content-Type": "application/json",
        },
        body: payload,
      };
      if (typeof AbortSignal !== "undefined" && typeof AbortSignal.timeout === "function") {
        init.signal = AbortSignal.timeout(REQUEST_TIMEOUT_MS);
      }

      const resp = await fetchFn(endpoint, init);
      const body = await resp.text();

      if (resp.ok) {
        return body;
      }

      if (RETRYABLE_STATUSES.has(resp.status) && attempt < MAX_GRAPHQL_ATTEMPTS) {
        await sleep(400 * 2 ** (attempt - 1));
        continue;
      }

      throw new Error(`GitHub GraphQL HTTP ${resp.status}: ${body.slice(0, 300)}`);
    } catch (err) {
      if (attempt < MAX_GRAPHQL_ATTEMPTS) {
        await sleep(400 * 2 ** (attempt - 1));
        continue;
      }
      throw err;
    }
  }

  throw new Error("GitHub GraphQL retries exhausted");
}

function buildSvgResponse(svg, ttlSeconds, cacheStatus, isHead, request) {
  const etag = `"${fnv1aHex(svg)}"`;
  const headers = {
    "Content-Type": "image/svg+xml; charset=utf-8",
    "Cache-Control": `public, max-age=${ttlSeconds}, s-maxage=${ttlSeconds}, stale-while-revalidate=300`,
    ETag: etag,
    "X-Content-Type-Options": "nosniff",
    "X-Metrics-Cache": cacheStatus,
  };

  if (request && request.headers.get("If-None-Match") === etag) {
    return new Response(null, { status: 304, headers });
  }

  return new Response(isHead ? null : svg, { status: 200, headers });
}

export function createHandler(wasmModule, options = {}) {
  const fetchFn = options.fetch ?? ((input, init) => fetch(input, init));
  const nowFn = options.now ?? (() => Date.now());
  const getEdgeCache =
    options.getEdgeCache ??
    (() => (typeof caches !== "undefined" && caches.default ? caches.default : null));

  let wasmExports = null;
  let memoryCache = null;
  let inFlight = null;

  function getWasm() {
    if (!wasmExports) {
      const instance =
        wasmModule instanceof WebAssembly.Instance
          ? wasmModule
          : new WebAssembly.Instance(wasmModule, {});
      wasmExports = instance.exports;
    }
    return wasmExports;
  }

  function callWasm(fnName, payload) {
    const wasm = getWasm();
    const inputBytes = encoder.encode(JSON.stringify(payload));
    const inPtr = wasm.alloc(inputBytes.length);
    if (inPtr === 0 && inputBytes.length > 0) {
      throw new Error("WASM allocation failed");
    }

    try {
      new Uint8Array(wasm.memory.buffer, inPtr, inputBytes.length).set(inputBytes);
      const packed = wasm[fnName](inPtr, inputBytes.length);
      const outPtr = Number(packed >> 32n);
      const outLen = Number(packed & 0xffffffffn);
      try {
        const outBytes = new Uint8Array(wasm.memory.buffer, outPtr, outLen);
        const parsed = JSON.parse(decoder.decode(outBytes));
        if (!parsed.ok) {
          throw new Error(parsed.error || `WASM call ${fnName} failed`);
        }
        return parsed;
      } finally {
        wasm.dealloc(outPtr, outLen);
      }
    } finally {
      wasm.dealloc(inPtr, inputBytes.length);
    }
  }

  async function refreshAndPersist(env, username, tzOffsetHours, ttlSeconds) {
    const token = env.GITHUB_TOKEN;
    if (!token) {
      throw new Error("GITHUB_TOKEN secret is missing");
    }

    const nowMs = nowFn();

    const overviewReq = callWasm("wasm_prepare_overview", {
      username,
      now_ms: nowMs,
      tz_offset_hours: tzOffsetHours,
    });

    const overviewJson = await postGraphQlWithRetry(
      fetchFn,
      overviewReq.endpoint,
      token,
      overviewReq.user_agent,
      overviewReq.query,
      overviewReq.variables
    );

    const identity = callWasm("wasm_parse_overview_identity", {
      username,
      overview_json: overviewJson,
    });

    const repoPagesJson = [];
    let cursor = null;
    for (let pageIndex = 0; pageIndex < 20; pageIndex += 1) {
      const pageReq = callWasm("wasm_prepare_repos_page", {
        login: identity.login,
        author_id: identity.author_id,
        cursor,
        now_ms: nowMs,
      });

      const pageJson = await postGraphQlWithRetry(
        fetchFn,
        overviewReq.endpoint,
        token,
        overviewReq.user_agent,
        pageReq.query,
        pageReq.variables
      );
      repoPagesJson.push(pageJson);

      const pageCursor = callWasm("wasm_parse_repos_cursor", {
        page_json: pageJson,
      });
      if (pageCursor.has_next_page && pageCursor.end_cursor) {
        cursor = pageCursor.end_cursor;
      } else {
        break;
      }
    }

    const rendered = callWasm("wasm_render_svgs", {
      username,
      now_ms: nowMs,
      tz_offset_hours: tzOffsetHours,
      overview_json: overviewJson,
      repo_pages_json: repoPagesJson,
    });

    const combinedHash = `${fnv1aHex(rendered.dark_svg)}:${fnv1aHex(rendered.light_svg)}`;
    const entry = {
      key: `${username}:${tzOffsetHours}`,
      dark: rendered.dark_svg,
      light: rendered.light_svg,
      hash: combinedHash,
      expiresAt: nowFn() + ttlSeconds * 1000,
    };
    memoryCache = entry;

    let kvWritten = false;
    if (env.METRICS_KV) {
      const keys = kvKeys(username, tzOffsetHours);
      const prevHash = await env.METRICS_KV.get(keys.hash, { type: "text" });
      if (prevHash !== combinedHash) {
        await Promise.all([
          env.METRICS_KV.put(keys.dark, rendered.dark_svg),
          env.METRICS_KV.put(keys.light, rendered.light_svg),
          env.METRICS_KV.put(keys.hash, combinedHash),
        ]);
        kvWritten = true;
      }
    }

    return { entry, kvWritten };
  }

  async function getFreshEntry(env, username, tzOffsetHours, ttlSeconds) {
    const cacheKey = `${username}:${tzOffsetHours}`;
    if (memoryCache && memoryCache.key === cacheKey && nowFn() < memoryCache.expiresAt) {
      return { entry: memoryCache, status: "HIT-MEM" };
    }

    if (!inFlight) {
      inFlight = refreshAndPersist(env, username, tzOffsetHours, ttlSeconds).finally(() => {
        inFlight = null;
      });
    }

    try {
      const { entry } = await inFlight;
      return { entry, status: "MISS" };
    } catch (err) {
      if (memoryCache && memoryCache.key === cacheKey) {
        return { entry: memoryCache, status: "STALE-FALLBACK" };
      }
      throw err;
    }
  }

  return {
    async scheduled(_event, env, ctx) {
      if (!env.GITHUB_TOKEN) {
        console.warn("Skipping scheduled refresh: GITHUB_TOKEN secret is not configured");
        return;
      }
      const { username, tzOffsetHours, ttlSeconds } = readConfig(env);
      const work = refreshAndPersist(env, username, tzOffsetHours, ttlSeconds)
        .then(({ entry, kvWritten }) => {
          console.log(
            `Scheduled metrics refresh complete for ${username}: hash=${entry.hash} kv_written=${kvWritten}`
          );
        })
        .catch((err) => {
          console.error(`Scheduled metrics refresh failed: ${err.message || String(err)}`);
        });

      if (ctx && typeof ctx.waitUntil === "function") {
        ctx.waitUntil(work);
      } else {
        await work;
      }
    },

    async fetch(request, env, ctx) {
      if (request.method !== "GET" && request.method !== "HEAD") {
        return new Response("Method Not Allowed", { status: 405 });
      }

      const url = new URL(request.url);
      const { username, tzOffsetHours, ttlSeconds } = readConfig(env);
      const keys = kvKeys(username, tzOffsetHours);

      if (url.pathname === "/health") {
        const kvHash = env.METRICS_KV
          ? await env.METRICS_KV.get(keys.hash, { type: "text" })
          : null;
        return new Response(
          JSON.stringify({
            ok: true,
            username,
            tz_offset_hours: tzOffsetHours,
            cache_ttl_seconds: ttlSeconds,
            token_configured: Boolean(env.GITHUB_TOKEN),
            kv_bound: Boolean(env.METRICS_KV),
            kv_warm: Boolean(kvHash),
            kv_hash: kvHash,
          }),
          {
            status: 200,
            headers: {
              "Content-Type": "application/json; charset=utf-8",
              "Cache-Control": "no-store",
            },
          }
        );
      }

      const theme = resolveTheme(url);
      if (!theme) {
        return new Response("Not Found. Try /metrics-dark.svg or /metrics-light.svg", {
          status: 404,
          headers: { "Content-Type": "text/plain; charset=utf-8" },
        });
      }

      if (!env.GITHUB_TOKEN) {
        return new Response("Server misconfigured: GITHUB_TOKEN secret is missing", {
          status: 500,
          headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store" },
        });
      }

      const isHead = request.method === "HEAD";
      const edgeCache = getEdgeCache();
      const darkCacheReq = new Request(new URL("/metrics-dark.svg", url.origin).toString(), {
        method: "GET",
      });
      const lightCacheReq = new Request(new URL("/metrics-light.svg", url.origin).toString(), {
        method: "GET",
      });
      const targetCacheReq = theme === "dark" ? darkCacheReq : lightCacheReq;

      // 1. Cloudflare per-colo edge cache (fastest, < 5ms).
      if (edgeCache) {
        const edgeHit = await edgeCache.match(targetCacheReq);
        if (edgeHit) {
          const headers = new Headers(edgeHit.headers);
          headers.set("X-Metrics-Cache", "HIT-EDGE");
          if (request.headers.get("If-None-Match") === headers.get("ETag")) {
            return new Response(null, { status: 304, headers });
          }
          return new Response(isHead ? null : edgeHit.body, {
            status: edgeHit.status,
            headers,
          });
        }
      }

      // 2. Isolate in-memory cache (< 2ms).
      const cacheKey = `${username}:${tzOffsetHours}`;
      if (memoryCache && memoryCache.key === cacheKey && nowFn() < memoryCache.expiresAt) {
        const svg = theme === "dark" ? memoryCache.dark : memoryCache.light;
        return buildSvgResponse(svg, ttlSeconds, "HIT-MEM", isHead, request);
      }

      // 3. Globally replicated Cloudflare KV populated by the 1-minute Cron Trigger (< 25ms).
      if (env.METRICS_KV) {
        const kvSvg = await env.METRICS_KV.get(theme === "dark" ? keys.dark : keys.light, {
          type: "text",
        });
        if (kvSvg) {
          const cacheableResp = buildSvgResponse(kvSvg, ttlSeconds, "HIT-KV", false, null);
          if (edgeCache) {
            const putTask = edgeCache.put(targetCacheReq, cacheableResp.clone());
            if (ctx && typeof ctx.waitUntil === "function") {
              ctx.waitUntil(putTask);
            } else {
              await putTask;
            }
          }
          return buildSvgResponse(kvSvg, ttlSeconds, "HIT-KV", isHead, request);
        }
      }

      // 4. Cold-boot fallback only when KV has never been populated yet.
      try {
        const { entry, status } = await getFreshEntry(env, username, tzOffsetHours, ttlSeconds);
        const darkResp = buildSvgResponse(entry.dark, ttlSeconds, status, false, null);
        const lightResp = buildSvgResponse(entry.light, ttlSeconds, status, false, null);

        if (edgeCache && status === "MISS") {
          const putAll = Promise.all([
            edgeCache.put(darkCacheReq, darkResp.clone()),
            edgeCache.put(lightCacheReq, lightResp.clone()),
          ]);
          if (ctx && typeof ctx.waitUntil === "function") {
            ctx.waitUntil(putAll);
          } else {
            await putAll;
          }
        }

        const svg = theme === "dark" ? entry.dark : entry.light;
        const effectiveTtl = status === "STALE-FALLBACK" ? 15 : ttlSeconds;
        return buildSvgResponse(svg, effectiveTtl, status, isHead, request);
      } catch (err) {
        return new Response(`Failed to render GitHub metrics: ${err.message || String(err)}`, {
          status: 502,
          headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store" },
        });
      }
    },
  };
}
