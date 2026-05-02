<script setup>
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from "vue";

const PHASES = [
  {
    id: "pack",
    track: "publish",
    station: "source",
    edge: "src-vault",
    label: "Packing source",
    detail: "Runs npm pack on your local package.",
    term: [
      { kind: "cmd", text: "smuggle publish" },
      { kind: "info", text: "→ npm pack ./my-package" },
    ],
  },
  {
    id: "store",
    track: "publish",
    station: "vault",
    edge: null,
    label: "Storing in vault",
    detail: "Tarball lands in ~/.smuggle, ready for install.",
    term: [
      { kind: "cmd", text: "smuggle publish" },
      { kind: "info", text: "→ npm pack ./my-package" },
      { kind: "ok", text: "✓ stored my-package@1.0.0" },
    ],
  },
  {
    id: "scan",
    track: "install",
    station: "target",
    edge: null,
    label: "Scanning dependencies",
    detail: "Reads package.json — finds smuggled matches.",
    term: [
      { kind: "cmd", text: "smuggle install" },
      { kind: "muted", text: "scanning package.json…" },
      { kind: "info", text: "match: my-package" },
    ],
  },
  {
    id: "backup",
    track: "install",
    station: "target",
    edge: null,
    label: "Backing up originals",
    detail: "Saves the real node_modules entries aside.",
    term: [
      { kind: "cmd", text: "smuggle install" },
      { kind: "info", text: "match: my-package" },
      { kind: "muted", text: "→ backing up node_modules/my-package" },
    ],
  },
  {
    id: "swap",
    track: "install",
    station: "target",
    edge: "vault-tgt",
    label: "Swapping in tarball",
    detail: "Smuggled package replaces the original. Real files.",
    term: [
      { kind: "cmd", text: "smuggle install" },
      { kind: "muted", text: "→ extracting my-package@1.0.0" },
      { kind: "ok", text: "✓ swapped my-package" },
    ],
  },
  {
    id: "watch",
    track: "install",
    station: "target",
    edge: "vault-tgt",
    label: "Watching for changes",
    detail: "Re-pack and re-swap on every save.",
    term: [
      { kind: "ok", text: "✓ swapped my-package" },
      { kind: "muted", text: "watching ~/projects/my-package…" },
      { kind: "info", text: "↻ src/index.ts changed → re-swap" },
    ],
  },
  {
    id: "restore",
    track: "install",
    station: "target",
    edge: "tgt-vault",
    label: "Restoring on exit",
    detail: "Originals come back — even on Ctrl-C.",
    term: [
      { kind: "muted", text: "watching…" },
      { kind: "warn", text: "^C  caught SIGINT" },
      { kind: "ok", text: "✓ restored originals" },
    ],
  },
];

const phaseIndex = ref(0);
const isPaused = ref(false);
const reduceMotion = ref(false);
const container = ref(null);

const current = computed(() => PHASES[phaseIndex.value]);
const trackName = computed(() =>
  current.value.track === "publish" ? "smuggle publish" : "smuggle install",
);

let timer = null;
function tick() {
  if (isPaused.value || reduceMotion.value) return;
  phaseIndex.value = (phaseIndex.value + 1) % PHASES.length;
}

function go(i) {
  phaseIndex.value = i;
  isPaused.value = true;
}

