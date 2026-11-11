import { copyFileSync, cpSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const desktopDir = resolve(scriptDir, '..');
const repoRoot = resolve(desktopDir, '..', '..');
const tauriCli = resolve(
  desktopDir,
  'node_modules',
  '.bin',
  process.platform === 'win32' ? 'tauri.cmd' : 'tauri',
);
const sourceExe = resolve(
  repoRoot,
  'target',
  'release',
  'octessera-desktop.exe',
);
const outputExe = resolve(desktopDir, 'dist-desktop', 'octessera.exe');
const outputSamples = resolve(desktopDir, 'dist-desktop', 'samples');
const sourceSamples = resolve(repoRoot, 'release-samples', 'samples');
const outputLegal = resolve(desktopDir, 'dist-desktop', 'legal');
const sourceLegal = resolve(
  repoRoot,
  'release-legal',
  'usr',
  'share',
  'doc',
  'octessera',
);
const sourceNoticeBundle = resolve(
  repoRoot,
  'resources',
  'legal',
  'notice-bundle.json',
);
const result = spawnSync(
  process.env.ComSpec ?? 'cmd.exe',
  ['/d', '/s', '/c', 'call', tauriCli, 'build', '--no-bundle'],
  {
    cwd: desktopDir,
    stdio: 'inherit',
  },
);

if (result.error) {
  console.error(result.error);
  process.exit(1);
}

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

mkdirSync(dirname(outputExe), { recursive: true });
copyFileSync(sourceExe, outputExe);
rmSync(outputSamples, { recursive: true, force: true });
cpSync(sourceSamples, outputSamples, { recursive: true, errorOnExist: true });
rmSync(outputLegal, { recursive: true, force: true });
cpSync(sourceLegal, outputLegal, { recursive: true, errorOnExist: true });
copyFileSync(sourceNoticeBundle, resolve(outputLegal, 'notice-bundle.json'));
console.log(`Portable exe copied to ${outputExe}`);
