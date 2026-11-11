---
title: "Plan First: Why Waterfall Deserves a Second Look in the LLM Era"
published: false
tags: [llm, process, engineering]
readtime: true
comments: true
---

For twenty years the advice has been the same: don't plan too much, ship something small, iterate. It was good advice. But it was a response to a specific price tag, and that price just dropped.

## Why we iterated

Two reasons, mixed together:

1. **Code was expensive.** Writing the wrong thing cost weeks, so we wrote small things and checked them early. This is the cost-of-change curve behind the [Agile Manifesto](https://agilemanifesto.org/).
2. **We didn't know what to build.** Requirements were fuzzy, users changed their minds, and only working software could settle the argument.

LLMs have wrecked the first reason. A working implementation takes minutes. They've done nothing for the second.

## Where the cost went

When generation is cheap, the expensive parts are the ones the model can't do for you:

- **Deciding what to build.** A vague spec produces confident, plausible, wrong code, quickly.
- **Reviewing it.** Review cost grows with the size of the diff, and nobody reviews a 3,000-line change well.
- **Architecture.** Rewriting a function is cheap. Unpicking a bad data model or protocol once other things depend on it still isn't.

Iterating by prompting, accepting, and patching spends your scarcest resource (attention) to save your cheapest (typing).

## It isn't the waterfall you were warned about

[Royce's 1970 paper](https://www.cs.umd.edu/class/spring2003/cmsc838p/Process/waterfall.pdf) is the one that drew the waterfall diagram, and it used the diagram as a cautionary example. The real failure of waterfall was late feedback. Planning was never the problem.

So keep the feedback and move it. **Use cheap code to learn, then plan, then use cheap code to build.**

1. **Prototype to learn.** Generate throwaway code to find out what's hard and what users actually want. Don't keep it.
2. **Write the spec.** Requirements, invariants, interfaces, failure modes, and tests, all before the real implementation.
3. **Generate against the spec.** The spec is the prompt, and the tests are the acceptance criteria.
4. **Review against the spec.** You're checking intent, not reading code looking for a vibe.
5. **If it's wrong, fix the spec and regenerate.** Don't patch. Regeneration is cheap, and patches pile up into code nobody understands.

You still iterate, but on the spec instead of the code.

## Where this matters most

In my own field, a bug in a cold or MPC wallet can lose funds, and "it passed the demo" isn't evidence of anything. There the spec (threat model, protocol, invariants) is the real product, and the code is a derivation of it. [Spec-driven tooling like GitHub Spec Kit](https://github.com/github/spec-kit) points the same way.

## The fair objections

- **"Requirements are never known upfront."** True, which is why step 1 exists. If you can't write the spec, you haven't finished learning.
- **"You'll over-specify."** Possible. Specify what has to be right (interfaces, invariants, failure modes), and leave the rest to the model.
- **"AI doesn't even make us faster."** The [METR study](https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/) found experienced developers were slower with early-2025 tools. That fits this argument, since unstructured prompting is expensive in review and rework.

## Takeaway

Agile priced in expensive code. Now that code is cheap, the cheapest place to find a mistake is in the spec, before generating anything. Iterate there.