onMounted(() => {
  if (typeof window !== "undefined") {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    reduceMotion.value = mq.matches;
    mq.addEventListener?.("change", (e) => (reduceMotion.value = e.matches));
  }
  timer = setInterval(tick, 2600);
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <div
    ref="container"
    class="smuggle-diagram"
    :class="[`phase-${current.id}`, `track-${current.track}`]"
    @mouseenter="isPaused = true"
    @mouseleave="isPaused = false"
  >
    <!-- Header: track + phase label -->
    <div class="diagram-head">
      <div class="track-tabs">
        <span class="track-tab" :class="{ active: current.track === 'publish' }">
          <span class="track-dot" /> smuggle publish
        </span>
        <span class="track-tab" :class="{ active: current.track === 'install' }">
          <span class="track-dot" /> smuggle install
        </span>
      </div>
      <div class="phase-label">
        <span class="phase-name">{{ current.label }}</span>
        <span class="phase-detail">{{ current.detail }}</span>
      </div>
    </div>

    <!-- Step tracker -->
    <div class="step-tracker" role="tablist" aria-label="Process steps">
      <button
        v-for="(p, i) in PHASES"
        :key="p.id"
        :class="[
          'step-pip',
          { active: i === phaseIndex, done: i < phaseIndex, [`track-${p.track}`]: true },
        ]"
        :aria-label="p.label"
        :aria-selected="i === phaseIndex"
        role="tab"
        @click="go(i)"
      >
        <span class="step-pip-dot" />
        <span class="step-pip-num">{{ i + 1 }}</span>
      </button>
      <span class="step-tracker-rail" />
      <span
        class="step-tracker-fill"
        :style="{ width: `${(phaseIndex / (PHASES.length - 1)) * 100}%` }"
      />
    </div>

    <!-- Stations with SVG connectors -->
    <div class="stage">
      <svg class="stage-bg" aria-hidden="true">
        <defs>
          <pattern
            id="sm-grid"
            width="24"
            height="24"
            patternUnits="userSpaceOnUse"
          >
            <path
              d="M 24 0 L 0 0 0 24"
              fill="none"
              stroke="currentColor"
              stroke-width="0.5"
              opacity="0.18"
            />
          </pattern>
          <radialGradient id="sm-glow" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stop-color="var(--sm-glow)" stop-opacity="0.45" />
            <stop offset="100%" stop-color="var(--sm-glow)" stop-opacity="0" />
          </radialGradient>
        </defs>
        <rect width="100%" height="100%" fill="url(#sm-grid)" />
        <ellipse cx="50%" cy="55%" rx="60%" ry="40%" fill="url(#sm-glow)" />
      </svg>

      <!-- Connector layer -->
      <svg class="connector-layer" viewBox="0 0 1000 360" preserveAspectRatio="none" aria-hidden="true">
        <defs>
          <linearGradient id="edge-grad-1" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stop-color="#d2691e" stop-opacity="0.15" />
            <stop offset="50%" stop-color="#d2691e" stop-opacity="0.55" />
            <stop offset="100%" stop-color="#d2691e" stop-opacity="0.15" />
          </linearGradient>
          <linearGradient id="edge-grad-2" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stop-color="#b7410e" stop-opacity="0.15" />
            <stop offset="50%" stop-color="#b7410e" stop-opacity="0.55" />
            <stop offset="100%" stop-color="#b7410e" stop-opacity="0.15" />
          </linearGradient>
          <filter id="edge-glow" x="-20%" y="-50%" width="140%" height="200%">
            <feGaussianBlur stdDeviation="3" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        <!-- Source → Vault -->
        <path
          class="edge-base edge-src-vault"
          d="M 200 180 C 290 180, 290 180, 380 180"
        />
        <path
          class="edge-active edge-src-vault"
          :class="{ on: current.edge === 'src-vault' }"
          d="M 200 180 C 290 180, 290 180, 380 180"
          filter="url(#edge-glow)"
        />

        <!-- Vault → Target -->
        <path
          class="edge-base edge-vault-tgt"
          d="M 620 180 C 710 180, 710 180, 800 180"
        />
        <path
          class="edge-active edge-vault-tgt"
          :class="{ on: current.edge === 'vault-tgt' }"
          d="M 620 180 C 710 180, 710 180, 800 180"
          filter="url(#edge-glow)"
        />

        <!-- Target → Vault (restore loop, drawn as arc above) -->
        <path
          class="edge-base edge-return"
          d="M 800 160 C 710 90, 710 90, 620 160"
        />
        <path
          class="edge-active edge-return"
          :class="{ on: current.edge === 'tgt-vault' }"
          d="M 800 160 C 710 90, 710 90, 620 160"
          filter="url(#edge-glow)"
        />

        <!-- Traveling tarball: src → vault -->
        <g
          class="parcel"
          :class="{ on: current.edge === 'src-vault' }"
          aria-hidden="true"
        >
          <animateMotion
            v-if="current.edge === 'src-vault'"
            dur="1.6s"
            repeatCount="indefinite"
            keyPoints="0;1"
            keyTimes="0;1"
            path="M 200 180 C 290 180, 290 180, 380 180"
            rotate="auto"
          />
          <circle r="10" class="parcel-glow" />
          <rect x="-7" y="-6" width="14" height="12" rx="2" class="parcel-body" />
          <path d="M -7 0 L 7 0 M 0 -6 L 0 6" class="parcel-tape" />
        </g>

        <!-- Traveling tarball: vault → target -->
        <g
          class="parcel"
          :class="{ on: current.edge === 'vault-tgt' }"
          aria-hidden="true"
        >
          <animateMotion
            v-if="current.edge === 'vault-tgt'"
            dur="1.6s"
            repeatCount="indefinite"
            keyPoints="0;1"
            keyTimes="0;1"
            path="M 620 180 C 710 180, 710 180, 800 180"
            rotate="auto"
          />
          <circle r="10" class="parcel-glow" />
          <rect x="-7" y="-6" width="14" height="12" rx="2" class="parcel-body" />
          <path d="M -7 0 L 7 0 M 0 -6 L 0 6" class="parcel-tape" />
        </g>

        <!-- Traveling parcel: target → vault (restore) -->
        <g
          class="parcel return"
          :class="{ on: current.edge === 'tgt-vault' }"
          aria-hidden="true"
        >
          <animateMotion
            v-if="current.edge === 'tgt-vault'"
            dur="1.6s"
            repeatCount="indefinite"
            keyPoints="0;1"
            keyTimes="0;1"
            path="M 800 160 C 710 90, 710 90, 620 160"
            rotate="auto"
          />
          <circle r="10" class="parcel-glow restore" />
          <rect x="-7" y="-6" width="14" height="12" rx="2" class="parcel-body restore" />
          <path d="M -7 0 L 7 0 M 0 -6 L 0 6" class="parcel-tape" />
        </g>
      </svg>

      <!-- Stations -->
      <div class="stations">
        <!-- SOURCE -->
        <div
          class="station"
          :class="{ active: current.station === 'source' }"
          data-station="source"
        >
          <div class="station-glow" />
          <div class="station-tag">source</div>
          <div class="station-icon">
            <svg viewBox="0 0 56 56" aria-hidden="true">
              <defs>
                <linearGradient id="src-face" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0" stop-color="#d2691e" />
                  <stop offset="1" stop-color="#8b4513" />
                </linearGradient>
              </defs>
              <path d="M8 18 L28 8 L48 18 L48 40 L28 50 L8 40 Z" fill="url(#src-face)" stroke="#6b3410" stroke-width="1" />
              <path d="M8 18 L28 28 L48 18" fill="none" stroke="#6b3410" stroke-width="1" />
              <path d="M28 28 L28 50" stroke="#6b3410" stroke-width="1" />
              <text x="28" y="22" text-anchor="middle" font-family="ui-monospace, monospace" font-size="8" font-weight="700" fill="#fff5e6">{ }</text>
            </svg>
          </div>
          <div class="station-title">your package</div>
          <div class="station-path">./my-package</div>
          <ul class="station-files">
            <li><span class="dot" /> package.json</li>
            <li><span class="dot" /> src/index.ts</li>
            <li><span class="dot" /> dist/</li>
          </ul>
        </div>

        <!-- VAULT -->
        <div
          class="station vault"
          :class="{ active: current.station === 'vault' }"
          data-station="vault"
        >
          <div class="station-glow" />
          <div class="station-tag">vault</div>
          <div class="station-icon">
            <svg viewBox="0 0 56 56" aria-hidden="true">
              <defs>
                <linearGradient id="vault-face" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0" stop-color="#cd853f" />
                  <stop offset="1" stop-color="#6b3410" />
                </linearGradient>
              </defs>
              <rect x="8" y="14" width="40" height="32" rx="3" fill="url(#vault-face)" stroke="#6b3410" />
              <rect x="12" y="18" width="14" height="10" rx="1" fill="#f5deb3" stroke="#8b4513" stroke-width="0.6" />
              <rect x="30" y="18" width="14" height="10" rx="1" fill="#f5deb3" stroke="#8b4513" stroke-width="0.6" />
              <rect x="12" y="32" width="14" height="10" rx="1" fill="#f5deb3" stroke="#8b4513" stroke-width="0.6" />
              <rect x="30" y="32" width="14" height="10" rx="1" fill="#f5deb3" stroke="#8b4513" stroke-width="0.6" />
              <circle cx="19" cy="23" r="1.4" fill="#b7410e" />
              <circle cx="37" cy="23" r="1.4" fill="#b7410e" />
              <circle cx="19" cy="37" r="1.4" fill="#b7410e" />
              <circle cx="37" cy="37" r="1.4" fill="#b7410e" />
            </svg>
          </div>
          <div class="station-title">smuggle store</div>
          <div class="station-path">~/.smuggle/</div>
          <ul class="station-files">
            <li><span class="dot" /> my-package@1.0.0.tgz</li>
            <li><span class="dot" /> @co/ui@2.3.1.tgz</li>
            <li class="muted"><span class="dot" /> …</li>
          </ul>
        </div>

        <!-- TARGET -->
        <div
          class="station"
          :class="{ active: current.station === 'target' }"
          data-station="target"
        >
          <div class="station-glow" />
          <div class="station-tag">target</div>
          <div class="station-icon">
            <svg viewBox="0 0 56 56" aria-hidden="true">
              <defs>
                <linearGradient id="tgt-face" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0" stop-color="#b7410e" />
                  <stop offset="1" stop-color="#6b3410" />
                </linearGradient>
              </defs>
              <path d="M6 18 L24 18 L28 14 L50 14 L50 46 L6 46 Z" fill="url(#tgt-face)" stroke="#6b3410" />
              <rect x="11" y="24" width="34" height="3" rx="1" fill="#f5deb3" opacity="0.85" />
              <rect x="11" y="30" width="26" height="3" rx="1" fill="#f5deb3" opacity="0.6" />
              <rect x="11" y="36" width="20" height="3" rx="1" fill="#f5deb3" opacity="0.45" />
            </svg>
          </div>
          <div class="station-title">your project</div>
          <div class="station-path">node_modules/</div>
          <ul class="station-files">
            <li
              :class="{
                hot:
                  current.id === 'swap' ||
                  current.id === 'watch' ||
                  current.id === 'backup',
              }"
            >
              <span class="dot hot" /> my-package
              <span class="badge swap" v-if="current.id === 'swap' || current.id === 'watch'">smuggled</span>
              <span class="badge restore" v-if="current.id === 'restore'">restored</span>
            </li>
            <li><span class="dot" /> react</li>
            <li><span class="dot" /> typescript</li>
          </ul>
        </div>
      </div>
    </div>

    <!-- Terminal -->
    <div class="terminal">
      <div class="terminal-bar">
        <span class="tb-dot tb-r" />
        <span class="tb-dot tb-y" />
        <span class="tb-dot tb-g" />
        <span class="tb-title">{{ trackName }}</span>
      </div>
      <div class="terminal-body">
        <transition-group name="term" tag="div" class="term-lines">
          <div
            v-for="(line, idx) in current.term"
            :key="`${current.id}-${idx}`"
            :class="['term-line', `term-${line.kind}`]"
          >
            <span v-if="line.kind === 'cmd'" class="term-prompt">$</span>
            <span class="term-text">{{ line.text }}</span>
            <span
              v-if="idx === current.term.length - 1 && current.id === 'watch'"
              class="term-cursor"
            />
          </div>
        </transition-group>
      </div>
    </div>
  </div>
