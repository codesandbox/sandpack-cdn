import { fetchV2Deps, V2Deps } from "./utils";
import { parse as parseSemver } from "semver";

function validateContract(data: V2Deps) {
  expect(typeof data).toBe("object");
  for (let [key, value] of Object.entries(data)) {
    const splitKey = key.split("@");
    const majorVersion = +(splitKey.pop() ?? "");
    expect(isNaN(majorVersion)).toBe(false);
    expect(typeof key).toBe("string");

    const parsed = parseSemver(value);
    expect(parsed?.major).toBe(majorVersion);
  }
}

test("react and next", async () => {
  const deps = await fetchV2Deps([
    { name: "react", range: "^18.1.0" },
    { name: "next", range: "^12.3.1" },
  ]);
  validateContract(deps);
});
