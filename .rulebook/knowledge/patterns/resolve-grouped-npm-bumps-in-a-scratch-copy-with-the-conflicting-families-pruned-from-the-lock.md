# Resolve grouped npm bumps in a scratch copy with the conflicting families pruned from the lock

**Category**: dependencies
**Tags**: npm, eresolve, lockfile, dependabot, typescript-eslint, vitest, phase11

## Description

Grouped Dependabot bumps of packages that pin each other exactly (typescript-eslint pins @typescript-eslint/eslint-plugin and parser at its own version; vitest pins @vitest/*) fail with ERESOLVE even when the target versions are consistent, because npm upgrades one root dependency at a time and each step hits the peer still locked for its sibling. Removing node_modules is not enough — the old package-lock.json anchors the resolver too. Without touching the project's node_modules: copy package.json and package-lock.json into an empty scratch directory, delete only the lock entries of the families being bumped (keys matching ^node_modules/(typescript-eslint|@typescript-eslint/|vitest|@vitest/)), run `npm install --package-lock-only --ignore-scripts`, then `npm audit fix --package-lock-only`, copy the lock back and run `npm install`. Every other locked version stays put, so the lock diff is limited to what the bump actually needs. Never reach for --force or --legacy-peer-deps here; the peers are consistent.

## Example

node -e "const fs=require('fs');const j=JSON.parse(fs.readFileSync('package-lock.json','utf8'));const re=/^node_modules\/(typescript-eslint|@typescript-eslint\/|vitest|@vitest\/)/;for(const k of Object.keys(j.packages))if(re.test(k))delete j.packages[k];fs.writeFileSync('package-lock.json',JSON.stringify(j,null,2)+'\n')"
npm install --package-lock-only --ignore-scripts
npm audit fix --package-lock-only --ignore-scripts

## When to Use

ERESOLVE on a grouped bump of packages that pin each other exactly, where the target versions are mutually consistent.

## When NOT to Use

A genuine peer conflict, where the requested versions cannot coexist: fix the version set instead.
