# Why scaff needed a usability rewrite

## The first-run problem

Old behavior: You install scaff, run `scaff research "question"`, and it prints a hint telling you to run three manual steps — set a key, update the corpus, then try again. That means the very first interaction with the product is the product telling you to get lost and come back later.

**New behavior: `scaff research "question"` just works on first run.**

Because: A CLI tool's first impression is the five seconds after you type its name. If the answer isn't "here's your output", you've already lost most users. scaff's job is to answer questions about Harness — not to be a thing you configure before it can do its job. So we hunt through every env var (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, `GROQ_API_KEY`), pick the first one that exists, seed the corpus automatically, and print the answer. No ceremony. No "before you begin" checklist.

---

## The invisible pipeline

Old behavior: A single spinner that says "Researching..." for 5-15 seconds with no indication of what's happening. The pipeline has five stages, but the user sees none of them.

**New behavior: Per-stage progress lines with timing and detail.**

Because: A spinner is a black box. When something takes more than three seconds, humans either assume it's broken or they walk away. By showing each stage — `plan → search → verify → synthesize → render` — with timestamps and results ("8 corpus hits", "12 claims verified"), the user develops a mental model of how the tool works. They learn that "verify" takes the longest, or that "search" found nothing. This isn't decoration. It's teaching the user how to trust the output. A cited answer from a visible pipeline is more believable than a cited answer from a magic box.

---

## The raw-markdown report

Old behavior: The report was a wall of markdown. Claim confidence was embedded in the text, but you had to read carefully to find it.

**New behavior: The report opens with a Confidence Summary (✅ High / ⚠️ Medium / 🔴 Low / ⚡ Contested) and a Key Claims section with per-claim badges and source annotations.**

Because: A cited report is only useful if the reader can quickly assess which parts to trust. By surfacing confidence as a scannable table at the top — not buried in prose — the user can decide in two seconds whether this answer is actionable or needs deeper investigation. This is the core value prop of scaff (verified claims) rendered as UX, not as architecture.

---

## The setup wizard

Old behavior: `scaff config set-key`, `scaff config set`, `scaff corpus update` — three separate commands you have to discover and remember.

**New behavior: `scaff setup` — one command that walks you through provider selection, API key entry, corpus seeding, and connection testing.**

Because: Configuration is a tax, not a feature. A wizard makes it a guided path instead of a scavenger hunt. By showing live provider status ("configured", "needs key", "running locally") and testing the connection before finishing, the user finishes setup knowing it works — rather than wondering if they configured it right.

---

## The CLI help wall

Old behavior: `scaff --help` showed 20+ commands with no orientation. A new user had no idea where to start.

**New behavior: The help text opens with `Quick start: scaff "What is Harness Continuous Delivery?"` and mentions provider auto-detection.**

Because: Help text is the most-read documentation page. If it lists every command alphabetically, the user reads nothing. By putting the most common action first and explaining that keys are auto-detected, we answer the two questions every new user has: "what do I type" and "does this need a credit card".