</template>

<style scoped>
.smuggle-diagram {
  --sm-rust-1: #b7410e;
  --sm-rust-2: #d2691e;
  --sm-rust-3: #8b4513;
  --sm-bronze: #cd853f;
  --sm-cream: #f5deb3;
  --sm-glow: rgba(210, 105, 30, 0.55);

  position: relative;
  margin: 2.5rem auto 1rem;
  max-width: 880px;
  padding: 1.25rem;
  border-radius: 18px;
  background:
    radial-gradient(120% 90% at 50% 0%, rgba(210, 105, 30, 0.08), transparent 60%),
    linear-gradient(180deg, var(--vp-c-bg-soft) 0%, var(--vp-c-bg) 100%);
  border: 1px solid var(--vp-c-divider);
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.04) inset,
    0 30px 60px -30px rgba(139, 69, 19, 0.18);
  color: var(--vp-c-text-1);
  overflow: hidden;
  isolation: isolate;
}

.dark .smuggle-diagram {
  background:
    radial-gradient(120% 90% at 50% 0%, rgba(210, 105, 30, 0.14), transparent 60%),
    linear-gradient(180deg, #1a1410 0%, #110e0c 100%);
  border-color: rgba(210, 105, 30, 0.18);
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.04) inset,
    0 40px 80px -30px rgba(0, 0, 0, 0.6);
}

