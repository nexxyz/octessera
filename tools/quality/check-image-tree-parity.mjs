import { execFileSync } from "node:child_process";

const PI_TREE = "tools/pi-image/stage4-octessera/files/root";
const ORANGE_TREE = "userpatches/overlay";
const IN_BOTH = [
  "etc/default/locale",
  "etc/octessera/setup-profile",
  "etc/profile.d/octessera-welcome.sh",
  "etc/sudoers.d/octessera-update",
  "etc/systemd/journald.conf.d/10-octessera.conf",
  "etc/systemd/system/octessera-setup-request.path",
  "etc/systemd/system/octessera-setup.service",
  "etc/systemd/system/octessera-show-hold.path",
  "etc/systemd/system/octessera-show-hold.service",
  "etc/systemd/system/octessera-update-switch@.service",
  "etc/systemd/system/octessera.service",
  "etc/tmpfiles.d/octessera-setup-request.conf",
  "usr/lib/systemd/system-shutdown/octessera-oled-off",
  "usr/local/lib/octessera/setup_config.py",
  "usr/local/lib/octessera/setup_hotspot.py",
  "usr/local/lib/octessera/setup_http.py",
  "usr/local/lib/octessera/setup_status.py",
  "usr/local/lib/octessera/setup_wifi_join.py",
  "usr/local/sbin/octessera-setup",
  "usr/local/sbin/octessera-show-hold-apply",
  "usr/local/share/octessera-setup-ui/README.md",
  "usr/local/share/octessera-setup-ui/css/styles.css",
  "usr/local/share/octessera-setup-ui/img/octessera-mark.svg",
  "usr/local/share/octessera-setup-ui/img/octessera-wordmark.svg",
  "usr/local/share/octessera-setup-ui/index.html",
  "usr/local/share/octessera-setup-ui/js/app.js",
];
const MAY_DIFFER = new Set([
  "etc/octessera/setup-profile",
  "etc/sudoers.d/octessera-update",
  "etc/systemd/system/octessera-show-hold.service",
  "etc/systemd/system/octessera.service",
  "etc/tmpfiles.d/octessera-setup-request.conf",
  "usr/lib/systemd/system-shutdown/octessera-oled-off",
  "usr/local/sbin/octessera-show-hold-apply",
  "usr/local/share/octessera-setup-ui/index.html",
]);

function trackedEntries(tree) {
  const listing = execFileSync("git", ["ls-files", "-s", "-z", "--", tree], {
    encoding: "utf8",
  });
  return new Map(
    listing
      .split("\0")
      .filter(Boolean)
      .map((line) => {
        const [meta, path] = line.split("\t");
        const [mode, blob] = meta.split(" ");
        return [path.slice(tree.length + 1), { mode, blob }];
      }),
  );
}

const pi = trackedEntries(PI_TREE);
const orange = trackedEntries(ORANGE_TREE);
const listed = new Set(IN_BOTH);
const problems = [];
for (const rel of MAY_DIFFER)
  if (!listed.has(rel))
    problems.push(`may differ but is not listed in both trees: ${rel}`);
for (const rel of pi.keys())
  if (orange.has(rel) && !listed.has(rel))
    problems.push(`in both image trees but not listed: ${rel}`);
for (const rel of IN_BOTH) {
  const [piEntry, orangeEntry] = [pi.get(rel), orange.get(rel)];
  if (!piEntry || !orangeEntry) {
    problems.push(
      `missing from the ${piEntry ? "Orange" : "Raspberry"} image tree: ${rel}`,
    );
  } else if (piEntry.mode !== orangeEntry.mode) {
    problems.push(`file mode differs between image trees: ${rel}`);
  } else if (!MAY_DIFFER.has(rel) && piEntry.blob !== orangeEntry.blob) {
    problems.push(`differs between image trees: ${rel}`);
  }
}
for (const problem of problems) console.error(problem);
if (problems.length > 0) process.exit(1);
console.log(`${IN_BOTH.length} shared image files checked.`);
