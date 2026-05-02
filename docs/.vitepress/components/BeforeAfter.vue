<script setup>
import { ref } from "vue";

const TOOLS = [
  {
    id: "npm-link",
    label: "npm link",
    rows: [
      {
        file: "node_modules/my-pkg",
        bad: [
          { kind: "del", text: "real folder" },
          { kind: "add", text: "→ ../../my-pkg  (symlink)" },
        ],
        badNote: "Bundlers and resolvers behave differently",
      },
      {
        file: "package.json",
        bad: [{ kind: "ctx", text: '"my-pkg": "^1.0.0"' }],
        badNote: "Range stays — but resolves to a symlink, not a release",
      },
      {
        file: "pnpm-lock.yaml / package-lock.json",
        bad: [{ kind: "ctx", text: "(usually unchanged)" }],
        badNote: "Easy to commit a symlinked install by accident",
      },
    ],
  },
  {
    id: "file-ref",
    label: 'file: reference',
    rows: [
      {
        file: "node_modules/my-pkg",
        bad: [{ kind: "ctx", text: "real folder" }],
        badNote: "OK — but everything below leaks into source control",
      },
      {
        file: "package.json",
        bad: [
          { kind: "del", text: '"my-pkg": "^1.0.0"' },
          { kind: "add", text: '"my-pkg": "file:../my-pkg"' },
        ],
        badNote: "Local path committed to repo — breaks for everyone else",
      },
      {
        file: "pnpm-lock.yaml / package-lock.json",
        bad: [
          { kind: "add", text: "+ my-pkg: link:../my-pkg" },
          { kind: "add", text: "+ specifiers, integrity drift" },
        ],
        badNote: "Lockfile diff every time you sync",
      },
    ],
  },
  {
    id: "yalc",
    label: "yalc",
    rows: [
      {
        file: "node_modules/my-pkg",
        bad: [{ kind: "ctx", text: "real folder" }],
        badNote: "OK — but yalc still leaves traces elsewhere",
      },
      {
        file: "package.json",
        bad: [
          { kind: "del", text: '"my-pkg": "^1.0.0"' },
          { kind: "add", text: '"my-pkg": "file:.yalc/my-pkg"' },
        ],
        badNote: "Modified — easy to commit by mistake",
      },
      {
        file: ".yalc/  +  yalc.lock",
        bad: [{ kind: "add", text: "tracked in your repo" }],
        badNote: "Manual cleanup with yalc remove --all",
      },
    ],
  },
];

const after = [
  {
    file: "node_modules/my-pkg",
    good: [{ kind: "ok", text: "real folder, real files" }],
    goodNote: "Bundlers, Vite, Node — all see a normal install",
  },
  {
    file: "package.json",
    good: [{ kind: "ctx", text: '"my-pkg": "^1.0.0"' }],
    goodNote: "Untouched. Same diff as if you ran nothing.",
  },
  {
    file: "pnpm-lock.yaml / package-lock.json",
    good: [{ kind: "ctx", text: "(unchanged)" }],
    goodNote: "No churn. Nothing to revert before committing.",
  },
];

const active = ref(TOOLS[0].id);
function pick(id) { active.value = id; }
const tool = () => TOOLS.find((t) => t.id === active.value);
</script>

