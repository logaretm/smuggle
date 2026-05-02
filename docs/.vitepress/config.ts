import { defineConfig } from "vitepress";
import fs from "node:fs";
import path from "node:path";

const DOCS_ORDER = [
  { path: "guide/getting-started.md", title: "Getting Started" },
  { path: "guide/why.md", title: "Why Smuggle?" },
  { path: "guide/ci.md", title: "CI / Non-Interactive Usage" },
  { path: "reference/commands.md", title: "Commands" },
  { path: "reference/flags.md", title: "Flags" },
];

function stripFrontmatter(content: string): string {
  return content.replace(/^---[\s\S]*?---\n*/, "").trim();
}

function generateLlmsTxt(siteConfig: { outDir: string; srcDir: string }) {
  const { outDir, srcDir } = siteConfig;

  const index = [
    "# Smuggle",
    "",
    "> Smuggle local npm packages into your projects — no symlinks, no lockfile pollution.",
    "",
    "## Docs",
    "",
    ...DOCS_ORDER.map((doc) => `- [${doc.title}](/${doc.path})`),
    "",
  ].join("\n");

  const full = [
    index,
    ...DOCS_ORDER.map((doc) => {
      const filePath = path.join(srcDir, doc.path);
      const content = fs.readFileSync(filePath, "utf-8");
      return stripFrontmatter(content);
    }),
  ].join("\n\n---\n\n");

  fs.writeFileSync(path.join(outDir, "llms.txt"), index);
  fs.writeFileSync(path.join(outDir, "llms-full.txt"), full);
}

export default defineConfig({
  base: "/smuggle/",
  title: "Smuggle",
  description:
    "Smuggle local npm packages into your projects — no symlinks, no lockfile pollution",
  head: [
    ["link", { rel: "preconnect", href: "https://fonts.googleapis.com" }],
    [
      "link",
      { rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "" },
    ],
    [
      "link",
      {
        rel: "stylesheet",
        href: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;600;700&display=swap",
      },
    ],
    [
      "style",
      {},
      `:root {
  --vp-c-brand-1: #b7410e;
  --vp-c-brand-2: #a0360b;
  --vp-c-brand-3: #8b2f0a;
  --vp-c-brand-soft: rgba(183, 65, 14, 0.14);
  --vp-home-hero-name-color: transparent;
  --vp-home-hero-name-background: linear-gradient(135deg, #b7410e, #d2691e, #8b4513);
  --vp-home-hero-image-background-image: linear-gradient(135deg, rgba(183, 65, 14, 0.3), rgba(139, 69, 19, 0.3));
  --vp-home-hero-image-filter: blur(56px);
}
.dark {
  --vp-c-brand-1: #d2691e;
  --vp-c-brand-2: #cd853f;
  --vp-c-brand-3: #b7410e;
  --vp-c-brand-soft: rgba(210, 105, 30, 0.14);
}`,
    ],
  ],
  markdown: {
    theme: {
      light: "vitesse-light",
      dark: "vitesse-dark",
    },
  },
  themeConfig: {
    logo: "/logo.svg",
    nav: [
      { text: "Guide", link: "/guide/getting-started" },
      { text: "Reference", link: "/reference/commands" },
    ],
    sidebar: [
      {
        text: "Guide",
        items: [
          { text: "Why Smuggle?", link: "/guide/why" },
          { text: "Getting Started", link: "/guide/getting-started" },
          { text: "CI / Non-Interactive Usage", link: "/guide/ci" },
        ],
      },
      {
        text: "Reference",
        items: [
          { text: "Commands", link: "/reference/commands" },
          { text: "Flags", link: "/reference/flags" },
        ],
      },
    ],
    socialLinks: [
      { icon: "github", link: "https://github.com/logaretm/smuggle" },
    ],
  },
  buildEnd: generateLlmsTxt,
});
