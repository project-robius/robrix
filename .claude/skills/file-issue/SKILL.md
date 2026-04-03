---
name: file-issue
description: Document a bug/fix locally in issues/ and create a matching GitHub issue
allowed-tools:
  - Bash(ls:*)
  - Bash(mkdir:*)
  - Bash(gh:*)
  - Glob
  - Grep
  - Read
  - Write
when_to_use: |
  Use when the user wants to document a discovered bug, applied fix, and remaining issues
  as both a local issue file and a GitHub issue. Typically invoked after a debugging/fix session.
  Examples: "file an issue for this", "record this bug", "create issue", "file-issue"
---

# File Issue

Document a bug discovery and fix as a local issue file in `issues/` and a matching GitHub issue.
All output is written in English regardless of conversation language.

## Goal

Produce two artifacts:
1. A detailed local issue document at `issues/NNN-slug.md`
2. A GitHub issue with a summary version

## Steps

### 1. Scan for next issue number

Check if `issues/` directory exists in the project root. Create it if missing.
List existing files to determine the next sequential number (e.g., if `001-*` exists, next is `002`).

**Success criteria**: Know the next issue number (zero-padded to 3 digits) and confirmed `issues/` dir exists.

### 2. Gather context from conversation

Extract from the current conversation:
- **Summary**: One-line description of the bug
- **Severity**: Critical / High / Medium / Low
- **Symptoms**: What the user observed (UI behavior, error messages, logs)
- **Root Cause**: Technical explanation of why it happens
- **Reproduction**: Steps to reproduce
- **Fix Applied**: What was changed and why (include code snippets if relevant)
- **Remaining Issues**: Known limitations, follow-up work, upstream bugs
- **Files Changed**: List of modified files
- **Test Verification**: Before/after comparison table

Generate a kebab-case slug from the summary (e.g., `dock-load-state-drawlist-corruption`).

**Success criteria**: All template sections populated with specific, accurate details from the session.

### 3. Write local issue document

Write to `issues/NNN-slug.md` using this template:

```markdown
# Issue #NNN: {Summary}

**Date:** {YYYY-MM-DD}
**Severity:** {Critical|High|Medium|Low}
**Status:** Fixed (workaround applied) | Fixed | Open
**Affected component:** {file path(s)}

## Summary
{One paragraph}

## Symptoms
{Bullet list of what the user observed}

## Root Cause
{Technical explanation with code snippets}

## Reproduction
{Numbered steps}

## Fix Applied
{Description + key code changes}

## Remaining Issues
{Numbered list of known limitations and follow-up work}

## Files Changed
{Bullet list}

## Test Verification
{Before/after table}
```

**Success criteria**: File written, all sections filled, no placeholder text remaining.

### 4. Create GitHub issue

Detect the repo with `gh repo view --json nameWithOwner`.
Create a GitHub issue via `gh issue create` with:
- Title: same as local doc summary (concise, under 80 chars)
- Label: `bug`
- Body: condensed version with Summary, Symptoms, Root Cause, Fix Applied, Remaining Issues (as checklist), and Environment section
- Reference the local doc path in the body

**Rules**:
- Use a HEREDOC for the body to preserve formatting
- Remaining Issues should be `- [ ]` checklist items
- Include a link/reference to the local issue doc

**Success criteria**: GitHub issue created, URL returned.

### 5. Report results

Tell the user:
- Local issue doc path
- GitHub issue URL (in `owner/repo#number` format for clickable link)

**Success criteria**: Both paths reported in a concise summary.
