import test from "node:test";
import assert from "node:assert/strict";

import {
  createOledFrameRevision,
  isPositiveOledFrameRevision,
} from "../src/index";

test("OLED frame revisions are positive safe integers", () => {
  assert.equal(isPositiveOledFrameRevision(1), true);
  assert.equal(isPositiveOledFrameRevision(0), false);
  assert.equal(isPositiveOledFrameRevision(-1), false);
  assert.equal(isPositiveOledFrameRevision(1.5), false);
  assert.throws(() => createOledFrameRevision(0), RangeError);
});
