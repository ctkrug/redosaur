---
title: "I built a tool that runs your regex to prove it can hang your server"
published: false
tags: rust, webassembly, security, regex
canonical_url: https://apps.charliekrug.com/redosaur/
---

A regex like `^(a+)+$` compiles without a warning, passes every unit test, and sails through code
review. Then one day a 30-character request shows up, the pattern falls into exponential
backtracking, and a single CPU core is pinned for the rest of the afternoon. That is
[ReDoS](https://owasp.org/www-community/attacks/Regular_expression_Denial_of_Service_-_ReDoS),
regular-expression denial of service, and it has taken down real systems (Cloudflare in 2019, Stack
Overflow in 2016, a steady drip of CVEs against npm and PyPI packages).

The tools that catch this usually pattern-match your regex *source* against a list of known-bad
shapes. That is fast, but it produces false positives that train you to ignore the tool and false
negatives on shapes nobody added to the list. I wanted something that does not guess from the shape
of the text. So [ReDoSaur](https://apps.charliekrug.com/redosaur/) actually runs the pattern and
measures the blowup. Here are the parts that were interesting to build.

## The engine is a backtracking matcher in continuation-passing style

The core is a small Rust crate with no dependencies. It parses a regex into an AST, then walks it
against an input in exactly the greedy, backtracking way that PCRE, Python's `re`, and JavaScript's
`RegExp` do. The trick that keeps it small is continuation-passing: every node's matcher takes a
closure `k` that represents "match the rest of the pattern from here," and returns whether the whole
thing succeeded.

```rust
fn match_node(ast: &Ast, input: &[char], pos: usize,
              counters: &mut Counters,
              k: &dyn Fn(usize, &mut Counters) -> bool) -> bool {
    // ...
    Ast::Repeat { node, min, max } =>
        match_repeat(node, *min, *max, 0, input, pos, counters, k),
    // ...
}
```

A repeat matches its body one more time, and its continuation is "try to match the rest, and if that
fails, backtrack and match one fewer repetition." Nesting two of those (`(a+)+`) is what makes the
engine explore an exponential number of ways to split the same run of characters. The matcher counts
every step into a `Counters` struct, and runs under `fullmatch` semantics so a single non-matching
character at the end forces it to exhaust every split before giving up. That trailing mismatch is the
whole reason the classic ReDoS patterns blow up.

## The verdict comes from measurement, not shape

Structural analysis (nested quantifiers, overlapping alternation, buried variable-length repeats)
only *seeds* candidate worst-case inputs. The actual Safe / Suspicious / Catastrophic verdict comes
from running the engine against generated inputs at increasing lengths and measuring how fast the
step count grows. If it roughly doubles per added character, that is exponential and real. This is
what lets it clear a scary-looking but bounded pattern like `(a{1,2}){1,2}` all the way down to Safe.

## It runs in your browser without freezing the tab

A genuinely catastrophic pattern can hit millions of steps. Doing that synchronously would freeze
the page, which is a bad look for a tool about not freezing things. The engine takes a step ceiling,
and the front end calls it repeatedly across animation frames with a doubling budget, so the counter
animates upward smoothly and the main thread stays responsive. The whole thing is Rust compiled to
WebAssembly, served as flat static files, so nothing you paste ever leaves your machine.

## The funniest bug: my DoS tool had two DoS vectors

While hardening it I found that a deeply nested pattern like `(((((...)))))` blew the native stack in
the recursive-descent parser, and a very long flat pattern did the same in the concat matcher. A tool
whose entire premise is "adversarial regex input is dangerous" was itself killable by adversarial
regex input. Both are now bounded by explicit depth and length caps that fail cleanly with a parse
error.

## What I would do differently

The rewrite suggester only knows a couple of transformations (flattening nested quantifiers,
de-duplicating alternation branches). Atomic groups and possessive quantifiers would cover a lot more
real patterns, and I would add a small CLI over the same core crate for CI pipelines.

Code and live demo:

- Try it: https://apps.charliekrug.com/redosaur/
- Source: https://github.com/ctkrug/redosaur
