import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const PI_TREE = "tools/pi-image/stage4-octessera/files/root";
const ORANGE_TREE = "userpatches/overlay";
const MAY_DIFFER = new Set([
  "etc/octessera/setup-profile",
  "etc/sudoers.d/octessera-update",
  "etc/systemd/system/octessera-show-hold.service",
  "etc/systemd/system/octessera.service",
  "etc/tmpfiles.d/octessera-setup-request.conf",
  "usr/local/sbin/octessera-show-hold-apply",
  "usr/local/share/octessera-setup-ui/index.html",
]);

function listFiles(dir, base = dir) {
  return readdirSync(dir).flatMap((entry) => {
    const full = join(dir, entry);
    return statSync(full).isDirectory()
      ? listFiles(full, base)
      : [relative(base, full).replace(/\\/g, "/")];
  });
}

const orange = new Set(listFiles(ORANGE_TREE));
const shared = listFiles(PI_TREE).filter((rel) => orange.has(rel));
const differing = shared.filter(
  (rel) =>
    !MAY_DIFFER.has(rel) &&
    !readFileSync(join(PI_TREE, rel)).equals(readFileSync(join(ORANGE_TREE, rel))),
);
for (const rel of differing) console.error(`differs between image trees: ${rel}`);
if (differing.length > 0) process.exit(1);
console.log(`${shared.length} shared image files checked.`);
