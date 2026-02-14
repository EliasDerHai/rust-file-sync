const CACHE = 'linkshare';

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', () => self.clients.claim());

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);

  // API calls always go to network
  if (url.pathname.startsWith('/api/')) {
    event.respondWith(fetch(event.request));
    return;
  }

  // Stale-while-revalidate with ignoreSearch so share.html?title=...&text=...
  // matches the cached share.html
  event.respondWith(
    caches.open(CACHE).then((cache) =>
      cache.match(event.request, { ignoreSearch: true }).then((cached) => {
        const fetched = fetch(event.request)
          .then((response) => {
            cache.put(event.request, response.clone());
            return response;
          })
          .catch(() => cached);
        return cached || fetched;
      })
    )
  );
});
