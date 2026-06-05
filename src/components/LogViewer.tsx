import { useState, useEffect, useRef } from "react";
import { bridge } from "../bridge";

export default function LogViewer() {
  const [content, setContent] = useState("");
  const [follow, setFollow] = useState(true);
  const bottomRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let active = true;

    async function poll() {
      try {
        const text = await bridge.getLog();
        if (active) setContent(text);
      } catch {
        // log file may not exist yet
      }
    }

    poll();
    const id = setInterval(poll, 1000);
    return () => { active = false; clearInterval(id); };
  }, []);

  useEffect(() => {
    if (follow) bottomRef.current?.scrollIntoView();
  }, [content, follow]);

  function handleScroll() {
    const el = containerRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    if (!atBottom && follow) setFollow(false);
    if (atBottom && !follow) setFollow(true);
  }

  return (
    <div className="h-screen flex flex-col bg-surface text-white">
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-surface-border shrink-0">
        <span className="text-xs font-semibold uppercase tracking-wider text-muted">
          Zen Sync — Log
        </span>
        <button
          type="button"
          onClick={() => setFollow((f) => !f)}
          className={`text-xs px-2.5 py-1 rounded-lg border transition-colors ${
            follow
              ? "bg-accent/15 border-accent/30 text-accent"
              : "bg-surface-overlay border-surface-border text-muted hover:text-white"
          }`}
        >
          {follow ? "Following" : "Follow"}
        </button>
      </div>
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto p-4"
      >
        <pre className="text-xs font-mono text-subtle leading-relaxed whitespace-pre-wrap break-all">
          {content || "No log entries yet."}
        </pre>
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
