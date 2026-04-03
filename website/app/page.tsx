export default function Home() {
  return (
    <main className="flex flex-col items-center justify-center min-h-screen px-6">
      {/* Hero */}
      <section className="flex flex-col items-center text-center max-w-3xl mt-24 mb-16">
        <div className="flex items-center gap-3 mb-6">
          <div className="w-12 h-12 rounded-xl bg-[#89b4fa] flex items-center justify-center">
            <span className="text-[#1e1e2e] text-2xl font-bold">F</span>
          </div>
          <h1 className="text-5xl font-bold tracking-tight text-white">
            Forge
          </h1>
        </div>

        <p className="text-xl text-[#a6adc8] mb-4">
          Multi-Agent IDE for macOS
        </p>

        <p className="text-[#6c7086] text-lg max-w-xl mb-10">
          A GPU-accelerated IDE built in Rust with GPUI. Manage multiple AI
          coding agents in parallel, with built-in terminal, git integration,
          and one-click commit &amp; push powered by Claude.
        </p>

        {/* Download button */}
        <a
          href="/api/download"
          className="inline-flex items-center gap-3 px-8 py-4 bg-[#89b4fa] hover:bg-[#b4befe] text-[#1e1e2e] font-bold text-lg rounded-xl transition-colors duration-200"
        >
          <svg className="w-6 h-6" fill="currentColor" viewBox="0 0 24 24">
            <path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.8-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z" />
          </svg>
          Download for Mac (Apple Silicon)
        </a>

        <p className="text-xs text-[#6c7086] mt-3">
          macOS 13+ &middot; Apple Silicon &middot; 4 MB
        </p>
      </section>

      {/* Features */}
      <section className="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-4xl w-full mb-20">
        <Feature
          icon=">"
          title="Multi-Agent Terminals"
          description="Run multiple Claude Code sessions in parallel tabs. Each terminal is a full PTY with ANSI color support."
        />
        <Feature
          icon="*"
          title="One-Click Git"
          description="Stage, generate commit message with AI, commit and push — all in one button. See changes with +/- stats."
        />
        <Feature
          icon="#"
          title="Built in Rust"
          description="GPU-accelerated with Metal via GPUI (Zed's framework). Native macOS performance on Apple Silicon."
        />
      </section>

      {/* Requirements */}
      <section className="max-w-2xl w-full mb-20">
        <h2 className="text-lg font-semibold text-[#a6adc8] mb-4 text-center">
          Requirements
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm text-center">
          <div className="bg-[#181825] rounded-lg p-4 border border-[#313244]">
            <p className="text-[#89b4fa] font-medium">macOS 13+</p>
            <p className="text-[#6c7086] text-xs mt-1">Apple Silicon</p>
          </div>
          <div className="bg-[#181825] rounded-lg p-4 border border-[#313244]">
            <p className="text-[#89b4fa] font-medium">Claude Code CLI</p>
            <p className="text-[#6c7086] text-xs mt-1">
              For AI commit messages
            </p>
          </div>
          <div className="bg-[#181825] rounded-lg p-4 border border-[#313244]">
            <p className="text-[#89b4fa] font-medium">Git</p>
            <p className="text-[#6c7086] text-xs mt-1">
              Configured with user.name
            </p>
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="text-center text-xs text-[#6c7086] pb-8">
        <p>
          Forge v0.1.0 &middot; Built with Rust &amp; GPUI &middot;{" "}
          <a
            href="https://github.com/melvin-viougea/forge"
            className="text-[#89b4fa] hover:underline"
          >
            GitHub
          </a>
        </p>
      </footer>
    </main>
  );
}

function Feature({
  icon,
  title,
  description,
}: {
  icon: string;
  title: string;
  description: string;
}) {
  return (
    <div className="bg-[#181825] rounded-xl p-6 border border-[#313244]">
      <div className="w-8 h-8 rounded-lg bg-[#313244] flex items-center justify-center text-[#89b4fa] font-mono font-bold mb-3">
        {icon}
      </div>
      <h3 className="text-white font-semibold mb-2">{title}</h3>
      <p className="text-sm text-[#6c7086]">{description}</p>
    </div>
  );
}
