<script setup>
import { ref, computed } from "vue";

const TABS = [
  { id: "npm", label: "npm", icon: "npm", cmd: "npm install -g smuggle-cli" },
  { id: "pnpm", label: "pnpm", icon: "pnpm", cmd: "pnpm add -g smuggle-cli" },
  { id: "yarn", label: "yarn", icon: "yarn", cmd: "yarn global add smuggle-cli" },
  { id: "bun", label: "bun", icon: "bun", cmd: "bun add -g smuggle-cli" },
  { id: "brew", label: "Homebrew", icon: "brew", cmd: "brew install logaretm/tap/smuggle" },
  { id: "cargo", label: "Cargo", icon: "cargo", cmd: "cargo install smuggle" },
  { id: "source", label: "From source", icon: "source", cmd: "git clone https://github.com/logaretm/smuggle.git\ncd smuggle\ncargo install --path ." },
];

const active = ref("npm");
const copied = ref(false);

const current = computed(() => TABS.find((t) => t.id === active.value));

async function copy() {
  try {
    await navigator.clipboard.writeText(current.value.cmd);
    copied.value = true;
    setTimeout(() => (copied.value = false), 1400);
  } catch {}
}
</script>

<template>
  <div class="it">
    <div class="it-tabs" role="tablist">
      <button
        v-for="t in TABS"
        :key="t.id"
        :class="['it-tab', { active: active === t.id }]"
        :aria-selected="active === t.id"
        role="tab"
        @click="active = t.id"
      >
        <span class="it-tab-icon" aria-hidden="true">
          <svg v-if="t.icon === 'npm'" viewBox="0 0 16 16"><rect x="1" y="3" width="14" height="10" fill="currentColor" /><path d="M3 5 H7 V11 H6 V6 H5 V11 H3 Z M8 5 H12 V11 H10 V10 H9 V11 H8 Z M10 6 H11 V9 H10 Z" fill="var(--vp-c-bg)" /></svg>
          <svg v-else-if="t.icon === 'pnpm'" viewBox="0 0 16 16"><rect x="1" y="1" width="4" height="4" fill="currentColor" /><rect x="6" y="1" width="4" height="4" fill="currentColor" /><rect x="11" y="1" width="4" height="4" fill="currentColor" /><rect x="6" y="6" width="4" height="4" fill="currentColor" /><rect x="11" y="6" width="4" height="4" fill="currentColor" opacity="0.6" /><rect x="11" y="11" width="4" height="4" fill="currentColor" opacity="0.4" /></svg>
          <svg v-else-if="t.icon === 'yarn'" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" stroke-width="1.4" /><path d="M5 5 Q8 8 11 5 M5 8 Q8 11 11 8" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
          <svg v-else-if="t.icon === 'bun'" viewBox="0 0 16 16"><ellipse cx="8" cy="9" rx="6.5" ry="5" fill="currentColor" /><circle cx="6" cy="8" r="0.9" fill="var(--vp-c-bg)" /><circle cx="10" cy="8" r="0.9" fill="var(--vp-c-bg)" /><path d="M6.5 11 Q8 12.5 9.5 11" fill="none" stroke="var(--vp-c-bg)" stroke-width="0.8" stroke-linecap="round" /></svg>
          <svg v-else-if="t.icon === 'brew'" viewBox="0 0 16 16"><path d="M3 12 V8 a5 5 0 0 1 10 0 v4 z" fill="currentColor" opacity="0.85" /><rect x="2" y="12" width="12" height="2" fill="currentColor" /><path d="M6 6 Q6 4 7 4 M9 6 Q9 4 10 4" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" /></svg>
          <svg v-else-if="t.icon === 'cargo'" viewBox="0 0 16 16"><rect x="2" y="5" width="12" height="9" rx="1" fill="currentColor" opacity="0.85" /><path d="M2 8 H14 M5 5 V14 M11 5 V14" stroke="var(--vp-c-bg)" stroke-width="0.8" /><path d="M6 4 L8 2 L10 4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" /></svg>
          <svg v-else viewBox="0 0 16 16"><path d="M5 4 L2 8 L5 12 M11 4 L14 8 L11 12 M9 3 L7 13" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </span>
        {{ t.label }}
      </button>
    </div>

    <div class="it-body">
      <pre class="it-code"><code><span class="it-prompt">$</span> {{ current.cmd }}</code></pre>
      <button class="it-copy" :class="{ ok: copied }" @click="copy" :aria-label="copied ? 'Copied' : 'Copy'">
        <svg v-if="!copied" viewBox="0 0 16 16" aria-hidden="true"><rect x="5" y="3" width="9" height="11" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.4" /><path d="M11 3 V2 a1 1 0 0 0 -1 -1 H3 a1 1 0 0 0 -1 1 v9 a1 1 0 0 0 1 1 h1" fill="none" stroke="currentColor" stroke-width="1.4" /></svg>
        <svg v-else viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8 L7 12 L13 4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
        <span class="it-copy-label">{{ copied ? "Copied" : "Copy" }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.it {
  position: relative;
  margin: 1rem 0 1.5rem;
  border-radius: 12px;
  overflow: hidden;
  border: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg-soft);
}

.it-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 0;
  background: linear-gradient(180deg, var(--vp-c-bg) 0%, var(--vp-c-bg-soft) 100%);
  border-bottom: 1px solid var(--vp-c-divider);
  padding: 0.4rem 0.4rem 0;
}

.it-tab {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.5rem 0.85rem;
  margin-right: 0.15rem;
  font-size: 0.82rem;
  font-weight: 500;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--vp-c-text-2);
  cursor: pointer;
  position: relative;
  transition: color 0.2s ease, border-color 0.2s ease;
}

.it-tab:hover { color: var(--vp-c-text-1); }

.it-tab.active {
  color: var(--vp-c-text-1);
  border-bottom-color: #d2691e;
}

.it-tab-icon {
  display: inline-grid;
  place-items: center;
  width: 16px;
  height: 16px;
  color: var(--vp-c-text-3);
}

.it-tab.active .it-tab-icon { color: #d2691e; }

.it-body {
  position: relative;
  padding: 0;
}

.it-code {
  margin: 0;
  padding: 1rem 1.1rem;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.86rem;
  line-height: 1.7;
  color: var(--vp-c-text-1);
  background: transparent;
  white-space: pre;
  overflow-x: auto;
}

.it-code code { background: transparent; padding: 0; font-size: inherit; }

.it-prompt {
  color: #d2691e;
  font-weight: 700;
  margin-right: 0.4rem;
  user-select: none;
}

.it-copy {
  position: absolute;
  top: 0.7rem;
  right: 0.7rem;
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.35rem 0.65rem;
  font-size: 0.72rem;
  font-family: ui-monospace, monospace;
  border-radius: 6px;
  background: var(--vp-c-bg);
  border: 1px solid var(--vp-c-divider);
  color: var(--vp-c-text-2);
  cursor: pointer;
  transition: all 0.2s ease;
}

.it-copy:hover { color: var(--vp-c-text-1); border-color: rgba(210, 105, 30, 0.4); }

.it-copy.ok {
  color: #4a8;
  border-color: rgba(127, 220, 126, 0.5);
  background: rgba(127, 220, 126, 0.08);
}

.dark .it-copy.ok { color: #a8e8a8; }

.it-copy svg { width: 12px; height: 12px; }

@media (max-width: 540px) {
  .it-copy-label { display: none; }
  .it-copy { padding: 0.4rem; }
}
</style>
