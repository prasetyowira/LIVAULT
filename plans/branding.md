# LiVault – Branding & UI Design Guide

## 🎨 Color Identity

- **Security & Trust** – Blue tones evoke reliability and safety (essential for vaults and sensitive data).
- **Tech-savvy & Decentralized** – Cyan leans toward futuristic and digital.
- **Emotional Depth** – Deep cyan has a quiet power, perfect for legacy and memory themes.
- **Versatile** – Palette works well in both light and dark mode UIs.

---

## 🔒 Logo Philosophy

- Combines **technical edge** with **warmth of legacy**
- Designed to be recognizable across **web**, **mobile**, and **dark mode** contexts
- Modern, clean, but emotional enough to convey trust and heritage

---

## 🌈 Complete Color Palette

| Color Code | Usage |
|------------|--------|
| `#007C91` | Base brand teal |
| `#59B6C6` | Soft teal (backgrounds or cards) |
| `#1B3C45` | Deep navy (text or headers) |
| `#CED9DB` | Misty light grey-blue (UI sections) |
| `#94AEB1` | Steel blue accent (hover/secondary) |
| `#F1F4F5` | Off-white (background or spacing) |

---

## 🧩 TailwindCSS Config
```js
"theme": {
  "extend": {
    "colors": {
      "brand": "#007C91",
      "accent1": "#59B6C6",
      "text": "#1B3C45",
      "background": "#F1F4F5"
    }
  }
}
```

---

## Figma swatches:

```csv
Name,Hex
brand,#007C91
accent1,#59B6C6
text,#1B3C45
background,#F1F4F5
neutral,#CED9DB
secondary,#94AEB1
```

---

## Font

| Role in UI | Font Family |  Why it works |
|------------|-------------|---------------|
| Primary UI / Body | Inter (variable) | Open‑source, excellent legibility at small sizes, neutral‑friendly shapes that sit well next to the deep‑navy text color #1B3C45. |
| Headings / Emphasis | Space Grotesk | Geometric grotesque with subtle quirks → gives modern‑tech personality that complements cyan accents without feeling cold. |
| Optional “Human” accent (e.g., personal letters preview) | Merriweather Serif | A warm, readable serif for small blocks of long‑form text; contrasts nicely with the primary sans while echoing “heritage.” |
| Code / Data (admin & tech docs) | JetBrains Mono | Monospaced, high‑x‑height, friendly curves—keeps dev‑oriented screens on‑brand. |

### Tailwind Font Config
```js
// tailwind.config.js
export default {
  theme: {
    extend: {
      fontFamily: {
        sans: ['InterVariable', 'ui-sans-serif', 'system-ui'],
        heading: ['"Space Grotesk"', 'InterVariable', 'sans-serif'],
        serif: ['Merriweather', 'Georgia', 'serif'],
        mono: ['"JetBrains Mono"', 'SFMono-Regular', 'monospace'],
      },
    },
  },
  plugins: [
    require('@tailwindcss/typography'),
    // If you install @tailwindcss/font-variant for variable fonts
  ],
};
```

## 🔗 Linked in PRD
Referenced in [LiVault – Product Requirements Document](prd.md).
