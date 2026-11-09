// Bump the version everywhere, commit, tag and push. CI then builds the
// installers and publishes the release that installed copies update from.
//
//   npm run release -- 0.2.0
import { execSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version || '')) {
  console.error('Usage: npm run release -- <major.minor.patch>');
  process.exit(1);
}

const edit = (file, fn) => writeFileSync(file, fn(readFileSync(file, 'utf8')));
edit('src-tauri/tauri.conf.json', (s) => s.replace(/"version":\s*"[^"]+"/, `"version": "${version}"`));
edit('src-tauri/Cargo.toml', (s) => s.replace(/^version = "[^"]+"/m, `version = "${version}"`));
edit('package.json', (s) => s.replace(/"version":\s*"[^"]+"/, `"version": "${version}"`));

const run = (cmd) => execSync(cmd, { stdio: 'inherit' });
run('cargo update -p oxide --offline || cargo check -q');
run('git add src-tauri/tauri.conf.json src-tauri/Cargo.toml package.json Cargo.lock');
run(`git commit -m "Release v${version}"`);
run(`git tag v${version}`);
run('git push');
run(`git push origin v${version}`);
console.log(`\nPushed v${version}. GitHub Actions is building the release now.`);
