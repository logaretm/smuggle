<script setup>
import { withBase } from "vitepress";
import { ref, computed } from "vue";

const STAMPS = [
  "DENIED",
  "NOT FOUND",
  "INTERCEPTED",
  "WRONG CRATE",
];

const stamp = ref(0);
const current = computed(() => STAMPS[stamp.value % STAMPS.length]);

function reroll() {
  stamp.value++;
}
</script>

<template>
  <div class="nf">
    <div class="nf-inner">
      <div class="nf-scene">
        <svg viewBox="0 0 360 220" aria-hidden="true" class="nf-art">
          <defs>
            <linearGradient id="nf-front" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stop-color="#d2691e"/>
              <stop offset="1" stop-color="#a0360b"/>
            </linearGradient>
            <linearGradient id="nf-top" x1="0" y1="1" x2="0" y2="0">
              <stop offset="0" stop-color="#cd853f"/>
              <stop offset="1" stop-color="#e8a070"/>
            </linearGradient>
            <linearGradient id="nf-right" x1="0" y1="0" x2="1" y2="0">
              <stop offset="0" stop-color="#8b2f0a"/>
              <stop offset="1" stop-color="#6b2407"/>
            </linearGradient>
            <linearGradient id="nf-tape-front" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stop-color="#a0360b"/>
              <stop offset="1" stop-color="#7a2807"/>
            </linearGradient>
          </defs>

          <!-- Ground line -->
          <line x1="0" y1="200" x2="360" y2="200" stroke="var(--sm-rust-3)" stroke-width="1" stroke-dasharray="3 5" opacity="0.35" />

          <!-- Shadow -->
          <ellipse cx="180" cy="200" rx="100" ry="6" fill="#000" opacity="0.22" />

          <!-- Crate (transformed-up to leave room for stamp) -->
          <g transform="translate(108 56)">
            <path d="M96 48 L96 112 L112 96 L112 32 Z" fill="url(#nf-right)" stroke="#4a1505" stroke-width="2" stroke-linejoin="round"/>
            <path d="M24 48 L40 32 L112 32 L96 48 Z" fill="url(#nf-top)" stroke="#4a1505" stroke-width="2" stroke-linejoin="round"/>
            <path d="M52 48 L68 32 L84 32 L68 48 Z" fill="#b7410e" stroke="#4a1505" stroke-width="1.2" stroke-linejoin="round" opacity="0.85"/>
            <path d="M24 48 L96 48 L96 112 L24 112 Z" fill="url(#nf-front)" stroke="#4a1505" stroke-width="2" stroke-linejoin="round"/>
            <path d="M52 48 L68 48 L68 112 L52 112 Z" fill="url(#nf-tape-front)" stroke="#4a1505" stroke-width="1.2" stroke-linejoin="round" opacity="0.85"/>
            <g stroke="#fff5e6" stroke-width="5.5" stroke-linecap="round" stroke-linejoin="round" fill="none">
              <path d="M50 64 Q42 64 42 71 L42 77 Q42 80 36 80 Q42 80 42 83 L42 89 Q42 96 50 96"/>
              <path d="M70 64 Q78 64 78 71 L78 77 Q78 80 84 80 Q78 80 78 83 L78 89 Q78 96 70 96"/>
            </g>
          </g>

          <!-- Stamp -->
          <g transform="translate(252 92) rotate(-14)">
            <rect
              x="-58" y="-22" width="116" height="44" rx="6"
              fill="none"
              stroke="#c44" stroke-width="3"
              opacity="0.9"
            />
            <text
              x="0" y="6"
              text-anchor="middle"
              font-family="ui-monospace, SF Mono, Menlo, monospace"
              font-size="18"
              font-weight="800"
              fill="#c44"
              letter-spacing="2"
            >{{ current }}</text>
          </g>

          <!-- Scattered package fragments -->
          <g opacity="0.7">
            <rect x="40" y="180" width="8" height="6" fill="#d2691e" transform="rotate(20 44 183)" />
            <rect x="320" y="186" width="6" height="5" fill="#cd853f" transform="rotate(-15 323 188)" />
            <rect x="280" y="190" width="10" height="4" fill="#8b4513" transform="rotate(8 285 192)" />
          </g>
        </svg>
      </div>

      <div class="nf-code">404</div>
      <h1 class="nf-title">This package never made it through customs</h1>
      <p class="nf-msg">
        The page you tried to smuggle in doesn't exist. Maybe it was renamed,
        moved, or never made it past inspection.
      </p>

      <div class="nf-actions">
        <a class="nf-btn primary" :href="withBase('/')">
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 8 L8 2 L14 8 M4 7 V14 H12 V7" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>
          Back to home
        </a>
        <a class="nf-btn" :href="withBase('/guide/getting-started')">
          <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="2" y="2" width="12" height="12" rx="2" fill="none" stroke="currentColor" stroke-width="1.6" /><path d="M5 6 H11 M5 9 H11 M5 12 H8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
          Read the guide
        </a>
        <button class="nf-btn ghost" @click="reroll" type="button">
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M13 5 a6 6 0 1 0 1.5 4 M13 1 V5 H9" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>
          Stamp again
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.nf {
  display: grid;
  place-items: center;
  min-height: calc(100vh - var(--vp-nav-height));
  padding: 3rem 1.5rem 5rem;
  background:
    radial-gradient(80% 60% at 50% 0%, rgba(210, 105, 30, 0.08), transparent 70%);
}

