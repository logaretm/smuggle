<script setup>
import { ref, onMounted, onUnmounted } from "vue";

const canvas = ref(null);
const container = ref(null);
let animationId = null;
let particles = [];
let paths = [];
let time = 0;

const PUBLISH_STEPS = [
  { id: "source", label: "Your Package", icon: "📦" },
  { id: "pack", label: "npm pack", icon: "📋" },
  { id: "store", label: "~/.smuggle/", icon: "🗄️" },
];

const INSTALL_STEPS = [
  { id: "scan", label: "Scan Dependencies", icon: "🔍" },
  { id: "backup", label: "Backup Originals", icon: "💾" },
  { id: "extract", label: "Swap into node_modules", icon: "📂" },
  { id: "watch", label: "Watch & Re-swap", icon: "👀" },
  { id: "restore", label: "Restore on Exit", icon: "♻️" },
];

function getNodeCenter(el) {
  if (!el || !container.value) return { x: 0, y: 0 };
  const cr = container.value.getBoundingClientRect();
  const nr = el.getBoundingClientRect();
  return {
    x: nr.left - cr.left + nr.width / 2,
    y: nr.top - cr.top + nr.height / 2,
  };
}

function buildPaths() {
  const c = container.value;
  if (!c) return;

  paths = [];
  const pubNodes = c.querySelectorAll(".publish-col .step-node");
  const instNodes = c.querySelectorAll(".install-col .step-node");

  for (let i = 0; i < pubNodes.length - 1; i++) {
    const from = getNodeCenter(pubNodes[i]);
    const to = getNodeCenter(pubNodes[i + 1]);
    paths.push({ from, to, color: "#d2691e" });
  }

  if (pubNodes.length && instNodes.length) {
    const from = getNodeCenter(pubNodes[pubNodes.length - 1]);
    const to = getNodeCenter(instNodes[0]);
    paths.push({ from, to, color: "#b7410e", bridge: true });
  }

  for (let i = 0; i < instNodes.length - 1; i++) {
    const from = getNodeCenter(instNodes[i]);
    const to = getNodeCenter(instNodes[i + 1]);
    paths.push({ from, to, color: "#8b4513" });
  }
}

function spawnParticles() {
  particles = [];
  paths.forEach((p, i) => {
    for (let j = 0; j < 3; j++) {
      particles.push({
        pathIndex: i,
        t: (j / 3 + Math.random() * 0.1) % 1,
        speed: 0.003 + Math.random() * 0.002,
        size: 3 + Math.random() * 2,
      });
    }
  });
}

function draw() {
  const ctx = canvas.value?.getContext("2d");
  if (!ctx || !container.value) return;

  const rect = container.value.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  canvas.value.width = rect.width * dpr;
  canvas.value.height = rect.height * dpr;
  canvas.value.style.width = rect.width + "px";
  canvas.value.style.height = rect.height + "px";
  ctx.scale(dpr, dpr);

  ctx.clearRect(0, 0, rect.width, rect.height);

  paths.forEach((p) => {
    ctx.beginPath();
    if (p.bridge) {
      const midX = (p.from.x + p.to.x) / 2;
      ctx.moveTo(p.from.x, p.from.y);
      ctx.bezierCurveTo(midX, p.from.y, midX, p.to.y, p.to.x, p.to.y);
    } else {
      ctx.moveTo(p.from.x, p.from.y);
      ctx.lineTo(p.to.x, p.to.y);
    }
    ctx.strokeStyle = p.color + "30";
    ctx.lineWidth = 2;
    ctx.stroke();
  });

  particles.forEach((particle) => {
    const p = paths[particle.pathIndex];
    if (!p) return;

    particle.t += particle.speed;
    if (particle.t > 1) particle.t -= 1;

    let x, y;
    if (p.bridge) {
      const midX = (p.from.x + p.to.x) / 2;
      const t = particle.t;
      const mt = 1 - t;
      x =
        mt * mt * mt * p.from.x +
        3 * mt * mt * t * midX +
        3 * mt * t * t * midX +
        t * t * t * p.to.x;
      y =
        mt * mt * mt * p.from.y +
        3 * mt * mt * t * p.from.y +
        3 * mt * t * t * p.to.y +
        t * t * t * p.to.y;
    } else {
      x = p.from.x + (p.to.x - p.from.x) * particle.t;
      y = p.from.y + (p.to.y - p.from.y) * particle.t;
    }

    ctx.beginPath();
    ctx.arc(x, y, particle.size, 0, Math.PI * 2);
    ctx.fillStyle = p.color;
    ctx.globalAlpha = 0.4 + 0.4 * Math.sin(particle.t * Math.PI);
    ctx.fill();
    ctx.globalAlpha = 1;
  });

  time++;
  animationId = requestAnimationFrame(draw);
}

let resizeObserver = null;

onMounted(() => {
  buildPaths();
  spawnParticles();
  draw();

  resizeObserver = new ResizeObserver(() => {
    buildPaths();
    spawnParticles();
  });
  resizeObserver.observe(container.value);
});

onUnmounted(() => {
  if (animationId) cancelAnimationFrame(animationId);
  if (resizeObserver) resizeObserver.disconnect();
});
</script>

<template>
  <div ref="container" class="process-diagram">
    <canvas ref="canvas" class="diagram-canvas" />
    <div class="diagram-content">
      <div class="publish-col">
        <div class="col-header">
          <code>smuggle publish</code>
        </div>
        <div v-for="step in PUBLISH_STEPS" :key="step.id" class="step-node">
          <span class="step-icon">{{ step.icon }}</span>
          <span class="step-label">{{ step.label }}</span>
        </div>
      </div>
      <div class="install-col">
        <div class="col-header">
          <code>smuggle install</code>
        </div>
        <div v-for="step in INSTALL_STEPS" :key="step.id" class="step-node">
          <span class="step-icon">{{ step.icon }}</span>
          <span class="step-label">{{ step.label }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.process-diagram {
  position: relative;
  margin: 2rem auto;
  max-width: 720px;
  padding: 1.5rem;
}

.diagram-canvas {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}

.diagram-content {
  position: relative;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 2rem;
}

@media (max-width: 640px) {
  .diagram-content {
    grid-template-columns: 1fr;
    gap: 1rem;
  }
}

.col-header {
  text-align: center;
  margin-bottom: 1rem;
  font-size: 1.1rem;
  font-weight: 600;
}

.col-header code {
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
  padding: 0.25rem 0.75rem;
  border-radius: 6px;
  font-size: 0.95rem;
}

.step-node {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.75rem 1rem;
  margin-bottom: 0.75rem;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-border);
  border-radius: 10px;
  transition: border-color 0.25s, box-shadow 0.25s;
}

.step-node:hover {
  border-color: var(--vp-c-brand-1);
  box-shadow: 0 2px 12px var(--vp-c-brand-soft);
}

.step-icon {
  font-size: 1.25rem;
  flex-shrink: 0;
}

.step-label {
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--vp-c-text-1);
}
</style>