/* Header */
.diagram-head {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 0.85rem;
}

.track-tabs {
  display: flex;
  gap: 0.4rem;
  flex-wrap: wrap;
}

.track-tab {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.32rem 0.7rem;
  border-radius: 999px;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.75rem;
  letter-spacing: 0.02em;
  background: var(--vp-c-bg);
  border: 1px solid var(--vp-c-divider);
  color: var(--vp-c-text-2);
  transition: all 0.3s ease;
}

.track-tab.active {
  background: linear-gradient(135deg, rgba(183, 65, 14, 0.12), rgba(210, 105, 30, 0.06));
  border-color: rgba(210, 105, 30, 0.4);
  color: var(--vp-c-text-1);
  box-shadow: 0 0 0 3px rgba(210, 105, 30, 0.06);
}

.track-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: var(--vp-c-text-3);
  transition: all 0.3s ease;
}

.track-tab.active .track-dot {
  background: var(--sm-rust-2);
  box-shadow: 0 0 0 3px rgba(210, 105, 30, 0.25), 0 0 8px var(--sm-rust-2);
}

.phase-label {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  text-align: right;
  min-width: 0;
  flex: 1;
}

.phase-name {
  font-weight: 600;
  font-size: 0.95rem;
  color: var(--vp-c-text-1);
}