.nf-inner {
  text-align: center;
  max-width: 560px;
}

.nf-scene {
  margin: 0 auto 1rem;
  max-width: 440px;
  filter: drop-shadow(0 30px 50px rgba(139, 69, 19, 0.18));
}

.nf-art { width: 100%; height: auto; }

.nf-code {
  font-family: var(--vp-font-family-mono);
  font-size: 0.85rem;
  font-weight: 600;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: var(--sm-rust-2);
  margin-bottom: 0.5rem;
}

.nf-title {
  font-size: 1.85rem;
  line-height: 1.2;
  letter-spacing: -0.018em;
  font-weight: 700;
  margin: 0 0 0.65rem;
  background: linear-gradient(135deg, #b7410e, #d2691e, #8b4513);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}

.nf-msg {
  color: var(--vp-c-text-2);
  font-size: 1rem;
  line-height: 1.6;
  margin: 0 0 2rem;
}

.nf-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.6rem;
  justify-content: center;
}

.nf-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.6rem 1.05rem;
  font-size: 0.92rem;
  font-weight: 500;
  border-radius: 999px;
  border: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg);
  color: var(--vp-c-text-1);
  text-decoration: none !important;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.2s ease;
}

.nf-btn:hover {
  border-color: rgba(210, 105, 30, 0.45);
  transform: translateY(-1px);
  box-shadow: 0 8px 24px -10px rgba(183, 65, 14, 0.4);
}

.nf-btn.primary {
  background: linear-gradient(135deg, #b7410e, #d2691e);
  border-color: transparent;
  color: #fff5e6;
  box-shadow: 0 6px 20px -6px rgba(183, 65, 14, 0.55);
}

.nf-btn.primary:hover {
  background: linear-gradient(135deg, #c4490f, #e0712a);
  box-shadow: 0 10px 28px -8px rgba(183, 65, 14, 0.7);
}

.nf-btn.ghost {
  background: transparent;
  border-color: var(--vp-c-divider);
  color: var(--vp-c-text-2);
}

.nf-btn.ghost:hover { color: var(--sm-rust-2); }

.nf-btn svg {
  width: 14px;
  height: 14px;
}

@media (max-width: 540px) {
  .nf-title { font-size: 1.5rem; }
  .nf-scene { max-width: 320px; }
}
</style>
