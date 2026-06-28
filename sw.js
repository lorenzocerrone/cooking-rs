// Service worker for cooking-rs.
//
// Strategy: network-first, falling back to cache. Successful same-origin GET
// responses are cached as they are fetched, so after the first load the whole
// app (wasm, JS, CSS, HTML — all data is embedded in the wasm) works offline.
// We don't precache hashed asset names because Trunk fingerprints them.

const CACHE = "cooking-rs-v1";

self.addEventListener("install", (event) => {
  self.skipWaiting();
  // Seed the cache with the app shell so the first offline navigation works.
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(["./", "./index.html"])));
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  if (new URL(req.url).origin !== self.location.origin) return;

  event.respondWith(
    fetch(req)
      .then((res) => {
        const copy = res.clone();
        caches.open(CACHE).then((cache) => cache.put(req, copy));
        return res;
      })
      .catch(() =>
        caches.match(req).then((hit) => {
          if (hit) return hit;
          // SPA fallback: serve the cached shell for navigations.
          if (req.mode === "navigate") return caches.match("./index.html");
          return Response.error();
        })
      )
  );
});
