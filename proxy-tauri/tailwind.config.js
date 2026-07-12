/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: { DEFAULT: "var(--bg)", card: "var(--bg-card)", elev: "var(--bg-elev)", input: "var(--bg-input)" },
        border: { DEFAULT: "var(--border)", focus: "var(--border-focus)" },
        text: { 1: "var(--text-1)", 2: "var(--text-2)", 3: "var(--text-3)" },
        accent: { DEFAULT: "var(--accent)", bg: "var(--accent-bg)" },
        green: { DEFAULT: "var(--green)", bg: "var(--green-bg)" },
        red: { DEFAULT: "var(--red)", bg: "var(--red-bg)" },
        yellow: { DEFAULT: "var(--yellow)", bg: "var(--yellow-bg)" },
        purple: { DEFAULT: "var(--purple)", bg: "var(--purple-bg)" },
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "-apple-system", "sans-serif"],
        mono: ["JetBrains Mono", "Fira Code", "monospace"],
      },
      borderRadius: { lg: "var(--radius)", md: "calc(var(--radius) - 2px)", sm: "calc(var(--radius) - 4px)" },
    },
  },
  plugins: [require("tailwindcss-animate"), require("@tailwindcss/typography")],
};
