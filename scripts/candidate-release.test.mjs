import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { URL } from "node:url";
import { candidatePlan } from "./candidate-release.mjs";

const source = { ref: "develop", sha: "a".repeat(40) };
const workflow = readFileSync(
  new URL("../.github/workflows/candidate-release.yml", import.meta.url),
  "utf8",
);
const imageWorkflow = readFileSync(
  new URL("../.github/workflows/images.yml", import.meta.url),
  "utf8",
);

test("candidate plan accepts independent SemVer release candidates", () => {
  const plan = candidatePlan({ ...source, version: "1.5.0-rc.1" });
  assert.equal(plan.releaseTag, "v1.5.0-rc.1");
  assert.deepEqual(plan.imageTags, ["1.5.0-rc.1"]);
  assert.equal(plan.assetName, "candidate-1.5.0-rc.1.json");
  assert.equal(plan.sourceCommit, source.sha);
});

test("candidate image plan cannot emit a stable, edge, or latest alias", () => {
  const { imageTags } = candidatePlan({ ...source, version: "0.4.0-rc.12" });
  assert.deepEqual(imageTags, ["0.4.0-rc.12"]);
  assert.equal(
    imageTags.some((tag) => ["edge", "latest", "0.4", "0"].includes(tag)),
    false,
  );
});

test("candidate plan rejects stable, malformed, and padded versions", () => {
  for (const version of [
    "1.5.0",
    "v1.5.0-rc.1",
    "1.5.0-rc.0",
    "01.5.0-rc.1",
    "1.5.0-rc.01",
    "1.5.0-rc.1+build",
  ]) {
    assert.throws(
      () => candidatePlan({ ...source, version }),
      /Candidate version/,
    );
  }
});

test("candidate plan requires the reviewed develop ref and a full commit SHA", () => {
  assert.throws(
    () =>
      candidatePlan({ version: "1.5.0-rc.1", ref: "main", sha: source.sha }),
    /develop/,
  );
  assert.throws(
    () =>
      candidatePlan({
        version: "1.5.0-rc.1",
        ref: "develop",
        sha: "a".repeat(39),
      }),
    /full commit SHA/,
  );
});

test("Studio candidate workflow is manual, requires a signed Runtime candidate, and runs the image checks", () => {
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /runtime_version:/);
  assert.match(workflow, /candidate: true/);
  assert.match(workflow, /runtime_digest:/);
  assert.match(workflow, /--latest=false/);
  assert.match(workflow, /isImmutable/);
  assert.match(workflow, /cosign verify/);
  assert.match(workflow, /Reject reused Studio references/);
  assert.ok(
    workflow.indexOf("gh release upload") < workflow.indexOf("draft=false"),
  );
});

test("Studio image metadata emits only the requested candidate tag", () => {
  assert.match(
    imageWorkflow,
    /type=raw,value=\$\{\{ inputs\.version \}\},enable=\$\{\{ inputs\.candidate == true \}\}/,
  );
  assert.match(
    imageWorkflow,
    /type=raw,value=latest,enable=\$\{\{ inputs\.version != '' && inputs\.candidate != true \}\}/,
  );
  assert.match(
    imageWorkflow,
    /type=semver,pattern=\{\{major\}\}\.\{\{minor\}\}[^\n]*inputs\.candidate != true/,
  );
  assert.match(
    imageWorkflow,
    /Prerelease versions must use the signed candidate workflow/,
  );
  assert.match(
    workflow,
    /Could not confirm that the candidate image tag is unused/,
  );
});
