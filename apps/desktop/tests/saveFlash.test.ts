import assert from "node:assert/strict";
import test from "node:test";
import { saveFlashVisible } from "../src/ui/saveFlash";

test("save flash expires without requiring a new runtime snapshot", () => {
  assert.equal(saveFlashVisible(1000, 1200), true);
  assert.equal(saveFlashVisible(1000, 1700), false);
  assert.equal(saveFlashVisible(null, 1200), false);
});
