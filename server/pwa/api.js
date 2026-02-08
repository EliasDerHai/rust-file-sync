export const linksPath = "/api/links";
const PENDING_KEY = "pendingLinks";

// Get pending queue from localStorage
function getPendingLinks() {
  return JSON.parse(localStorage.getItem(PENDING_KEY) || "[]");
}

// Save pending queue to localStorage
function savePendingLinks(links) {
  localStorage.setItem(PENDING_KEY, JSON.stringify(links));
}

// Queue a link for later sync
function queueLink(url, title) {
  const pending = getPendingLinks();
  pending.push({ url, title, timestamp: Date.now() });
  savePendingLinks(pending);
}

export async function getLinks() {
  const response = await fetch(linksPath, {
    method: "GET",
    headers: { "Content-Type": "application/json" },
  });

  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${await response.text()}`);
  }

  return response.json();
}

// POST a link - queues to localStorage on failure
export async function postLink(url, title) {
  try {
    const response = await fetch(linksPath, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ url, title }),
    });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return { synced: true };
  } catch (err) {
    queueLink(url, title);
    return { synced: false, queued: true };
  }
}

// Sync all pending links - call on page load
export async function syncPendingLinks() {
  const pending = getPendingLinks();
  if (pending.length === 0) return { synced: 0, failed: 0 };

  const results = { synced: 0, failed: 0 };
  const stillPending = [];

  for (const link of pending) {
    try {
      const response = await fetch(linksPath, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url: link.url, title: link.title }),
      });
      if (response.ok) {
        results.synced++;
      } else {
        stillPending.push(link);
        results.failed++;
      }
    } catch {
      stillPending.push(link);
      results.failed++;
    }
  }

  savePendingLinks(stillPending);
  return results;
}

// Get count of pending links (for UI)
export function getPendingCount() {
  return getPendingLinks().length;
}