.phase-detail {
  font-size: 0.78rem;
  color: var(--vp-c-text-2);
  line-height: 1.4;
}

/* Step tracker */
.step-tracker {
  position: relative;
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 22px;
  margin: 0.4rem 0.25rem 1.25rem;
  padding: 0;
}

.step-tracker-rail,
.step-tracker-fill {
  position: absolute;
  top: 50%;
  left: 11px;
  right: 11px;
  height: 2px;
  transform: translateY(-50%);
  border-radius: 2px;
  pointer-events: none;
}

.step-tracker-rail {
  background: var(--vp-c-divider);
}

.step-tracker-fill {
  left: 11px;
  right: auto;
  background: linear-gradient(90deg, var(--sm-rust-1), var(--sm-rust-2));
  box-shadow: 0 0 8px rgba(210, 105, 30, 0.5);
  transition: width 0.6s cubic-bezier(0.4, 0, 0.2, 1);
}

.step-pip {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  background: transparent;
  cursor: pointer;
  z-index: 1;
}

.step-pip-dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  background: var(--vp-c-bg);
  border: 2px solid var(--vp-c-divider);
  transition: all 0.3s ease;
}

.step-pip.done .step-pip-dot {
  background: var(--sm-rust-2);
  border-color: var(--sm-rust-2);
}

.step-pip.active .step-pip-dot {
  background: var(--sm-rust-2);
  border-color: var(--sm-rust-2);
  transform: scale(1.4);
  box-shadow:
    0 0 0 4px rgba(210, 105, 30, 0.15),
    0 0 16px var(--sm-rust-2);
  animation: pip-pulse 1.6s ease-in-out infinite;
}

.step-pip-num {
  position: absolute;
  top: 110%;
  left: 50%;
  transform: translateX(-50%);
  font-size: 0.65rem;
  font-family: ui-monospace, monospace;
  color: var(--vp-c-text-3);
  opacity: 0;
  transition: opacity 0.2s ease;
}

.step-pip.active .step-pip-num,
.step-pip:hover .step-pip-num {
  opacity: 1;
  color: var(--sm-rust-2);
}

@keyframes pip-pulse {
  0%, 100% { box-shadow: 0 0 0 4px rgba(210, 105, 30, 0.15), 0 0 16px var(--sm-rust-2); }
  50% { box-shadow: 0 0 0 8px rgba(210, 105, 30, 0.05), 0 0 24px var(--sm-rust-2); }
}

/* Stage */
.stage {
  position: relative;
  margin: 0 -0.25rem;
  padding: 1rem 0.25rem 0.5rem;
  min-height: 280px;
}

.stage-bg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  color: var(--sm-rust-3);
  opacity: 0.45;
  pointer-events: none;
  z-index: 0;
}

.dark .stage-bg {
  opacity: 0.6;
}

.connector-layer {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 1;
  overflow: visible;
}

.edge-base {
  fill: none;
  stroke: var(--vp-c-divider);
  stroke-width: 2;
  stroke-dasharray: 4 6;
  opacity: 0.6;
}

.edge-active {
  fill: none;
  stroke-width: 3;
  stroke-linecap: round;
  opacity: 0;
  transition: opacity 0.45s ease;
}

