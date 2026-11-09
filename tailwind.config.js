/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        surface: {
          DEFAULT: "#0f0f13",
          raised: "#16161d",
          overlay: "#1e1e28",
          border: "#2a2a38",
        },
        accent: {
          DEFAULT: "#7c6af7",
          hover: "#9180f9",
          muted: "#3d3670",
        },
        success: "#22c55e",
        warning: "#f59e0b",
        danger: "#ef4444",
        // 5:1 on surface-raised; the old #6b7280 was 3.7:1, below WCAG AA for small text.
        muted: "#808897",
        subtle: "#9ca3af",
      },
      fontFamily: {
        sans: [
          "Inter Variable",
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "sans-serif",
        ],
        mono: ["Cascadia Mono", "Consolas", "monospace"],
      },
      borderRadius: {
        xl: "0.875rem",
        "2xl": "1.25rem",
      },
      animation: {
        "fade-in": "fadeIn 0.2s ease-out",
        "slide-up": "slideUp 0.25s ease-out",
        spin: "spin 1s linear infinite",
        pulse: "pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite",
      },
      keyframes: {
        fadeIn: {
          from: { opacity: "0" },
          to: { opacity: "1" },
        },
        slideUp: {
          from: { opacity: "0", transform: "translateY(8px)" },
          to: { opacity: "1", transform: "translateY(0)" },
        },
      },
    },
  },
  plugins: [],
};
