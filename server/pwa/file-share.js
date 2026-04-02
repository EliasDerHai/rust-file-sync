// file-share.js — page-side helpers for the file share flow.
// Uses the same IndexedDB database/store as sw.js.

const SHARE_DB_NAME = 'linkshare-share-db';
const SHARE_STORE = 'pending-shares';

// ── IndexedDB ────────────────────────────────────────────────────────────────

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

export async function getShare(shareId) {
  const db = await openShareDb();
  return new Promise((resolve, reject) => {
    const req = db.transaction(SHARE_STORE).objectStore(SHARE_STORE).get(shareId);
    req.onsuccess = () => resolve(req.result || null);
    req.onerror = () => reject(req.error);
  });
}

export async function deleteShare(shareId) {
  const db = await openShareDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(SHARE_STORE, 'readwrite');
    tx.objectStore(SHARE_STORE).delete(shareId);
    tx.oncomplete = resolve;
    tx.onerror = () => reject(tx.error);
  });
}

// ── API ──────────────────────────────────────────────────────────────────────

export async function getWatchGroups() {
  const response = await fetch('/api/watch-groups');
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${await response.text()}`);
  }
  return response.json(); // [{id, name}, ...]
}

export async function uploadFileToWatchGroup(file, wgId) {
  const formData = new FormData();
  formData.append('file', file, file.name);
  const response = await fetch(`/api/watch-groups/${wgId}/files`, {
    method: 'POST',
    body: formData,
  });
  if (!response.ok) {
    throw new Error(`Upload failed (${response.status}): ${await response.text()}`);
  }
}

// ── Utilities ────────────────────────────────────────────────────────────────

export function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
