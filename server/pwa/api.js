export const linksPath = "/api/links";

export async function getLinks() {
  const response = await fetch(linksPath, {
    method: "GET",
    headers: { "Response-Type": "application/json" },
  });

  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${await response.text()}`);
  }

  return response.json();
}

export async function postLink(url, title) {
  const response = await fetch(linksPath, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ url, title }),
  });

  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${await response.text()}`);
  }
}
