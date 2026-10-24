// Service worker for cooking-rs.
//
// Strategy: network-first, falling back to cache. Successful same-origin GET
// responses are cached as they are fetched, so after the first load the whole
// app (wasm, JS, CSS, HTML — all data is embedded in the wasm) works offline.
// We don't precache hashed asset names because Trunk fingerprints them.

const CACHE = "cooking-rs-v2";

// On a local dev host a service worker sits in front of a live-reloading dev
// server and happily serves stale/broken assets. Detect dev and self-destruct
// instead, so a worker left over from an earlier session cleans itself up.
const IS_DEV = ["localhost", "127.0.0.1"].includes(self.location.hostname);

self.addEventListener("install", (event) => {
  self.skipWaiting();
  if (IS_DEV) return;
  // Seed the cache with the app shell so the first offline navigation works.
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(["./", "./index.html"])));
});

self.addEventListener("activate", (event) => {
  if (IS_DEV) {
    // Remove ourselves and every cache, then reload controlled pages so they
    // fetch straight from the dev server.
    event.waitUntil(
      caches
        .keys()
        .then((keys) => Promise.all(keys.map((k) => caches.delete(k))))
        .then(() => self.registration.unregister())
        .then(() => self.clients.matchAll())
        .then((clients) => clients.forEach((c) => c.navigate(c.url)))
    );
    return;
  }

  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (event) => {
  // In dev, never intercept — let every request hit the network directly.
  if (IS_DEV) return;

  const req = event.request;
  if (req.method !== "GET") return;
  if (new URL(req.url).origin !== self.location.origin) return;

  event.respondWith(
    fetch(req)
      .then((res) => {
        // Only cache successful, same-origin responses. Caching 404s/errors here
        // is what makes a missing asset resurface later as a broken image.
        if (res.ok && res.type === "basic") {
          const copy = res.clone();
          caches.open(CACHE).then((cache) => cache.put(req, copy));
        }
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
