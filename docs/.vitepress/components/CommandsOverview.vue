<script setup>
const COMMANDS = [
  { id: "publish", name: "smuggle publish", blurb: "Pack and register local packages.", blocking: false, anchor: "smuggle-publish" },
  { id: "install", name: "smuggle install", aka: "smuggle", blurb: "Swap registered packages into node_modules.", blocking: true, anchor: "smuggle-install" },
  { id: "add", name: "smuggle add <pkg>", blurb: "Add an unreleased package and swap it in.", blocking: true, anchor: "smuggle-add" },
  { id: "dev", name: "smuggle dev", blurb: "Swap packages and run your dev server.", blocking: true, anchor: "smuggle-dev" },
  { id: "list", name: "smuggle list", blurb: "List all registered packages.", blocking: false, anchor: "smuggle-list" },
  { id: "unpublish", name: "smuggle unpublish", blurb: "Remove a registered package.", blocking: false, anchor: "smuggle-unpublish" },
];
</script>

<template>
  <div class="co">
    <a
      v-for="c in COMMANDS"
      :key="c.id"
      :href="`#${c.anchor}`"
      class="co-card"
    >
      <div class="co-head">
        <code class="co-name">{{ c.name }}</code>
        <span :class="['co-badge', c.blocking ? 'co-blocks' : 'co-quick']">
          <span class="co-badge-dot" />
          {{ c.blocking ? "blocks" : "quick" }}
        </span>
      </div>
      <div v-if="c.aka" class="co-aka">also <code>{{ c.aka }}</code></div>
      <div class="co-blurb">{{ c.blurb }}</div>
      <div class="co-arrow" aria-hidden="true">
        <svg viewBox="0 0 16 16"><path d="M5 4 L11 8 L5 12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </div>
    </a>
  </div>
</template>

<style scoped>
.co {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 0.8rem;
  margin: 1rem 0 2rem;
}

.co-card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.95rem 1.05rem;
  border-radius: 12px;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
  text-decoration: none;
  color: var(--vp-c-text-1);
  transition: all 0.25s ease;
  overflow: hidden;
}

.co-card:hover {
  border-color: rgba(210, 105, 30, 0.45);
  transform: translateY(-2px);
  box-shadow: 0 0 0 3px rgba(210, 105, 30, 0.05), 0 12px 30px -12px rgba(183, 65, 14, 0.3);
}

.co-card::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: linear-gradient(180deg, #b7410e, #d2691e);
  opacity: 0;
  transition: opacity 0.25s ease;
}

.co-card:hover::before { opacity: 1; }

.co-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.co-name {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.92rem;
  font-weight: 600;
  color: var(--vp-c-text-1);
  background: transparent;
  padding: 0;
  letter-spacing: -0.005em;
}

.co-badge {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.18rem 0.5rem;
  font-size: 0.65rem;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  font-weight: 700;
  font-family: ui-monospace, monospace;
  border-radius: 999px;
  margin-left: auto;
}

.co-blocks {
  color: #c87a40;
  background: rgba(210, 105, 30, 0.1);
  border: 1px solid rgba(210, 105, 30, 0.3);
}

.co-quick {
  color: #4a8;
  background: rgba(127, 220, 126, 0.08);
  border: 1px solid rgba(127, 220, 126, 0.3);
}

.dark .co-blocks { color: #e8a070; }
.dark .co-quick { color: #a8e8a8; }

.co-badge-dot {
  width: 5px;
  height: 5px;
  border-radius: 999px;
  background: currentColor;
}

.co-blocks .co-badge-dot { box-shadow: 0 0 6px currentColor; }

.co-aka {
  font-size: 0.78rem;
  color: var(--vp-c-text-3);
}

.co-aka code {
  font-size: 0.85em;
  padding: 0.05rem 0.3rem;
  background: var(--vp-c-bg);
  border-radius: 3px;
}

.co-blurb {
  font-size: 0.85rem;
  color: var(--vp-c-text-2);
  line-height: 1.5;
}

.co-arrow {
  position: absolute;
  right: 0.85rem;
  bottom: 0.85rem;
  width: 18px;
  height: 18px;
  color: var(--vp-c-text-3);
  opacity: 0;
  transform: translateX(-4px);
  transition: all 0.25s ease;
}

.co-card:hover .co-arrow {
  opacity: 1;
  transform: none;
  color: #d2691e;
}
</style>
