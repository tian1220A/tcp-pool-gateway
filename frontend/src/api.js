const BASE = '/api';

async function json(res) {
  if (!res.ok) {
    throw new Error(`HTTP ${res.status}`);
  }
  return res.json();
}

export async function fetchStats() {
  const res = await fetch(`${BASE}/pool/stats`);
  return (await json(res)).stats;
}

export async function fetchHistory() {
  const res = await fetch(`${BASE}/pool/history`);
  return json(res);
}

export async function fetchLogs() {
  const res = await fetch(`${BASE}/logs`);
  return json(res);
}

export async function updateConfig(cfg) {
  const res = await fetch(`${BASE}/pool/config`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(cfg),
  });
  return (await json(res)).stats;
}
