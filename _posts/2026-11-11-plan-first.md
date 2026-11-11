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

LLMs have wrecked the first reason. A working implementation takes minutes. They don't answer the second by themselves, though they make the answer cheaper to find.

## Where the cost went

When generation is cheap, the expensive parts are the ones the model can't do for you:

- **Deciding what to build.** A vague spec produces confident, plausible, wrong code, quickly.
- **Reviewing it.** Review cost grows with the size of the diff, and nobody reviews a 3,000-line change well.
- **Architecture.** Rewriting a function is cheap. Unpicking a bad data model or protocol once other things depend on it still isn't.

Iterating by prompting, accepting, and patching spends your scarcest resource (attention) to save your cheapest (typing).

## It isn't the waterfall you were warned about

[Royce's 1970 paper](https://www.cs.umd.edu/class/spring2003/cmsc838p/Process/waterfall.pdf) is the one that drew the waterfall diagram, and it used the diagram as a cautionary example. The real failure of waterfall was late feedback. Planning was never the problem.

So keep the feedback and move it. **Use cheap code to learn, then plan what you learned, then use cheap code to build.**

1. **Prototype to learn.** Generate throwaway code to find out what's hard and what users actually want. Don't keep it.
2. **Write the spec.** Requirements, invariants, interfaces, failure modes, and tests, all before the real implementation.
3. **Generate against the spec.** The spec is the prompt, and the tests are the acceptance criteria.
4. **Review against the spec.** You're checking intent, not reading code looking for a vibe.
5. **If it's wrong, fix the spec and regenerate.** Don't patch. Regeneration is cheap, and patches pile up into code nobody understands.

You still iterate. The loop is prototype, plan, build a thin slice, and revise the plan, with the spec as the artifact you keep.

## But you can't plan what you haven't seen

Complexity mostly shows up when you build. A plan written before that is a hypothesis, and the first version of it will be wrong. [Gall's law](https://en.wikipedia.org/wiki/Gall%27s_law) says working complex systems grow from simple ones that worked, and Brooks's advice was to ["plan to throw one away"](https://en.wikipedia.org/wiki/The_Mythical_Man-Month) because you will anyway.

What changed is that throwing one away is now an afternoon of work. So explore by building freely, then plan at the level that matters: fix what is expensive to reverse (data model, interfaces, protocols, invariants) and leave the rest loose. Make the plan executable with tests and property checks, so it can fail. Then generate one thin end-to-end slice from it to see whether the spec survives contact with code. Planning here consolidates what building taught you. It doesn't replace building.

## The fair objections

- **"Requirements are never known upfront."** True, which is why step 1 exists, and why the spec gets revised after the first slice. A spec you can't yet write is a sign to prototype more.
- **"You'll over-specify."** Possible. Specify what has to be right (interfaces, invariants, failure modes), and leave the rest to the model.
- **"AI doesn't even make us faster."** The [METR study](https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/) found experienced developers were slower with early-2025 tools. That fits this argument, since unstructured prompting is expensive in review and rework.

## Takeaway

Agile priced in expensive code. Now that code is cheap, the cheap place to fix a mistake is the spec, and the cheap way to find most of them is a throwaway build. Explore with code, then commit to a plan, and keep the plan, not the code, as the thing you iterate on.