<template>
  <div class="ba">
    <div class="ba-tabs">
      <span class="ba-tabs-label">Compare against:</span>
      <button
        v-for="t in TOOLS"
        :key="t.id"
        :class="['ba-tab', { active: active === t.id }]"
        @click="pick(t.id)"
      >
        {{ t.label }}
      </button>
    </div>

    <div class="ba-grid">
      <!-- BEFORE -->
      <div class="ba-panel ba-bad">
        <div class="ba-panel-head">
          <span class="ba-panel-tag">before</span>
          <span class="ba-panel-title">{{ tool().label }}</span>
          <span class="ba-panel-glyph ba-bad-glyph" aria-hidden="true">
            <svg viewBox="0 0 16 16"><path d="M4 4 L12 12 M12 4 L4 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg>
          </span>
        </div>
        <div class="ba-rows">
          <div v-for="row in tool().rows" :key="row.file" class="ba-row">
            <div class="ba-row-file">{{ row.file }}</div>
            <div class="ba-row-diff">
              <div
                v-for="(d, i) in row.bad"
                :key="i"
                :class="['ba-diff-line', `ba-${d.kind}`]"
              >
                <span class="ba-diff-mark">{{ d.kind === "add" ? "+" : d.kind === "del" ? "-" : " " }}</span>
                <span>{{ d.text }}</span>
              </div>
            </div>
            <div class="ba-row-note">{{ row.badNote }}</div>
          </div>
        </div>
      </div>

      <div class="ba-arrow" aria-hidden="true">
        <svg viewBox="0 0 32 32">
          <defs>
            <linearGradient id="ba-arrow-grad" x1="0" y1="0" x2="1" y2="0">
              <stop offset="0" stop-color="#b7410e" />
              <stop offset="1" stop-color="#d2691e" />
            </linearGradient>
          </defs>
          <path d="M4 16 L24 16 M18 9 L25 16 L18 23" fill="none" stroke="url(#ba-arrow-grad)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </div>

      <!-- AFTER -->
      <div class="ba-panel ba-good">
        <div class="ba-panel-head">
          <span class="ba-panel-tag">after</span>
          <span class="ba-panel-title">smuggle install</span>
          <span class="ba-panel-glyph ba-good-glyph" aria-hidden="true">
            <svg viewBox="0 0 16 16"><path d="M3 8 L7 12 L13 4" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </span>
        </div>
        <div class="ba-rows">
          <div v-for="row in after" :key="row.file" class="ba-row">
            <div class="ba-row-file">{{ row.file }}</div>
            <div class="ba-row-diff">
              <div
                v-for="(d, i) in row.good"
                :key="i"
                :class="['ba-diff-line', `ba-${d.kind}`]"
              >
                <span class="ba-diff-mark">{{ d.kind === "ok" ? "✓" : " " }}</span>
                <span>{{ d.text }}</span>
              </div>
            </div>
            <div class="ba-row-note">{{ row.goodNote }}</div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ba {
  margin: 1.5rem 0 2rem;
}

.ba-tabs {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
  margin-bottom: 1rem;
  font-size: 0.85rem;
}

.ba-tabs-label {
  color: var(--vp-c-text-2);
  margin-right: 0.3rem;
}

.ba-tab {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.78rem;
  padding: 0.34rem 0.75rem;
  border-radius: 999px;
  background: var(--vp-c-bg);
  border: 1px solid var(--vp-c-divider);
  color: var(--vp-c-text-2);
  cursor: pointer;
  transition: all 0.25s ease;
}

.ba-tab:hover { border-color: var(--vp-c-text-3); color: var(--vp-c-text-1); }

.ba-tab.active {
  background: linear-gradient(135deg, rgba(183, 65, 14, 0.14), rgba(210, 105, 30, 0.06));
  border-color: rgba(210, 105, 30, 0.45);
  color: var(--vp-c-text-1);
  box-shadow: 0 0 0 3px rgba(210, 105, 30, 0.06);
}

.ba-grid {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  gap: 0.75rem;
  align-items: stretch;
}

@media (max-width: 760px) {
  .ba-grid { grid-template-columns: 1fr; }
  .ba-arrow { transform: rotate(90deg); margin: 0.25rem auto; }
}

.ba-panel {
  position: relative;
  border-radius: 12px;
  overflow: hidden;
  background: var(--vp-c-bg);
  border: 1px solid var(--vp-c-divider);
  display: flex;
  flex-direction: column;
}

.ba-bad {
  border-color: rgba(220, 80, 80, 0.28);
  background: linear-gradient(180deg, rgba(220, 80, 80, 0.04) 0%, var(--vp-c-bg) 60%);
}

