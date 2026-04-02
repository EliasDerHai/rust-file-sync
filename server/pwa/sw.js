const CACHE = 'linkshare';
const SHARE_DB_NAME = 'linkshare-share-db';
const SHARE_STORE = 'pending-shares';
const SHARE_TTL_MS = 24 * 60 * 60 * 1000; // 24 hours

// ── IndexedDB helpers ────────────────────────────────────────────────────────

function openShareDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(SHARE_DB_NAME, 1);
    req.onupgradeneeded = (e) => {
      e.target.result.createObjectStore(SHARE_STORE, { keyPath: 'shareId' });
    };
    req.onsuccess = (e) => resolve(e.target.result);
    req.onerror = () => reject(req.error);
  });
}

async function storeShare(data) {
  const db = await openShareDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(SHARE_STORE, 'readwrite');
    tx.objectStore(SHARE_STORE).put(data);
    tx.oncomplete = resolve;
    tx.onerror = () => reject(tx.error);
  });
}

async function getShare(shareId) {
  const db = await openShareDb();
  return new Promise((resolve, reject) => {
    const req = db.transaction(SHARE_STORE).objectStore(SHARE_STORE).get(shareId);
    req.onsuccess = () => resolve(req.result || null);
    req.onerror = () => reject(req.error);
  });
}

async function deleteShare(shareId) {
  const db = await openShareDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(SHARE_STORE, 'readwrite');
    tx.objectStore(SHARE_STORE).delete(shareId);
    tx.oncomplete = resolve;
    tx.onerror = () => reject(tx.error);
  });
}

async function deleteExpiredShares() {
  const cutoff = Date.now() - SHARE_TTL_MS;
  const db = await openShareDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(SHARE_STORE, 'readwrite');
    const store = tx.objectStore(SHARE_STORE);
    const req = store.openCursor();
    req.onsuccess = (e) => {
      const cursor = e.target.result;
      if (!cursor) return;
      if (cursor.value.timestamp < cutoff) cursor.delete();
      cursor.continue();
    };
    tx.oncomplete = resolve;
    tx.onerror = () => reject(tx.error);
  });
}

// ── Share POST handler ───────────────────────────────────────────────────────

async function handleSharePost(request) {
  try {
    const formData = await request.formData();
    const file = formData.get('file');
    const isFile = file instanceof File && file.size > 0;

    const shareId = crypto.randomUUID();
    await storeShare({
      shareId,
      type: isFile ? 'file' : 'url',
      url: formData.get('url'),
      title: formData.get('title'),
      text: formData.get('text'),
      file: isFile ? file : null,
      timestamp: Date.now(),
    });
    return Response.redirect(`/pwa/share.html?shareId=${shareId}`, 303);
  } catch (err) {
    return Response.redirect('/pwa/share.html?error=storage', 303);
  }
}

// ── Service worker lifecycle ─────────────────────────────────────────────────

self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', (event) => {
  self.clients.claim();
  event.waitUntil(deleteExpiredShares().catch(() => {}));
});

// ── Fetch handler ────────────────────────────────────────────────────────────

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);

  // Intercept the Web Share Target POST
  if (url.pathname === '/pwa/share.html' && event.request.method === 'POST') {
    event.respondWith(handleSharePost(event.request));
    return;
  }

  // API calls always go to network
  if (url.pathname.startsWith('/api/')) {
    event.respondWith(fetch(event.request));
    return;
  }

  // Static assets: stale-while-revalidate with ignoreSearch so
  // share.html?shareId=... still matches the cached share.html
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
