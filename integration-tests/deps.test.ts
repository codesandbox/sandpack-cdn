import { fetchV2Deps, V2Deps } from "./utils";
import { parse as parseSemver } from "semver";

function validateContract(data: V2Deps) {
  expect(typeof data).toBe("object");
  for (let [key, value] of Object.entries(data)) {
    const splitKey = key.split("@");
    const majorVersion = splitKey.pop() ?? "";
    // TODO: Use a proper regex for this
    try {
      const parsedVersion = +majorVersion;
      expect(isNaN(parsedVersion)).toBe(false);
      const parsed = parseSemver(value);
      expect(parsed?.major).toBe(parsedVersion);
    } catch (err) {
      expect(typeof majorVersion).toBe("string");
    }
    expect(typeof key).toBe("string");
  }
}

test("react and next", async () => {
  const deps = await fetchV2Deps([
    { name: "react", range: "^18.1.0" },
    { name: "next", range: "^12.3.1" },
  ]);
  validateContract(deps);
  expect(typeof deps["react@18"]).toBe("string");
  expect(typeof deps["next@12"]).toBe("string");
});

test("next@latest", async () => {
  const deps = await fetchV2Deps([{ name: "next", range: "latest" }]);
  validateContract(deps);
  expect(typeof deps["next@13"]).toBe("string");
});

test("react@*", async () => {
  const deps = await fetchV2Deps([{ name: "react", range: "*" }]);
  validateContract(deps);
  expect(typeof deps["react@18"]).toBe("string");
});

test("react without range", async () => {
  const deps = await fetchV2Deps([{ name: "react", range: "" }]);
  validateContract(deps);
  expect(typeof deps["react@18"]).toBe("string");
});