.ba-good {
  border-color: rgba(127, 220, 126, 0.28);
  background: linear-gradient(180deg, rgba(127, 220, 126, 0.04) 0%, var(--vp-c-bg) 60%);
}

.dark .ba-bad {
  background: linear-gradient(180deg, rgba(220, 80, 80, 0.06) 0%, rgba(26, 19, 16, 0.6) 60%);
}
.dark .ba-good {
  background: linear-gradient(180deg, rgba(127, 220, 126, 0.05) 0%, rgba(20, 24, 20, 0.6) 60%);
}

.ba-panel-head {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.7rem 0.95rem;
  border-bottom: 1px solid var(--vp-c-divider);
  background: linear-gradient(180deg, var(--vp-c-bg-soft), transparent);
}

.ba-panel-tag {
  font-family: ui-monospace, monospace;
  font-size: 0.6rem;
  font-weight: 700;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  padding: 0.15rem 0.5rem;
  border-radius: 4px;
}

.ba-bad .ba-panel-tag { color: #c44; background: rgba(220, 80, 80, 0.1); border: 1px solid rgba(220, 80, 80, 0.3); }
.ba-good .ba-panel-tag { color: #4a8; background: rgba(127, 220, 126, 0.1); border: 1px solid rgba(127, 220, 126, 0.3); }

.dark .ba-bad .ba-panel-tag { color: #ff8888; }
.dark .ba-good .ba-panel-tag { color: #a8e8a8; }

.ba-panel-title {
  font-family: ui-monospace, monospace;
  font-weight: 600;
  font-size: 0.88rem;
  color: var(--vp-c-text-1);
}

.ba-panel-glyph {
  margin-left: auto;
  width: 22px;
  height: 22px;
  display: grid;
  place-items: center;
  border-radius: 999px;
}

.ba-bad-glyph { background: rgba(220, 80, 80, 0.12); color: #c44; }
.ba-good-glyph { background: rgba(127, 220, 126, 0.12); color: #4a8; }
.dark .ba-bad-glyph { color: #ff8888; }
.dark .ba-good-glyph { color: #a8e8a8; }

.ba-panel-glyph svg { width: 14px; height: 14px; }

.ba-rows {
  padding: 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  flex: 1;
}

.ba-row {
  padding: 0.65rem 0.75rem;
  border-radius: 8px;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
}

.dark .ba-row { background: rgba(255, 255, 255, 0.02); }

.ba-row-file {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.78rem;
  color: var(--vp-c-text-2);
  margin-bottom: 0.4rem;
  letter-spacing: -0.01em;
}

.ba-row-diff {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.8rem;
  line-height: 1.55;
  display: flex;
  flex-direction: column;
}

.ba-diff-line {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.15rem 0.4rem;
  border-radius: 4px;
}

.ba-diff-mark {
  width: 0.9rem;
  text-align: center;
  font-weight: 700;
  flex-shrink: 0;
}

.ba-add {
  background: rgba(220, 80, 80, 0.08);
  color: #d44;
}
.ba-add .ba-diff-mark { color: #d44; }
.ba-del {
  background: rgba(220, 80, 80, 0.05);
  color: var(--vp-c-text-2);
  text-decoration: line-through;
  text-decoration-color: rgba(220, 80, 80, 0.5);
}
.ba-ctx { color: var(--vp-c-text-2); }
.ba-ok {
  background: rgba(127, 220, 126, 0.08);
  color: #4a8;
}
.ba-ok .ba-diff-mark { color: #4a8; font-weight: 800; }

.dark .ba-add { color: #ff9999; }
.dark .ba-ok { color: #a8e8a8; }

.ba-row-note {
  margin-top: 0.45rem;
  font-size: 0.78rem;
  color: var(--vp-c-text-3);
  line-height: 1.5;
}

.ba-arrow {
  display: grid;
  place-items: center;
  width: 44px;
}

.ba-arrow svg { width: 32px; height: 32px; }
</style>
