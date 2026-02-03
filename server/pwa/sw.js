// Service worker - required for PWA installability and share target registration
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', () => self.clients.claim());
self.addEventListener('fetch', (event) => event.respondWith(fetch(event.request)));
