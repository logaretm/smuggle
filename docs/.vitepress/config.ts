import { defineConfig } from "vitepress";
import fs from "node:fs";
import path from "node:path";

const DOCS_ORDER = [
  { path: "guide/getting-started.md", title: "Getting Started" },
  { path: "guide/why.md", title: "Why Smuggle?" },
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
    "> Test your local npm packages in a real project, without npm link.",
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

const SITE_URL = "https://awad.dev/smuggle";
const OG_IMAGE = `${SITE_URL}/og.png`;
const SITE_TITLE = "Smuggle";
const SITE_DESCRIPTION =
  "Test your local npm packages in a real project, without npm link";

export default defineConfig({
  base: "/smuggle/",
  title: SITE_TITLE,
  description: SITE_DESCRIPTION,
  head: [
    ["link", { rel: "icon", type: "image/svg+xml", href: "/smuggle/logo.svg" }],
    [
      "link",
      { rel: "icon", type: "image/png", sizes: "32x32", href: "/smuggle/favicon-32x32.png" },
    ],
    [
      "link",
      { rel: "icon", type: "image/png", sizes: "16x16", href: "/smuggle/favicon-16x16.png" },
    ],
    [
      "link",
      { rel: "apple-touch-icon", sizes: "180x180", href: "/smuggle/apple-touch-icon.png" },
    ],
    ["link", { rel: "manifest", href: "/smuggle/site.webmanifest" }],
    ["meta", { name: "theme-color", content: "#b7410e" }],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:title", content: SITE_TITLE }],
    ["meta", { property: "og:description", content: SITE_DESCRIPTION }],
    ["meta", { property: "og:url", content: SITE_URL }],
    ["meta", { property: "og:image", content: OG_IMAGE }],
    ["meta", { property: "og:image:width", content: "1200" }],
    ["meta", { property: "og:image:height", content: "630" }],
    ["meta", { property: "og:site_name", content: SITE_TITLE }],
    ["meta", { name: "twitter:card", content: "summary_large_image" }],
    ["meta", { name: "twitter:title", content: SITE_TITLE }],
    ["meta", { name: "twitter:description", content: SITE_DESCRIPTION }],
    ["meta", { name: "twitter:image", content: OG_IMAGE }],
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
    search: {
      provider: "local",
      options: {
        miniSearch: {
          searchOptions: {
            fuzzy: 0.2,
            prefix: true,
            boost: { title: 4, text: 2, titles: 1 },
          },
        },
      },
    },
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
