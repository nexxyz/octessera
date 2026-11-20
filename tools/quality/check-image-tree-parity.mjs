import { execFileSync } from "node:child_process";

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

function trackedBlobs(tree) {
  const listing = execFileSync("git", ["ls-files", "-s", "-z", "--", tree], {
    encoding: "utf8",
  });
  return new Map(
    listing
      .split("\0")
      .filter(Boolean)
      .map((line) => {
        const [meta, path] = line.split("\t");
        return [path.slice(tree.length + 1), meta.split(" ")[1]];
      }),
  );
}

const pi = trackedBlobs(PI_TREE);
const orange = trackedBlobs(ORANGE_TREE);
const shared = [...pi.keys()].filter((rel) => orange.has(rel));
const differing = shared.filter(
  (rel) => !MAY_DIFFER.has(rel) && pi.get(rel) !== orange.get(rel),
);
for (const rel of differing)
  console.error(`differs between image trees: ${rel}`);
if (differing.length > 0) process.exit(1);
console.log(`${shared.length} shared image files checked.`);