.edge-active.on {
  opacity: 1;
  stroke: url(#edge-grad-2);
  stroke-dasharray: 8 8;
  animation: edge-flow 1.2s linear infinite;
}

.edge-active.edge-src-vault.on {
  stroke: url(#edge-grad-1);
}

@keyframes edge-flow {
  to { stroke-dashoffset: -32; }
}

/* Parcel (traveling tarball) */
.parcel {
  opacity: 0;
  transition: opacity 0.3s ease;
}

.parcel.on { opacity: 1; }

.parcel-body {
  fill: var(--sm-cream);
  stroke: var(--sm-rust-1);
  stroke-width: 1.2;
  filter: drop-shadow(0 0 6px rgba(210, 105, 30, 0.6));
}

.parcel-body.restore {
  fill: var(--sm-cream);
  stroke: var(--sm-bronze);
}

.parcel-tape {
  fill: none;
  stroke: var(--sm-rust-1);
  stroke-width: 1.4;
  stroke-linecap: round;
}

.parcel-glow {
  fill: var(--sm-rust-2);
  opacity: 0.3;
  filter: blur(4px);
}

.parcel-glow.restore {
  fill: var(--sm-bronze);
}

/* Stations */
.stations {
  position: relative;
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: 1.25rem;
  z-index: 2;
}

.station {
  position: relative;
  padding: 1rem 0.85rem 0.85rem;
  border-radius: 14px;
  background: linear-gradient(180deg, var(--vp-c-bg) 0%, var(--vp-c-bg-soft) 100%);
  border: 1px solid var(--vp-c-divider);
  backdrop-filter: blur(6px);
  transition: transform 0.4s cubic-bezier(0.4, 0, 0.2, 1), border-color 0.3s, box-shadow 0.3s;
  z-index: 1;
}

.dark .station {
  background: linear-gradient(180deg, rgba(36, 26, 22, 0.9) 0%, rgba(26, 19, 16, 0.9) 100%);
  border-color: rgba(210, 105, 30, 0.12);
}

.station.active {
  border-color: rgba(210, 105, 30, 0.55);
  transform: translateY(-3px);
  box-shadow:
    0 0 0 3px rgba(210, 105, 30, 0.08),
    0 18px 40px -16px rgba(183, 65, 14, 0.45);
}

.station-glow {
  position: absolute;
  inset: -2px;
  border-radius: 14px;
  background: radial-gradient(80% 60% at 50% 0%, rgba(210, 105, 30, 0.35), transparent 70%);
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.4s ease;
  z-index: -1;
  filter: blur(10px);
}

.station.active .station-glow { opacity: 1; }

.station-tag {
  display: inline-block;
  font-size: 0.62rem;
  letter-spacing: 0.16em;
  text-transform: uppercase;
  font-weight: 600;
  color: var(--vp-c-text-3);
  margin-bottom: 0.5rem;
  padding: 0.15rem 0.5rem;
  background: var(--vp-c-bg-soft);
  border-radius: 4px;
  border: 1px solid var(--vp-c-divider);
}

.station.active .station-tag {
  color: var(--sm-rust-2);
  border-color: rgba(210, 105, 30, 0.4);
  background: rgba(210, 105, 30, 0.08);
}

.station-icon {
  width: 56px;
  height: 56px;
  margin-bottom: 0.6rem;
  filter: drop-shadow(0 4px 10px rgba(139, 69, 19, 0.18));
  transition: transform 0.5s cubic-bezier(0.4, 0, 0.2, 1);
}

.station-icon svg { width: 100%; height: 100%; }

.station.active .station-icon {
  transform: scale(1.06) rotate(-2deg);
  animation: station-bob 2.2s ease-in-out infinite;
}

@keyframes station-bob {
  0%, 100% { transform: scale(1.06) rotate(-2deg) translateY(0); }
  50% { transform: scale(1.06) rotate(-2deg) translateY(-3px); }
}

.station-title {
  font-weight: 600;
  font-size: 0.92rem;
  color: var(--vp-c-text-1);
  margin-bottom: 0.1rem;
}

.station-path {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.75rem;
  color: var(--sm-rust-2);
  margin-bottom: 0.75rem;
  word-break: break-all;
}

.station-files {
  list-style: none;
  margin: 0;
  padding: 0.55rem 0 0;
  border-top: 1px dashed var(--vp-c-divider);
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.72rem;
  color: var(--vp-c-text-2);
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.station-files li {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  transition: color 0.3s ease;
}

.station-files li.muted { color: var(--vp-c-text-3); }

.station-files li.hot {
  color: var(--sm-rust-2);
  font-weight: 600;
}

.station-files .dot {
  width: 6px;
  height: 6px;
  border-radius: 2px;
  background: var(--vp-c-text-3);
  flex-shrink: 0;
  transition: all 0.3s ease;
}

.station-files .dot.hot,
.station-files li.hot .dot {
  background: var(--sm-rust-2);
  box-shadow: 0 0 0 2px rgba(210, 105, 30, 0.18);
}

.badge {
  display: inline-flex;
  align-items: center;
  margin-left: auto;
  padding: 0.08rem 0.4rem;
  border-radius: 4px;
  font-size: 0.6rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  font-weight: 700;
  font-family: ui-monospace, monospace;
  animation: badge-in 0.4s ease;
}

.badge.swap {
  background: linear-gradient(135deg, var(--sm-rust-1), var(--sm-rust-2));
  color: #fff5e6;
  box-shadow: 0 0 12px rgba(210, 105, 30, 0.5);
}

.badge.restore {
  background: var(--vp-c-bg-soft);
  color: var(--sm-bronze);
  border: 1px solid var(--sm-bronze);
}

@keyframes badge-in {
  from { opacity: 0; transform: translateX(4px) scale(0.9); }
  to { opacity: 1; transform: none; }
}

/* Terminal */
.terminal {
  margin-top: 1rem;
  border-radius: 10px;
  overflow: hidden;
  border: 1px solid var(--vp-c-divider);
  background: #1a120e;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  box-shadow: 0 12px 30px -16px rgba(0, 0, 0, 0.5);
}

.terminal-bar {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.45rem 0.7rem;
  background: linear-gradient(180deg, #2a1d16, #1f1611);
  border-bottom: 1px solid rgba(210, 105, 30, 0.12);
}

.tb-dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  background: #444;
}
.tb-r { background: #ff5f56; }
.tb-y { background: #ffbd2e; }
.tb-g { background: #27c93f; }

.tb-title {
  margin-left: auto;
  font-size: 0.72rem;
  color: #b8a89a;
  letter-spacing: 0.02em;
}

.terminal-body {
  padding: 0.7rem 0.9rem 0.85rem;
  min-height: 86px;
  font-size: 0.78rem;
  line-height: 1.65;
  color: #e8d9c8;
}

.term-lines { display: flex; flex-direction: column; }

.term-line {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  white-space: pre;
}

.term-prompt {
  color: var(--sm-rust-2);
  font-weight: 700;
}

.term-cmd .term-text { color: #fff5e6; }
.term-info .term-text { color: #d6c1a8; }
.term-muted .term-text { color: #8b7864; }
.term-ok .term-text { color: #7fdc7e; }
.term-warn .term-text { color: #ffbd2e; }

.term-cursor {
  width: 7px;
  height: 1em;
  background: var(--sm-rust-2);
  display: inline-block;
  margin-left: 2px;
  animation: blink 1s steps(1) infinite;
}

@keyframes blink {
  50% { opacity: 0; }
}

.term-enter-active {
  transition: opacity 0.4s ease, transform 0.4s ease;
}
.term-enter-from {
  opacity: 0;
  transform: translateY(4px);
}
.term-leave-active {
  position: absolute;
  transition: opacity 0.25s ease;
}
.term-leave-to { opacity: 0; }

/* Mobile */
@media (max-width: 720px) {
  .smuggle-diagram { padding: 1rem 0.75rem; }
  .stations { grid-template-columns: 1fr; gap: 0.75rem; }
  .stage { min-height: 0; padding: 0.5rem 0; }
  .connector-layer { display: none; }
  .stage-bg { display: none; }
  .phase-label { align-items: flex-start; text-align: left; }
  .diagram-head { flex-direction: column; align-items: stretch; }
  .station {
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: auto auto auto;
    gap: 0.25rem 0.85rem;
    padding: 0.85rem;
  }
  .station-tag { grid-column: 1 / -1; }
  .station-icon {
    grid-row: 2 / 4;
    margin: 0;
    width: 44px; height: 44px;
  }
  .station-files { grid-column: 1 / -1; }
}
</style>
