// Bump the version everywhere, commit, tag and push. CI then builds the
// installers and publishes the release that installed copies update from.
//
//   npm run release -- 1.0.1 notes.md
//
// notes.md says what changed, in Markdown. It becomes the GitHub release
// text and the "What's new" in the in-app update prompt (shown as plain
// text there, first 600 characters), so lead with the most important
// change. The notes travel in the version tag's message.
import { execFileSync, execSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';

const [version, notesFile] = process.argv.slice(2);
const usage = () => {
  console.error('Usage: npm run release -- <major.minor.patch> <notes.md>');
  process.exit(1);
};
if (!/^\d+\.\d+\.\d+$/.test(version || '')) usage();
if (!notesFile) usage();
if (!existsSync(notesFile)) {
  console.error(`Release notes not found: ${notesFile}`);
  process.exit(1);
}
const notes = readFileSync(notesFile, 'utf8').replace(/\r\n/g, '\n').trim();
if (!notes) {
  console.error(`Release notes are empty: ${notesFile}`);
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
// Annotated tag carrying the notes. --cleanup=verbatim keeps Markdown
// headings ("## ...") that git would otherwise drop as comments.
execFileSync('git', ['tag', '-a', `v${version}`, '--cleanup=verbatim', '-F', '-'], { input: notes + '\n', stdio: ['pipe', 'inherit', 'inherit'] });
run('git push');
run(`git push origin v${version}`);
console.log(`\nPushed v${version}. GitHub Actions is building the release now.`);
