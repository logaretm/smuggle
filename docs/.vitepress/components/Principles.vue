<script setup>
const ITEMS = [
  { glyph: "registry", title: "A real install", body: "Requests are answered from your machine, but resolution, download and install all happen exactly as they normally would." },
  { glyph: "links", title: "No symlinks", body: "Your package manager unpacks the tarball itself, so node_modules holds real files that bundlers and Node treat like any other dependency." },
  { glyph: "hash", title: "Caches cannot hide it", body: "The integrity in your lockfile is rewritten to your build. A hash nothing has ever seen cannot come from a cache, so the fetch has to go out." },
  { glyph: "lock", title: "Lockfile put back", body: "Pinned while a session runs, restored when it ends, and repaired on the next run if a session was killed outright." },
  { glyph: "json", title: "No package.json edits", body: "Version ranges are preserved. Smuggle never writes to your manifest." },
  { glyph: "broom", title: "Nothing outlives the session", body: "Interception lasts exactly as long as the command runs. Quitting removes the redirect and reinstalls the published packages." },
  { glyph: "graph", title: "Transitive packages follow", body: "If a package you are serving depends on another one you have published, both are served." },
];
</script>

<template>
  <div class="pp">
    <div v-for="(it, i) in ITEMS" :key="it.title" class="pp-card" :style="{ '--i': i }">
      <div class="pp-glyph">
        <svg v-if="it.glyph === 'links'" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M9 12 a4 4 0 0 1 4-4 h3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
          <path d="M15 12 a4 4 0 0 1 -4 4 h-3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
          <path d="M5 19 L19 5" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
        </svg>
        <svg v-else-if="it.glyph === 'lock'" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M8 11 V8 a4 4 0 0 1 8 0 v3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
          <rect x="6" y="11" width="12" height="9" rx="2" fill="currentColor" opacity="0.18" stroke="currentColor" stroke-width="1.8" />
          <path d="M10 15 L11.5 16.5 L14.5 13.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <svg v-else-if="it.glyph === 'registry'" viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="1.8" />
          <ellipse cx="12" cy="12" rx="3.5" ry="8" fill="none" stroke="currentColor" stroke-width="1.5" />
          <path d="M4 12 H20" stroke="currentColor" stroke-width="1.5" />
          <path d="M5 18 L19 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
        </svg>
        <svg v-else-if="it.glyph === 'json'" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M9 5 h-2 a2 2 0 0 0 -2 2 v3 a2 2 0 0 1 -2 2 a2 2 0 0 1 2 2 v3 a2 2 0 0 0 2 2 h2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
          <path d="M15 5 h2 a2 2 0 0 1 2 2 v3 a2 2 0 0 0 2 2 a2 2 0 0 0 -2 2 v3 a2 2 0 0 1 -2 2 h-2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
        </svg>
        <svg v-else-if="it.glyph === 'broom'" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M14 4 L20 10 L17 13 L11 7 Z" fill="currentColor" opacity="0.2" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
          <path d="M11 7 L4 14 V20 H10 L17 13" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
          <path d="M5 17 L8 20 M7 15 L11 19" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
        </svg>
        <svg v-else-if="it.glyph === 'hash'" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M9 4 L7 20 M17 4 L15 20 M4 9 H20 M3 15 H19" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
        </svg>
        <svg v-else viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="6" cy="6" r="2.5" fill="currentColor" />
          <circle cx="18" cy="6" r="2.5" fill="currentColor" opacity="0.5" />
          <circle cx="12" cy="18" r="2.5" fill="currentColor" />
          <circle cx="18" cy="18" r="2.5" fill="currentColor" opacity="0.5" />
          <path d="M6 8 L12 16 M6 8 L18 8 M18 8 L18 16 M12 18 L18 18" stroke="currentColor" stroke-width="1.4" fill="none" />
        </svg>
      </div>
      <div class="pp-body">
        <div class="pp-title">{{ it.title }}</div>
        <div class="pp-text">{{ it.body }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.pp {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 0.85rem;
  margin: 1.5rem 0 2rem;
}

@media (max-width: 720px) { .pp { grid-template-columns: 1fr; } }

.pp-card {
  display: flex;
  align-items: flex-start;
  gap: 0.85rem;
  padding: 1rem;
  border-radius: 12px;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
  transition: border-color 0.25s ease, transform 0.25s ease;
  animation: pp-in 0.5s cubic-bezier(0.4, 0, 0.2, 1) backwards;
  animation-delay: calc(var(--i) * 50ms);
}

.pp-card:hover {
  border-color: rgba(210, 105, 30, 0.4);
  transform: translateY(-2px);
}

.pp-glyph {
  flex-shrink: 0;
  width: 38px;
  height: 38px;
  padding: 7px;
  border-radius: 9px;
  background: linear-gradient(135deg, rgba(210, 105, 30, 0.12), rgba(183, 65, 14, 0.04));
  border: 1px solid rgba(210, 105, 30, 0.22);
  color: #d2691e;
  display: grid;
  place-items: center;
}

.pp-glyph svg { width: 100%; height: 100%; }

.pp-title {
  font-weight: 600;
  font-size: 0.95rem;
  color: var(--vp-c-text-1);
  margin-bottom: 0.15rem;
  letter-spacing: -0.005em;
}

.pp-text {
  font-size: 0.82rem;
  color: var(--vp-c-text-2);
  line-height: 1.55;
}

@keyframes pp-in {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: none; }
}

@media (prefers-reduced-motion: reduce) {
  .pp-card { animation: none; }
  .pp-card:hover { transform: none; }
}
</style>
